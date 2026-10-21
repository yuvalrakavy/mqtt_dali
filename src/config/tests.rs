use super::*;
use serde_json::{json, Value};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    directory: PathBuf,
    paths: ConfigPaths,
}

impl Fixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "mqtt-dali-v2-config-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        let paths = ConfigPaths::in_directory(&directory);
        Self { directory, paths }
    }

    fn legacy(&self, value: &Value) -> Vec<u8> {
        let bytes = serde_json::to_vec_pretty(value).unwrap();
        fs::write(&self.paths.legacy, &bytes).unwrap();
        bytes
    }

    fn load(&self) -> Result<LoadedConfig, ConfigError> {
        load_or_migrate(&self.paths, "DaliController")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn legacy() -> Value {
    json!({
        "name": "HouseController",
        "buses": [{
            "bus": 2,
            "description": "KitchenBus",
            "status": "NoPower",
            "channels": [
                { "short_address": 17, "description": "Café.50% #1" },
                { "short_address": 3, "description": "Counter" }
            ],
            "groups": [{
                "group_address": 9,
                "description": "Evening",
                "members": [17, 3]
            }]
        }]
    })
}

#[test]
fn imports_names_addresses_groups_and_order_without_writing_v1() {
    let fixture = Fixture::new();
    let original = fixture.legacy(&legacy());
    let loaded = fixture.load().unwrap();
    assert_eq!(loaded.origin, Origin::Migrated);
    assert_eq!(loaded.inventory.schema_version, 2);
    assert_eq!(loaded.inventory.revision, 1);
    assert_eq!(loaded.inventory.inventory_epoch.len(), 32);
    assert_eq!(loaded.inventory.name, "HouseController");
    assert_eq!(loaded.inventory.description, None);
    let bus = &loaded.inventory.buses[0];
    assert_eq!((bus.bus, bus.name.as_str()), (2, "KitchenBus"));
    assert!(bus.enabled, "cached NoPower is not a disabled setting");
    assert_eq!(bus.outputs[0].name, "Café.50% #1");
    assert_eq!(bus.outputs[0].short_address, 17);
    assert_eq!(bus.outputs[1].short_address, 3);
    assert_eq!(bus.groups[0].group_address, 9);
    assert_eq!(bus.groups[0].name, "Evening");
    assert_eq!(bus.groups[0].members, [17, 3]);
    assert_eq!(fs::read(&fixture.paths.legacy).unwrap(), original);
    let disk: Inventory =
        serde_json::from_slice(&fs::read(&fixture.paths.current).unwrap()).unwrap();
    assert_eq!(disk, loaded.inventory);
}

#[test]
fn existing_v2_wins_even_if_v1_is_corrupt_and_startup_name_differs() {
    let fixture = Fixture::new();
    fixture.legacy(&legacy());
    let first = fixture.load().unwrap();
    let current_bytes = fs::read(&fixture.paths.current).unwrap();
    fs::write(&fixture.paths.legacy, b"not JSON").unwrap();
    let loaded = load_or_migrate(&fixture.paths, "OtherController").unwrap();
    assert_eq!(loaded.origin, Origin::Existing);
    assert_eq!(loaded.inventory, first.inventory);
    assert_eq!(fs::read(&fixture.paths.current).unwrap(), current_bytes);
    assert_eq!(fs::read(&fixture.paths.legacy).unwrap(), b"not JSON");
}

#[test]
fn deleting_v2_reimports_current_v1_with_a_fresh_inventory_epoch() {
    let fixture = Fixture::new();
    fixture.legacy(&legacy());
    let first = fixture.load().unwrap();
    fs::remove_file(&fixture.paths.current).unwrap();
    let mut changed = legacy();
    changed["buses"][0]["channels"][0]["description"] = json!("RenamedInV1");
    let bytes = fixture.legacy(&changed);
    let second = fixture.load().unwrap();
    assert_eq!(second.origin, Origin::Migrated);
    assert_eq!(second.inventory.buses[0].outputs[0].name, "RenamedInV1");
    assert_ne!(
        first.inventory.inventory_epoch,
        second.inventory.inventory_epoch
    );
    assert_eq!(fs::read(&fixture.paths.legacy).unwrap(), bytes);
}

#[test]
fn omitted_legacy_groups_become_empty() {
    let fixture = Fixture::new();
    let mut value = legacy();
    value["buses"][0].as_object_mut().unwrap().remove("groups");
    fixture.legacy(&value);
    assert!(fixture.load().unwrap().inventory.buses[0].groups.is_empty());
}

#[test]
fn new_installation_creates_an_empty_inventory_without_a_legacy_file() {
    let fixture = Fixture::new();
    let loaded = load_or_migrate(&fixture.paths, "NewController").unwrap();
    assert_eq!(loaded.origin, Origin::Created);
    assert_eq!(loaded.inventory.name, "NewController");
    assert!(loaded.inventory.buses.is_empty());
    assert!(!fixture.paths.legacy.exists());
    assert!(fixture.paths.current.is_file());
}

#[test]
fn malformed_existing_v2_never_falls_back_or_gets_replaced() {
    let fixture = Fixture::new();
    let original = fixture.legacy(&legacy());
    fs::write(&fixture.paths.current, b"{broken").unwrap();
    assert!(fixture.load().is_err());
    assert_eq!(fs::read(&fixture.paths.current).unwrap(), b"{broken");
    assert_eq!(fs::read(&fixture.paths.legacy).unwrap(), original);
}

#[test]
fn unsupported_and_unknown_v2_fields_are_errors_not_lossy_reads() {
    let fixture = Fixture::new();
    fixture.legacy(&legacy());
    fixture.load().unwrap();
    let valid: Value = serde_json::from_slice(&fs::read(&fixture.paths.current).unwrap()).unwrap();
    for mutation in ["version", "unknown", "revision", "epoch"] {
        let mut value = valid.clone();
        match mutation {
            "version" => value["schema_version"] = json!(3),
            "unknown" => value["future_settings"] = json!({"must_not_be_lost": true}),
            "revision" => value["revision"] = json!(0),
            "epoch" => value["inventory_epoch"] = json!("invalid"),
            _ => unreachable!(),
        }
        let bytes = serde_json::to_vec(&value).unwrap();
        fs::write(&fixture.paths.current, &bytes).unwrap();
        assert!(fixture.load().is_err(), "must reject {mutation}");
        assert_eq!(fs::read(&fixture.paths.current).unwrap(), bytes);
    }
}

#[test]
fn invalid_legacy_references_names_and_addresses_do_not_create_v2() {
    let mutations: Vec<(&str, fn(&mut Value))> = vec![
        ("bus index", |v| v["buses"][0]["bus"] = json!(4)),
        ("short address", |v| {
            v["buses"][0]["channels"][0]["short_address"] = json!(64)
        }),
        ("group address", |v| {
            v["buses"][0]["groups"][0]["group_address"] = json!(16)
        }),
        ("missing member", |v| {
            v["buses"][0]["groups"][0]["members"] = json!([63])
        }),
        ("duplicate member", |v| {
            v["buses"][0]["groups"][0]["members"] = json!([17, 17])
        }),
        ("duplicate output name", |v| {
            v["buses"][0]["channels"][1]["description"] =
                v["buses"][0]["channels"][0]["description"].clone()
        }),
        ("duplicate output address", |v| {
            v["buses"][0]["channels"][1]["short_address"] = json!(17)
        }),
        ("empty name", |v| {
            v["buses"][0]["channels"][0]["description"] = json!(" ")
        }),
        ("path name", |v| {
            v["buses"][0]["channels"][0]["description"] = json!("Kitchen/Light")
        }),
        ("control character", |v| {
            v["buses"][0]["channels"][0]["description"] = json!("Light\n1")
        }),
        ("controller wildcard", |v| v["name"] = json!("House#")),
        ("unknown legacy field", |v| {
            v["buses"][0]["extra"] = json!(1)
        }),
    ];
    for (reason, mutate) in mutations {
        let fixture = Fixture::new();
        let mut value = legacy();
        mutate(&mut value);
        let bytes = fixture.legacy(&value);
        assert!(fixture.load().is_err(), "must reject {reason}");
        assert!(
            !fixture.paths.current.exists(),
            "partial file after {reason}"
        );
        assert_eq!(fs::read(&fixture.paths.legacy).unwrap(), bytes);
    }
}

#[test]
fn names_are_case_sensitive_and_scoped_by_bus_and_address_space() {
    let fixture = Fixture::new();
    let mut value = legacy();
    value["buses"][0]["channels"][0]["description"] = json!("Counter");
    value["buses"][0]["channels"][1]["description"] = json!("counter");
    value["buses"][0]["groups"][0]["description"] = json!("Counter");
    let mut other = value["buses"][0].clone();
    other["bus"] = json!(0);
    other["description"] = json!("OtherBus");
    value["buses"].as_array_mut().unwrap().push(other);
    fixture.legacy(&value);
    let loaded = fixture.load().unwrap();
    assert_eq!(
        loaded
            .inventory
            .buses
            .iter()
            .map(|b| b.bus)
            .collect::<Vec<_>>(),
        [2, 0]
    );
}

#[test]
fn concurrent_importers_use_one_winner_and_preserve_v1() {
    let fixture = Fixture::new();
    let original = fixture.legacy(&legacy());
    let barrier = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let paths = fixture.paths.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                load_or_migrate(&paths, "Ignored").unwrap()
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(
        results
            .iter()
            .filter(|c| c.origin == Origin::Migrated)
            .count(),
        1
    );
    assert!(results.iter().all(|c| c.inventory == results[0].inventory));
    assert_eq!(fs::read(&fixture.paths.legacy).unwrap(), original);
    let files: Vec<_> = fs::read_dir(&fixture.directory).unwrap().collect();
    assert_eq!(
        files.len(),
        2,
        "completed imports clean up their temporary names"
    );
}

#[test]
fn oversized_v2_and_oversized_serialized_migration_are_rejected() {
    let fixture = Fixture::new();
    fs::write(&fixture.paths.current, vec![b' '; MAX_CONFIG_BYTES + 1]).unwrap();
    assert!(fixture.load().is_err());
    fs::remove_file(&fixture.paths.current).unwrap();
    let mut value = legacy();
    let base = serde_json::to_vec(&value).unwrap().len();
    value["buses"][0]["channels"][0]["description"] = json!("x".repeat(MAX_CONFIG_BYTES - base));
    let bytes = serde_json::to_vec(&value).unwrap();
    assert!(bytes.len() < MAX_CONFIG_BYTES);
    fs::write(&fixture.paths.legacy, &bytes).unwrap();
    assert!(fixture.load().is_err());
    assert!(!fixture.paths.current.exists());
    assert_eq!(fs::read(&fixture.paths.legacy).unwrap(), bytes);
}

#[test]
fn same_paths_and_hard_link_aliases_are_rejected() {
    let fixture = Fixture::new();
    let paths = ConfigPaths {
        legacy: fixture.paths.legacy.clone(),
        current: fixture.paths.legacy.clone(),
    };
    assert!(matches!(
        load_or_migrate(&paths, "Ignored"),
        Err(ConfigError::AliasedPaths)
    ));
    assert!(
        !paths.legacy.exists(),
        "initialization must never write v2 into the v1 path"
    );
    let alias = fixture.directory.join("alias");
    std::os::unix::fs::symlink(&fixture.directory, &alias).unwrap();
    let resolved = ConfigPaths {
        legacy: paths.legacy.clone(),
        current: alias.join("dali.json"),
    };
    assert!(matches!(
        load_or_migrate(&resolved, "Ignored"),
        Err(ConfigError::AliasedPaths)
    ));
    assert!(!paths.legacy.exists());
    let original = fixture.legacy(&legacy());
    assert!(matches!(
        load_or_migrate(&paths, "Ignored"),
        Err(ConfigError::AliasedPaths)
    ));
    fs::hard_link(&fixture.paths.legacy, &fixture.paths.current).unwrap();
    assert!(matches!(fixture.load(), Err(ConfigError::AliasedPaths)));
    assert_eq!(fs::read(&fixture.paths.legacy).unwrap(), original);
}

#[test]
fn invalid_destination_and_orphan_temporary_files_never_damage_legacy() {
    let fixture = Fixture::new();
    let original = fixture.legacy(&legacy());
    let bad_paths = ConfigPaths {
        legacy: fixture.paths.legacy.clone(),
        current: fixture.directory.join("absent/dali.v2.json"),
    };
    assert!(load_or_migrate(&bad_paths, "Ignored").is_err());
    assert_eq!(fs::read(&fixture.paths.legacy).unwrap(), original);
    let orphan = fixture.directory.join(".dali.v2.json.orphan.tmp");
    fs::write(&orphan, b"{partial").unwrap();
    assert_eq!(fixture.load().unwrap().origin, Origin::Migrated);
    assert_eq!(fs::read(&orphan).unwrap(), b"{partial");
}

#[cfg(unix)]
#[test]
fn symlink_inputs_are_rejected_including_a_dangling_current_file() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let original = fixture.legacy(&legacy());
    symlink("missing-target", &fixture.paths.current).unwrap();
    assert!(fixture.load().is_err());
    assert!(!fixture.directory.join("missing-target").exists());
    assert_eq!(fs::read(&fixture.paths.legacy).unwrap(), original);
    fs::remove_file(&fixture.paths.current).unwrap();
    let real = fixture.directory.join("real-v1.json");
    fs::rename(&fixture.paths.legacy, &real).unwrap();
    symlink(&real, &fixture.paths.legacy).unwrap();
    assert!(fixture.load().is_err());
    assert!(!fixture.paths.current.exists());
    assert_eq!(fs::read(real).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn migrated_configuration_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    fixture.legacy(&legacy());
    fixture.load().unwrap();
    assert_eq!(
        fs::metadata(&fixture.paths.current)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn denied_current_read_or_destination_write_preserves_existing_files() {
    use std::os::unix::fs::PermissionsExt;
    // Root deliberately bypasses Unix mode checks; nextest on development/CI
    // runs as an ordinary user so the actual denied filesystem path is tested.
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let fixture = Fixture::new();
    let original = fixture.legacy(&legacy());
    fs::set_permissions(&fixture.directory, fs::Permissions::from_mode(0o500)).unwrap();
    let denied = fixture.load();
    fs::set_permissions(&fixture.directory, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(
        matches!(denied, Err(ConfigError::Io { .. })),
        "permission-denied publication must fail: {denied:?}"
    );
    assert!(!fixture.paths.current.exists());
    assert_eq!(fs::read(&fixture.paths.legacy).unwrap(), original);
    fixture.load().unwrap();
    let current = fs::read(&fixture.paths.current).unwrap();
    fs::set_permissions(&fixture.paths.current, fs::Permissions::from_mode(0o000)).unwrap();
    let denied = fixture.load();
    fs::set_permissions(&fixture.paths.current, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(
        matches!(denied, Err(ConfigError::Io { .. })),
        "an unreadable current file must never fall back: {denied:?}"
    );
    assert_eq!(fs::read(&fixture.paths.current).unwrap(), current);
    assert_eq!(fs::read(&fixture.paths.legacy).unwrap(), original);
}

#[test]
fn a_creator_arriving_at_publication_cannot_be_overwritten() {
    let fixture = Fixture::new();
    let winner = br#"{"winner":"other process"}"#;
    let created = publish_new_with(&fixture.paths.current, b"losing document", || {
        fs::write(&fixture.paths.current, winner).unwrap();
    });
    assert!(
        matches!(created, Ok(false)),
        "a publication replaced the concurrent winner: {created:?}"
    );
    assert_eq!(fs::read(&fixture.paths.current).unwrap(), winner);
}

#[cfg(unix)]
#[test]
fn a_symlink_swapped_after_metadata_check_is_not_followed() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let original = fixture.legacy(&legacy());
    let real = fixture.directory.join("protected.json");
    let result = read_regular_with(&fixture.paths.legacy, || {
        fs::rename(&fixture.paths.legacy, &real).unwrap();
        symlink(&real, &fixture.paths.legacy).unwrap();
    });
    assert!(
        result.is_err(),
        "the descriptor must be opened with no-follow"
    );
    assert_eq!(fs::read(real).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn a_fifo_swapped_after_metadata_check_does_not_block_or_get_read() {
    use std::sync::mpsc;
    use std::time::Duration;
    let fixture = Fixture::new();
    fixture.legacy(&legacy());
    let path = fixture.paths.legacy.clone();
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let result = read_regular_with(&path, || {
            fs::remove_file(&path).unwrap();
            let status = std::process::Command::new("mkfifo")
                .arg(&path)
                .status()
                .unwrap();
            assert!(status.success());
        });
        let _ = send.send(result.is_err());
    });
    assert!(receive
        .recv_timeout(Duration::from_secs(2))
        .expect("FIFO replacement must not block"));
}
#[test]
fn target_and_complete_snapshot_preflight_run_before_creating_the_v2_import() {
    for aggregate in [false, true] {
        let fixture = Fixture::new();
        let mut value = legacy();
        if aggregate {
            value["buses"][0]["channels"] = (0..64).map(|address| json!({
                "short_address": address, "description": format!("Light{address}{}", "x".repeat(1000))
            })).collect();
            value["buses"][0]["groups"] = (0..16).map(|address| json!({
                "group_address": address, "description": format!("Group{address}"), "members": (0..64).collect::<Vec<_>>()
            })).collect();
        } else {
            value["buses"][0]["channels"][0]["description"] = "💡".repeat(1000).into();
        }
        let original = fixture.legacy(&value);
        let error = match fixture.load() {
            Err(error) => error.to_string(),
            Ok(_) => panic!("unpublishable v1 inventory created a v2 file"),
        };
        assert!(
            error.contains(if aggregate {
                "complete inventory State"
            } else {
                "outputs[0].name"
            }),
            "wrong preflight failure: {error}"
        );
        assert!(
            !fixture.paths.current.exists(),
            "preflight failure published a v2 file"
        );
        assert_eq!(fs::read(&fixture.paths.legacy).unwrap(), original);
    }
}
