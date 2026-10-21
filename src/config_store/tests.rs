use super::*;
use crate::config::{ConfigPaths, Inventory};
use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, time::Duration};

struct Fixture {
    directory: PathBuf,
    paths: ConfigPaths,
    legacy: Vec<u8>,
}
impl Fixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "dali-store-{}-{:032x}",
            std::process::id(),
            rand::random::<u128>()
        ));
        fs::create_dir(&directory).unwrap();
        let paths = ConfigPaths::in_directory(&directory);
        let legacy = serde_json::to_vec(&json!({"name":"HouseController", "buses":[{"bus":0,"description":"Bus1","status":"Active","channels":[{"short_address":0,"description":"Light1"}]}]})).unwrap();
        fs::write(&paths.legacy, &legacy).unwrap();
        Self {
            directory,
            paths,
            legacy,
        }
    }
    async fn open(&self) -> ConfigStore {
        ConfigStore::open(self.paths.clone(), "Unused".into(), deadline())
            .await
            .unwrap()
    }
    fn disk(&self) -> Inventory {
        serde_json::from_slice(&fs::read(&self.paths.current).unwrap()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(2)
}
fn rename(name: &str) -> Vec<Edit> {
    vec![Edit::RenameOutput {
        bus: 0,
        address: 0,
        name: name.into(),
    }]
}

use super::disk::{Hooks, Stage, Step};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct Hold {
    entered: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
    hooks: Hooks,
}
impl Hold {
    fn new(at: Step) -> Self {
        let entered = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let (inside, resume) = (entered.clone(), release.clone());
        let hooks = Hooks {
            callback: Some(Arc::new(move |step| {
                if step == at && !inside.swap(true, Ordering::SeqCst) {
                    let limit = std::time::Instant::now() + Duration::from_secs(10);
                    while !resume.load(Ordering::SeqCst) {
                        assert!(
                            std::time::Instant::now() < limit,
                            "test failed to release held filesystem worker"
                        );
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
                Ok(())
            })),
        };
        Self {
            entered,
            release,
            hooks,
        }
    }
    async fn entered(&self) {
        tokio::time::timeout(Duration::from_secs(2), async {
            while !self.entered.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("filesystem injection not reached");
    }
    fn release(&self) {
        self.release.store(true, Ordering::SeqCst);
    }
}
impl Drop for Hold {
    fn drop(&mut self) {
        self.release();
    }
}
async fn idle(store: &ConfigStore) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while store.client.control.busy.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("filesystem worker did not finish");
}

#[tokio::test]
async fn timeouts_around_both_replacements_freeze_admission_until_reload_and_burn_the_revision() {
    for step in [
        Step::BeforeRename(Stage::Reservation),
        Step::AfterRename(Stage::Reservation),
        Step::BeforeRename(Stage::Mapping),
        Step::AfterRename(Stage::Mapping),
    ] {
        let fixture = Fixture::new();
        let hold = Hold::new(step);
        let store = ConfigStore::open_with(
            fixture.paths.clone(),
            "Unused".into(),
            deadline(),
            hold.hooks.clone(),
        )
        .await
        .unwrap();
        let client = store.clone();
        let pending = tokio::spawn(async move {
            client
                .edit(
                    1,
                    rename("Kitchen"),
                    Instant::now() + Duration::from_millis(100),
                )
                .await
        });
        hold.entered().await;
        assert!(matches!(pending.await.unwrap(), Outcome::Uncertain(_)));
        assert!(
            matches!(store.snapshot().phase, Phase::Uncertain(_)),
            "timed-out save reopened routing admission"
        );
        assert!(matches!(
            store.edit(1, rename("MustNotRun"), deadline()).await,
            Outcome::NotCommitted(_)
        ));
        assert!(
            ConfigStore::open(fixture.paths.clone(), "Unused".into(), deadline())
                .await
                .is_err(),
            "timeout released lifetime writer lock"
        );
        hold.release();
        idle(&store).await;
        assert!(
            matches!(store.snapshot().phase, Phase::Uncertain(_)),
            "late filesystem completion skipped recovery"
        );
        let revision = match store.recover(deadline()).await {
            Outcome::Committed { revision, .. } => revision,
            outcome => panic!("recovery failed: {outcome:?}"),
        };
        assert_eq!(store.snapshot().phase, Phase::Ready);
        assert_eq!(fixture.disk().revision_high_water, Some(2));
        assert_eq!(
            store.edit(revision, rename("Hall"), deadline()).await,
            Outcome::Committed {
                revision: 3,
                changed: true
            },
            "a possibly committed revision was reused"
        );
        assert_eq!(fs::read(&fixture.paths.legacy).unwrap(), fixture.legacy);
        assert!(store.close(deadline()).await);
    }
}

#[tokio::test]
async fn canceled_callers_keep_the_slot_and_close_does_not_claim_a_blocked_writer_finished() {
    let fixture = Fixture::new();
    let hold = Hold::new(Step::AfterRename(Stage::Mapping));
    let store = ConfigStore::open_with(
        fixture.paths.clone(),
        "Unused".into(),
        deadline(),
        hold.hooks.clone(),
    )
    .await
    .unwrap();
    let retained = store.clone();
    let client = store.clone();
    let task = tokio::spawn(async move { client.edit(1, rename("Kitchen"), deadline()).await });
    hold.entered().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(matches!(store.snapshot().phase, Phase::Uncertain(_)));
    assert!(
        matches!(store.recover(deadline()).await, Outcome::NotCommitted(_)),
        "recovery overlapped a pending writer"
    );
    assert!(
        !store
            .close(Instant::now() + Duration::from_millis(10))
            .await,
        "close claimed a blocked worker had released ownership"
    );
    assert_eq!(retained.snapshot().phase, Phase::Stopping);
    assert!(
        ConfigStore::open(fixture.paths.clone(), "Unused".into(), deadline())
            .await
            .is_err()
    );
    hold.release();
    assert!(store.close(deadline()).await);
    let restarted = fixture.open().await;
    assert_eq!(
        restarted.snapshot().model.inventory().buses[0].outputs[0].name,
        "Kitchen"
    );
    assert!(restarted.close(deadline()).await);
}

#[tokio::test]
async fn failed_reservation_sync_never_starts_mapping_and_failed_recovery_stays_frozen() {
    let fixture = Fixture::new();
    let fail = Arc::new(AtomicBool::new(true));
    let mappings = Arc::new(AtomicUsize::new(0));
    let (fault, count) = (fail.clone(), mappings.clone());
    let hooks = Hooks {
        callback: Some(Arc::new(move |step| {
            if step == Step::BeforeRename(Stage::Mapping) {
                count.fetch_add(1, Ordering::SeqCst);
            }
            if fault.load(Ordering::SeqCst)
                && matches!(
                    step,
                    Step::DirectorySync(Stage::Reservation) | Step::RecoverySync
                )
            {
                return Err("injected sync failure".into());
            }
            Ok(())
        })),
    };
    let store = ConfigStore::open_with(fixture.paths.clone(), "Unused".into(), deadline(), hooks)
        .await
        .unwrap();
    assert!(matches!(
        store.edit(1, rename("Kitchen"), deadline()).await,
        Outcome::Uncertain(_)
    ));
    assert_eq!(
        mappings.load(Ordering::SeqCst),
        0,
        "mapping written without durable revision reservation"
    );
    assert!(matches!(
        store.recover(deadline()).await,
        Outcome::Uncertain(_)
    ));
    assert!(matches!(store.snapshot().phase, Phase::Uncertain(_)));
    fail.store(false, Ordering::SeqCst);
    assert_eq!(
        store.recover(deadline()).await,
        Outcome::Committed {
            revision: 1,
            changed: false
        }
    );
    assert_eq!(
        store.edit(1, rename("Hall"), deadline()).await,
        Outcome::Committed {
            revision: 3,
            changed: true
        }
    );
    assert!(store.close(deadline()).await);
}

#[tokio::test]
async fn restart_after_an_uncertain_mapping_rollback_cannot_reuse_its_reserved_revision() {
    let fixture = Fixture::new();
    let hooks = Hooks {
        callback: Some(Arc::new(|step| {
            if step == Step::DirectorySync(Stage::Mapping) {
                Err("injected mapping sync failure".into())
            } else {
                Ok(())
            }
        })),
    };
    let store = ConfigStore::open_with(fixture.paths.clone(), "Unused".into(), deadline(), hooks)
        .await
        .unwrap();
    assert!(matches!(
        store.edit(1, rename("Kitchen"), deadline()).await,
        Outcome::Uncertain(_)
    ));
    assert_eq!(fixture.disk().revision, 2);
    // Simulate the directory returning to the durably reserved old document.
    let reserved = store.snapshot().model;
    assert_eq!(reserved.inventory().revision, 1);
    assert_eq!(reserved.inventory().revision_high_water, Some(2));
    assert!(store.close(deadline()).await);
    fs::write(
        &fixture.paths.current,
        serde_json::to_vec_pretty(reserved.inventory()).unwrap(),
    )
    .unwrap();
    let restarted = fixture.open().await;
    assert_eq!(
        restarted.edit(1, rename("Hall"), deadline()).await,
        Outcome::Committed {
            revision: 3,
            changed: true
        },
        "restart reused an uncertain revision"
    );
    assert!(restarted.close(deadline()).await);
}

#[tokio::test]
async fn unrecognized_external_files_are_never_adopted_or_overwritten_by_recovery() {
    let fixture = Fixture::new();
    let store = fixture.open().await;
    let mut external = fixture.disk();
    external.description = Some("unexpected offline edit".into());
    let bytes = serde_json::to_vec_pretty(&external).unwrap();
    fs::write(&fixture.paths.current, &bytes).unwrap();
    assert!(matches!(
        store.edit(1, rename("Kitchen"), deadline()).await,
        Outcome::Uncertain(_)
    ));
    assert!(
        matches!(store.recover(deadline()).await, Outcome::Uncertain(_)),
        "recovery adopted an unrecognized canonical document"
    );
    assert_eq!(fs::read(&fixture.paths.current).unwrap(), bytes);
    assert!(matches!(store.snapshot().phase, Phase::Uncertain(_)));
    assert!(store.close(deadline()).await);
}

#[tokio::test]
async fn writer_lock_refuses_symlinks_hard_links_and_public_permissions() {
    for case in 0..3 {
        let fixture = Fixture::new();
        let path = disk::lock_path(&fixture.paths).unwrap();
        match case {
            0 => std::os::unix::fs::symlink(&fixture.paths.legacy, &path).unwrap(),
            1 => fs::hard_link(&fixture.paths.legacy, &path).unwrap(),
            _ => {
                fs::write(&path, b"").unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            }
        }
        assert!(
            ConfigStore::open(fixture.paths.clone(), "Unused".into(), deadline())
                .await
                .is_err(),
            "accepted unsafe writer lock case {case}"
        );
        assert!(!fixture.paths.current.exists());
        assert_eq!(fs::read(&fixture.paths.legacy).unwrap(), fixture.legacy);
    }
}

#[tokio::test]
async fn startup_requires_file_and_directory_durability_before_admission() {
    let fixture = Fixture::new();
    let store = fixture.open().await;
    assert!(store.close(deadline()).await);
    let hooks = Hooks {
        callback: Some(Arc::new(|step| {
            if step == Step::StartupSync {
                Err("injected startup durability failure".into())
            } else {
                Ok(())
            }
        })),
    };
    let result =
        ConfigStore::open_with(fixture.paths.clone(), "Unused".into(), deadline(), hooks).await;
    assert!(
        result.is_err(),
        "startup admitted a mapping without confirming file and directory durability"
    );
}

#[tokio::test]
async fn the_adjacent_lock_must_not_create_or_modify_the_legacy_config_path() {
    let fixture = Fixture::new();
    let mut paths = fixture.paths.clone();
    paths.legacy = disk::lock_path(&paths).unwrap();
    let result = ConfigStore::open(paths.clone(), "HouseController".into(), deadline()).await;
    assert!(result.is_err());
    assert!(
        !paths.legacy.exists(),
        "opening the lock created the legacy configuration file"
    );
    assert!(!paths.current.exists());
}

#[tokio::test]
async fn cancellation_winning_finalization_keeps_edit_and_recovery_frozen() {
    for recovering in [false, true] {
        let fixture = Fixture::new();
        let hold = Hold::new(Step::BeforeFinalize);
        let held = hold.hooks.clone();
        let first = AtomicBool::new(recovering);
        let hooks = Hooks {
            callback: Some(Arc::new(move |step| {
                if step == Step::DirectorySync(Stage::Mapping)
                    && first.swap(false, Ordering::SeqCst)
                {
                    return Err("first mapping sync fails to require recovery".into());
                }
                held.check(step)
            })),
        };
        let store =
            ConfigStore::open_with(fixture.paths.clone(), "Unused".into(), deadline(), hooks)
                .await
                .unwrap();
        if recovering {
            assert!(matches!(
                store.edit(1, rename("Kitchen"), deadline()).await,
                Outcome::Uncertain(_)
            ));
        }
        let client = store.clone();
        let task = tokio::spawn(async move {
            if recovering {
                client.recover(deadline()).await
            } else {
                client.edit(1, rename("Kitchen"), deadline()).await
            }
        });
        hold.entered().await;
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(matches!(store.snapshot().phase, Phase::Uncertain(_)));
        hold.release();
        idle(&store).await;
        assert!(
            matches!(store.snapshot().phase, Phase::Uncertain(_)),
            "canceled finalization skipped required recovery (recovering={recovering})"
        );
        assert_eq!(
            store.recover(deadline()).await,
            Outcome::Committed {
                revision: 2,
                changed: false
            }
        );
        assert_eq!(store.snapshot().phase, Phase::Ready);
        assert!(store.close(deadline()).await);
    }
}

#[tokio::test]
async fn failures_before_replace_are_not_committed_and_descriptive_revisions_are_reserved_too() {
    for stage in [Stage::Reservation, Stage::Mapping] {
        let fixture = Fixture::new();
        let first = AtomicBool::new(true);
        let hooks = Hooks {
            callback: Some(Arc::new(move |step| {
                if step == Step::BeforeRename(stage) && first.swap(false, Ordering::SeqCst) {
                    Err("injected before replacement".into())
                } else {
                    Ok(())
                }
            })),
        };
        let store =
            ConfigStore::open_with(fixture.paths.clone(), "Unused".into(), deadline(), hooks)
                .await
                .unwrap();
        let edit = || {
            vec![Edit::Description {
                description: Some("Test controller".into()),
            }]
        };
        assert!(matches!(
            store.edit(1, edit(), deadline()).await,
            Outcome::NotCommitted(_)
        ));
        assert_eq!(store.snapshot().phase, Phase::Ready);
        assert_eq!(fixture.disk().revision, 1);
        let expected = if stage == Stage::Mapping { 3 } else { 2 };
        assert_eq!(
            store.edit(1, edit(), deadline()).await,
            Outcome::Committed {
                revision: expected,
                changed: true
            },
            "descriptive save reused a durably reserved revision"
        );
        let disk = fixture.disk();
        assert_eq!(disk.revision_high_water, Some(expected));
        assert!(disk.identity_ledger.as_ref().unwrap().batches.is_empty());
        assert_eq!(store.snapshot().model.projections()[0].binding_revision, 1);
        assert_eq!(
            fs::read_dir(&fixture.directory).unwrap().count(),
            3,
            "completed save left a temporary file"
        );
        assert!(store.close(deadline()).await);
    }
}

#[tokio::test]
async fn a_canceled_queued_edit_never_touches_disk_or_consumes_a_revision() {
    let fixture = Fixture::new();
    let hold = Hold::new(Step::BeforeRequest);
    let store = ConfigStore::open_with(
        fixture.paths.clone(),
        "Unused".into(),
        deadline(),
        hold.hooks.clone(),
    )
    .await
    .unwrap();
    let before = fs::read(&fixture.paths.current).unwrap();
    let client = store.clone();
    let task = tokio::spawn(async move { client.edit(1, rename("Never"), deadline()).await });
    hold.entered().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    hold.release();
    idle(&store).await;
    assert_eq!(store.snapshot().phase, Phase::Ready);
    assert_eq!(
        before,
        fs::read(&fixture.paths.current).unwrap(),
        "canceled queued work touched disk"
    );
    assert_eq!(
        store.edit(1, rename("Kitchen"), deadline()).await,
        Outcome::Committed {
            revision: 2,
            changed: true
        }
    );
    assert!(store.close(deadline()).await);
}

#[tokio::test]
async fn canceling_open_keeps_ownership_until_the_worker_stops_and_prevents_late_import() {
    let fixture = Fixture::new();
    let hold = Hold::new(Step::StartupBeforeLoad);
    let paths = fixture.paths.clone();
    let hooks = hold.hooks.clone();
    let task = tokio::spawn(async move {
        ConfigStore::open_with(paths, "Unused".into(), deadline(), hooks).await
    });
    hold.entered().await;
    task.abort();
    assert!(task.await.is_err());
    assert!(
        ConfigStore::open(fixture.paths.clone(), "Unused".into(), deadline())
            .await
            .is_err(),
        "canceled startup released ownership while its worker was still alive"
    );
    hold.release();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(lease) = disk::acquire(&fixture.paths) {
                drop(lease);
                break;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        !fixture.paths.current.exists(),
        "a canceled startup imported a late file"
    );
    let replacement = fixture.open().await;
    assert!(replacement.close(deadline()).await);
}

#[tokio::test]
async fn durable_edits_preserve_v1_and_private_files_and_reload_exact_history() {
    let fixture = Fixture::new();
    let store = fixture.open().await;
    assert_eq!(store.snapshot().phase, Phase::Ready);
    assert_eq!(
        store.edit(1, rename("Kitchen"), deadline()).await,
        Outcome::Committed {
            revision: 2,
            changed: true
        }
    );
    let committed = store.snapshot().model;
    assert_eq!(fixture.disk(), *committed.inventory());
    assert_eq!(fixture.disk().revision_high_water, Some(2));
    assert_eq!(
        fixture
            .disk()
            .identity_ledger
            .as_ref()
            .unwrap()
            .batches
            .len(),
        1
    );
    assert_eq!(fs::read(&fixture.paths.legacy).unwrap(), fixture.legacy);
    assert_eq!(
        fs::metadata(&fixture.paths.current)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let bytes = fs::read(&fixture.paths.current).unwrap();
    assert_eq!(
        store.edit(2, rename("Kitchen"), deadline()).await,
        Outcome::Committed {
            revision: 2,
            changed: false
        }
    );
    assert_eq!(bytes, fs::read(&fixture.paths.current).unwrap());
    assert!(matches!(
        store.edit(1, rename("Bad"), deadline()).await,
        Outcome::NotCommitted(_)
    ));
    assert!(store.close(deadline()).await);
    // Public handles do not retain writer ownership after a successful close.
    let reopened = fixture.open().await;
    assert_eq!(
        reopened.snapshot().model.state_bytes().unwrap(),
        committed.state_bytes().unwrap()
    );
    assert!(matches!(
        store.edit(2, rename("Closed"), deadline()).await,
        Outcome::NotCommitted(_)
    ));
    assert!(reopened.close(deadline()).await);
}

#[tokio::test]
async fn a_second_writer_is_refused_and_a_retained_handle_does_not_prevent_close() {
    let fixture = Fixture::new();
    let store = fixture.open().await;
    let retained = store.clone();
    assert!(
        ConfigStore::open(fixture.paths.clone(), "Unused".into(), deadline())
            .await
            .is_err(),
        "two writers acquired one config lifetime"
    );
    assert!(store.close(deadline()).await);
    assert_eq!(retained.snapshot().phase, Phase::Closed);
    let replacement = fixture.open().await;
    assert!(replacement.close(deadline()).await);
}
