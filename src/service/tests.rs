use super::*;
use crate::{config::Inventory, inventory::Location};
use serde_json::json;
mod buses;
mod commissioning;
mod diagnostics;
mod discovery;
mod emergency;
mod emergency_uart;
mod emulator_tcp;
mod energy;
mod group_output;
mod groups;
mod identification;
mod input;
mod input_assignment;
mod live_input;
mod luminaire;
mod memory;
mod output_settings;
mod readback;
mod reassignment;
mod removal;
mod restoration;
mod scene_operations;
mod scene_saved_view;
mod scene_settings;
mod scenes;
mod tcp_hat;

#[derive(Default)]
pub(super) struct Pause {
    pub(super) armed: std::sync::atomic::AtomicBool,
    pub(super) entered: tokio::sync::Notify,
    pub(super) release: tokio::sync::Notify,
}
impl Pause {
    pub(super) async fn point(&self) {
        if self.armed.swap(false, std::sync::atomic::Ordering::AcqRel) {
            self.entered.notify_one();
            self.release.notified().await;
        }
    }
}
pub(super) fn model() -> Model {
    Model::new(serde_json::from_value::<Inventory>(json!({"schema_version":2,"revision":1,"inventory_epoch":"0123456789abcdef0123456789abcdef","name":"Controller","buses":[{"bus":0,"name":"Bus1","enabled":true,"outputs":[{"short_address":0,"name":"Light1"}],"groups":[{"group_address":0,"name":"All","members":[0]}]}]})).unwrap()).unwrap()
}
pub(super) fn snapshot(model: Model, phase: Phase) -> Snapshot {
    Snapshot {
        model: Arc::new(model),
        phase,
        hardware: Hardware::NotStarted,
        outputs: Default::default(),
        group_outputs: Default::default(),
        inputs: Default::default(),
        feedbacks: Default::default(),
        input_contexts: Default::default(),
        input_listeners: Default::default(),
        identification: None,
        version: env!("CARGO_PKG_VERSION"),
        build_utc: build_info::BUILT_TIME_UTC,
    }
}
#[test]
fn routing_is_frozen_until_runtime_is_attached_and_while_a_save_is_uncertain() {
    let context = RequestContext::new("0123456789abcdef0123456789abcdef.1").unwrap();
    for phase in [
        Phase::AwaitingRuntime,
        Phase::Writing,
        Phase::Uncertain("pending".into()),
        Phase::Stopping,
        Phase::Closed,
    ] {
        let snap = snapshot(model(), phase);
        assert!(
            routing::admit(
                &snap,
                &Change::Edit {
                    expected: 1,
                    edit: MetadataEdit::Description(None)
                },
                &context
            )
            .is_err(),
            "frozen routing admitted a mutation"
        );
    }
}
#[test]
fn generation_tables_preserve_fixture_versions_but_rotate_controller_metadata() {
    let model = model();
    let first = routing::generations(&model);
    assert_eq!(first.instance, "0123456789abcdef0123456789abcdef.1");
    let described = model
        .edit(
            1,
            &[Edit::Description {
                description: Some("house".into()),
            }],
        )
        .unwrap();
    let second = routing::generations(&described);
    assert_eq!(
        second.targets["Bus1.Outputs.Light1"],
        first.targets["Bus1.Outputs.Light1"]
    );
    assert_ne!(second.instance, first.instance);
    assert_eq!(second.targets["__inventory"], second.instance);
    let renamed = described
        .edit(
            2,
            &[Edit::RenameOutput {
                bus: 0,
                address: 0,
                name: "Kitchen".into(),
            }],
        )
        .unwrap();
    let table = routing::generations(&renamed);
    assert!(!table.targets.contains_key("Bus1.Outputs.Light1"));
    assert_eq!(
        table.targets["Bus1.Outputs.Kitchen"],
        "0123456789abcdef0123456789abcdef.3"
    );
    assert_eq!(
        table.targets["Bus1.Groups.All"],
        first.targets["Bus1.Groups.All"]
    );
    assert_eq!(
        renamed
            .projections()
            .iter()
            .find(|p| p.target == "Bus1.Outputs.Kitchen")
            .unwrap()
            .location,
        Location::Output {
            bus: 0,
            short_address: 0
        }
    );
}

use crate::config_store::test_hooks::{Hooks, Stage, Step};
use mqtt_bridge_kit::{Bridge, Running};
use mqtt_test_broker::{FakeBroker, SendOptions};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
pub(super) struct Fixture {
    directory: PathBuf,
    paths: ConfigPaths,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "dali-service-{}-{:032x}",
            std::process::id(),
            rand::random::<u128>()
        ));
        fs::create_dir(&directory).unwrap();
        let paths = ConfigPaths::in_directory(&directory);
        fs::write(
            &paths.current,
            serde_json::to_vec(model().inventory()).unwrap(),
        )
        .unwrap();
        fs::set_permissions(&paths.current, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&paths.legacy, b"legacy must not be touched").unwrap();
        Self { directory, paths }
    }
    pub(super) fn service(&self) -> (DaliService, ServiceHandle) {
        DaliService::new(self.paths.clone(), "Controller".into())
    }
    pub(super) fn add_output(&self, address: u8, name: &str) {
        let mut inventory = self.disk();
        inventory.buses[0].outputs.push(crate::config::Output {
            short_address: address,
            name: name.into(),
            description: None,
            color_representation: None,
        });
        fs::write(&self.paths.current, serde_json::to_vec(&inventory).unwrap()).unwrap();
    }
    pub(super) fn disk(&self) -> Inventory {
        serde_json::from_slice(&fs::read(&self.paths.current).unwrap()).unwrap()
    }
    pub(super) fn write_inventory(&self, inventory: &Inventory) {
        fs::write(&self.paths.current, serde_json::to_vec(inventory).unwrap()).unwrap();
    }
    pub(super) fn current_bytes(&self) -> Vec<u8> {
        fs::read(&self.paths.current).unwrap()
    }
    pub(super) fn add_button(&self) {
        let mut inventory = self.disk();
        inventory.buses[0].inputs = serde_json::from_value(json!([
            {"short_address":0,"name":"Panel","instances":[{"number":0,"name":"Button","instance_type":1}]}])).unwrap();
        fs::write(&self.paths.current, serde_json::to_vec(&inventory).unwrap()).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}
pub(super) fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(2)
}
pub(super) fn context(revision: u64) -> RequestContext {
    RequestContext::new(format!("0123456789abcdef0123456789abcdef.{revision}")).unwrap()
}

#[tokio::test]
async fn a_ready_response_resumed_after_its_original_deadline_is_uncertain() {
    use std::{future::Future, task::Poll};
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(service, &handle, &broker).await;
    let until = Instant::now() + Duration::from_millis(300);
    let mut request = Box::pin(handle.edit(
        1,
        MetadataEdit::Description(Some("saved".into())),
        context(1),
        until,
    ));
    std::future::poll_fn(|cx| {
        assert!(request.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    wait_for(&handle, |s| s.model.inventory().revision == 2).await;
    tokio::time::sleep_until(until + Duration::from_millis(10)).await;
    assert!(
        matches!(request.await, Err(ServiceError::Uncertain(_))),
        "ready response escaped original caller deadline"
    );
    assert!(running.stop().await);
}

#[tokio::test]
async fn the_actual_bridge_instance_must_match_the_persisted_controller_before_attachment() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = Bridge::new("DALI")
        .instance("Wrong")
        .broker(&broker.address())
        .start_managed(service)
        .await
        .unwrap();
    let reached_ready = tokio::time::timeout(
        Duration::from_millis(500),
        wait_for(&handle, |s| s.phase == Phase::Ready),
    )
    .await;
    assert!(
        reached_ready.is_err(),
        "mismatched runtime instance activated persisted controller"
    );
    assert!(broker
        .received_on("DALI/State/Wrong/__inventory")
        .is_empty());
    assert!(running.stop().await);
}
pub(super) async fn wait_for(handle: &ServiceHandle, predicate: impl Fn(&Snapshot) -> bool) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if handle.snapshot().is_some_and(|s| predicate(&s)) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .expect("service did not reach expected phase");
}
pub(super) async fn start(
    service: DaliService,
    handle: &ServiceHandle,
    broker: &FakeBroker,
) -> Running {
    let running = Bridge::new("DALI")
        .instance("Controller")
        .broker(&broker.address())
        .start_managed(service)
        .await
        .expect("service failed startup");
    wait_for(handle, |s| s.phase == Phase::Ready).await;
    running
}
#[tokio::test]
async fn inventory_service_works_without_mqtt_or_hardware_and_releases_the_real_writer() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    broker.refuse_connections(true);
    let (service, handle) = fixture.service();
    let running = start(service, &handle, &broker).await;
    let saved = handle
        .edit(
            1,
            MetadataEdit::RenameOutput {
                bus: 0,
                address: 0,
                name: "Kitchen".into(),
            },
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(saved.model.inventory().buses[0].outputs[0].name, "Kitchen");
    assert_eq!(fixture.disk().buses[0].outputs[0].name, "Kitchen");
    assert_eq!(saved.hardware, Hardware::NotStarted);
    assert!(
        handle
            .edit(1, MetadataEdit::Description(None), context(1), deadline())
            .await
            .is_err(),
        "stale revision was admitted"
    );
    assert_eq!(
        fs::read(&fixture.paths.legacy).unwrap(),
        b"legacy must not be touched"
    );
    assert!(
        running.stop().await,
        "healthy service could not stop without MQTT"
    );
    assert_eq!(handle.snapshot().unwrap().phase, Phase::Closed);
    assert_eq!(
        handle
            .edit(2, MetadataEdit::Description(None), context(2), deadline())
            .await
            .unwrap_err(),
        ServiceError::Closed
    );
    let reopened = crate::config_store::ConfigStore::open(
        fixture.paths.clone(),
        "Controller".into(),
        deadline(),
    )
    .await
    .expect("retained frontend handle kept writer ownership");
    assert!(reopened.close(deadline()).await);
}

#[tokio::test]
async fn mqtt_metadata_commands_use_original_tokens_and_publish_the_durable_inventory() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(service, &handle, &broker).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(3))
            .await
    );
    assert!(broker.publish("DALI/Command/Controller",r#"{"command":"rename","target":"Bus1.Outputs.Light1","name":"Kitchen","expected_revision":1}"#,&SendOptions::default().user_property("target-generation",context(1).generation().unwrap()).response_topic("test/reply")));
    wait_for(&handle, |s| {
        s.model.inventory().revision == 2 && s.phase == Phase::Ready
    })
    .await;
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| b
                .received_on("DALI/State/Controller/__inventory")
                .iter()
                .any(|m| m.user_property("target-generation")
                    == context(2).generation()
                    && serde_json::from_slice::<serde_json::Value>(&m.payload)
                        .is_ok_and(|v| v["Inventory"]["buses"][0]["outputs"][0]
                            ["name"]
                            == "Kitchen")))
            .await,
        "new inventory was not published with its confirmed generation"
    );
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| !b
                .received_on("test/reply")
                .is_empty())
            .await
    );
    let replies = broker.received_on("test/reply");
    assert_eq!(
        replies.len(),
        1,
        "committing command received multiple replies"
    );
    let reply: serde_json::Value = serde_json::from_slice(&replies[0].payload).unwrap();
    assert_eq!(
        reply["value"]["revision"], 2,
        "successful MQTT rename retired its own reply: {reply}"
    );
    assert!(reply.get("error").is_none());
    assert_eq!(
        replies[0].user_property("target-generation"),
        context(1).generation()
    );
    assert!(broker.publish("DALI/Command/Controller",r#"{"command":"rename","target":"Bus1.Outputs.Kitchen","name":"Bad","expected_revision":2}"#,&SendOptions::default().user_property("target-generation",context(1).generation().unwrap()).response_topic("test/stale")));
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| !b
                .received_on("test/stale")
                .is_empty())
            .await
    );
    assert_eq!(
        fixture.disk().buses[0].outputs[0].name,
        "Kitchen",
        "retired MQTT command changed configuration"
    );
    for (command, token, revision, topic) in [
        (
            r#"{"command":"describe","description":"House","expected_revision":2}"#,
            2,
            3,
            "test/describe",
        ),
        (
            r#"{"command":"rename_bus","bus":0,"name":"Upper","expected_revision":3}"#,
            3,
            4,
            "test/bus",
        ),
        (
            r#"{"command":"rename","target":"Upper.Groups.All","name":"WholeFloor","expected_revision":4}"#,
            4,
            5,
            "test/group",
        ),
    ] {
        assert!(broker.publish(
            "DALI/Command/Controller",
            command,
            &SendOptions::default()
                .user_property("target-generation", context(token).generation().unwrap())
                .response_topic(topic)
        ));
        assert!(
            broker
                .wait_until(Duration::from_secs(2), |b| !b.received_on(topic).is_empty())
                .await
        );
        let replies = broker.received_on(topic);
        assert_eq!(replies.len(), 1);
        let reply: serde_json::Value = serde_json::from_slice(&replies[0].payload).unwrap();
        assert_eq!(
            reply["value"]["revision"], revision,
            "committing metadata command did not return success: {reply}"
        );
        assert!(reply.get("error").is_none());
        assert_eq!(
            replies[0].user_property("target-generation"),
            context(token).generation()
        );
    }
    assert_eq!(fixture.disk().buses[0].name, "Upper");
    assert_eq!(fixture.disk().buses[0].groups[0].name, "WholeFloor");
    assert_eq!(fixture.disk().description.as_deref(), Some("House"));
    assert!(running.stop().await);
    for topic in ["test/reply", "test/describe", "test/bus", "test/group"] {
        assert_eq!(
            broker.received_on(topic).len(),
            1,
            "metadata command received a later duplicate completion"
        );
    }
}

#[tokio::test]
async fn mqtt_color_selection_is_durable_fenced_and_completes_under_original_token() {
    async fn command(
        broker: &FakeBroker,
        body: serde_json::Value,
        token: u64,
        number: usize,
    ) -> serde_json::Value {
        let topic = format!("test/color-selection/{number}");
        assert!(broker.publish(
            "DALI/Command/Controller",
            body.to_string(),
            &SendOptions::default()
                .user_property("target-generation", context(token).generation().unwrap())
                .response_topic(&topic)
        ));
        assert!(
            broker
                .wait_until(Duration::from_secs(2), |b| !b
                    .received_on(&topic)
                    .is_empty())
                .await,
            "color selection command never completed"
        );
        let replies = broker.received_on(&topic);
        assert_eq!(
            replies.len(),
            1,
            "color selection command received duplicate completion"
        );
        assert_eq!(
            replies[0].user_property("target-generation"),
            context(token).generation()
        );
        serde_json::from_slice(&replies[0].payload).unwrap()
    }
    fn selection(format: serde_json::Value, revision: u64) -> serde_json::Value {
        json!({"command":"set_color_representation","target":"Bus1.Outputs.Light1",
            "expected_revision":revision,"color_representation":format})
    }
    let fixture = Fixture::new();
    let legacy = fs::read(&fixture.paths.legacy).unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(service, &handle, &broker).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(3))
            .await
    );
    let reply = command(&broker, selection(json!("temperature"), 1), 1, 0).await;
    assert_eq!(
        reply["value"]["revision"], 2,
        "valid color selection did not commit: {reply}"
    );
    assert_eq!(
        serde_json::to_value(fixture.disk()).unwrap()["buses"][0]["outputs"][0]
            ["color_representation"],
        "temperature"
    );
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on("DALI/State/Controller/__inventory")
                .iter()
                .any(|m| m.user_property("target-generation")
                    == context(2).generation()
                    && serde_json::from_slice::<serde_json::Value>(&m.payload)
                        .is_ok_and(|v| v["Inventory"]["buses"][0]["outputs"][0]
                            ["color_representation"]
                            == "temperature")))
            .await,
        "durable color selection missing from MQTT inventory"
    );
    let reply = command(
        &broker,
        json!({"command":"describe","description":"House","expected_revision":2}),
        2,
        1,
    )
    .await;
    assert_eq!(reply["value"]["revision"], 3);
    // The controller is at 3, while this output's original binding remains 2.
    let reply = command(&broker, selection(json!("xy"), 3), 2, 2).await;
    assert_eq!(
        reply["value"]["revision"], 4,
        "color selection used controller token instead of output token: {reply}"
    );
    let reply = command(&broker, selection(json!("xy"), 4), 4, 3).await;
    assert_eq!(
        reply["value"]["revision"], 4,
        "same color selection was not a no-op"
    );
    let invalid = [
        (selection(json!("rgbwaf"), 4), 2),
        (selection(json!("rgbwaf"), 3), 4),
        (selection(json!("rgb"), 4), 4),
        (selection(json!({"temperature":null}), 4), 4),
        (
            json!({"command":"set_color_representation","target":"Bus1.Outputs.Light1","expected_revision":4}),
            4,
        ),
        (
            json!({"command":"set_color_representation","target":"Bus1.Groups.All","expected_revision":4,"color_representation":{"xy":null}}),
            1,
        ),
        (
            json!({"command":"set_color_representation","expected_revision":4,"color_representation":"xy"}),
            4,
        ),
        (
            json!({"command":"set_color_representation","target":"Bus1.Outputs.Light1","expected_revision":4,"color_representation":"xy","surprise":true}),
            4,
        ),
    ];
    let clear_number = 4 + invalid.len();
    for (index, (body, token)) in invalid.into_iter().enumerate() {
        let before = fs::read(&fixture.paths.current).unwrap();
        let reply = command(&broker, body, token, index + 4).await;
        assert!(
            reply.get("error").is_some(),
            "invalid color selection committed: {reply}"
        );
        assert_eq!(
            fs::read(&fixture.paths.current).unwrap(),
            before,
            "invalid color selection changed durable config"
        );
    }
    let reply = command(
        &broker,
        selection(serde_json::Value::Null, 4),
        4,
        clear_number,
    )
    .await;
    assert_eq!(
        reply["value"]["revision"], 5,
        "clearing color selection failed: {reply}"
    );
    assert!(
        serde_json::to_value(fixture.disk()).unwrap()["buses"][0]["outputs"][0]
            .get("color_representation")
            .is_none()
    );
    assert_eq!(fs::read(&fixture.paths.legacy).unwrap(), legacy);
    assert!(running.stop().await);
    for number in 0..=clear_number {
        assert_eq!(
            broker
                .received_on(&format!("test/color-selection/{number}"))
                .len(),
            1,
            "color selection received a later duplicate completion"
        );
    }
    let reopened = crate::config_store::ConfigStore::open(
        fixture.paths.clone(),
        "Controller".into(),
        deadline(),
    )
    .await
    .unwrap();
    assert_eq!(reopened.snapshot().model.inventory().revision, 5);
    assert!(reopened.close(deadline()).await);
}

struct Hold {
    entered: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
    hooks: Hooks,
}

#[tokio::test]
async fn mqtt_recovery_after_a_canceled_save_returns_one_original_context_success() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let hold = Hold::new(Step::AfterRename(Stage::Mapping));
    let (mut service, handle) = fixture.service();
    service.hooks = hold.hooks.clone();
    let running = start(service, &handle, &broker).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    let client = handle.clone();
    let pending = tokio::spawn(async move {
        client
            .edit(
                1,
                MetadataEdit::Description(Some("saved".into())),
                context(1),
                deadline(),
            )
            .await
    });
    hold.entered().await;
    pending.abort();
    let _ = pending.await;
    wait_for(&handle, |s| matches!(s.phase, Phase::Uncertain(_))).await;
    hold.release.store(true, Ordering::SeqCst);
    // Recovery may initially refuse while the previous disk owner is still
    // finishing. Each explicit attempt has its own response topic/correlation.
    let until = deadline();
    let mut attempt = 0;
    let successful = loop {
        assert!(
            Instant::now() < until,
            "MQTT recovery never activated durable mapping"
        );
        let topic = format!("test/recover/{attempt}");
        attempt += 1;
        assert!(broker.publish(
            "DALI/Command/Controller",
            r#"{"command":"recover"}"#,
            &SendOptions::default()
                .user_property("target-generation", context(1).generation().unwrap())
                .response_topic(&topic)
                .correlation_data("recovery")
        ));
        assert!(
            broker
                .wait_until(Duration::from_secs(1), |b| !b
                    .received_on(&topic)
                    .is_empty())
                .await
        );
        let replies = broker.received_on(&topic);
        assert_eq!(replies.len(), 1);
        let reply: serde_json::Value = serde_json::from_slice(&replies[0].payload).unwrap();
        if reply.get("error").is_none() {
            assert_eq!(
                reply["value"]["revision"], 2,
                "MQTT recovery returned wrong committed revision"
            );
            assert_eq!(
                replies[0].user_property("target-generation"),
                context(1).generation()
            );
            assert_eq!(replies[0].correlation_data().unwrap().as_ref(), b"recovery");
            break topic;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    };
    assert_eq!(handle.snapshot().unwrap().phase, Phase::Ready);
    assert_eq!(
        handle
            .snapshot()
            .unwrap()
            .model
            .inventory()
            .description
            .as_deref(),
        Some("saved")
    );
    assert!(running.stop().await);
    assert_eq!(
        broker.received_on(&successful).len(),
        1,
        "MQTT recovery received a duplicate completion"
    );
}
impl Hold {
    fn new(at: Step) -> Self {
        let entered = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let (inside, resume) = (entered.clone(), release.clone());
        let hooks = Hooks {
            callback: Some(Arc::new(move |step| {
                if step == at && !inside.swap(true, Ordering::SeqCst) {
                    let limit = std::time::Instant::now() + Duration::from_secs(15);
                    while !resume.load(Ordering::SeqCst) {
                        assert!(
                            std::time::Instant::now() < limit,
                            "test held worker was not released"
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
        .expect("save hook not reached");
    }
}
impl Drop for Hold {
    fn drop(&mut self) {
        self.release.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn canceled_save_stays_frozen_until_recovery_retires_the_mapping_even_when_changed_is_false()
{
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let hold = Hold::new(Step::AfterRename(Stage::Mapping));
    let (mut service, handle) = fixture.service();
    service.hooks = hold.hooks.clone();
    let running = start(service, &handle, &broker).await;
    let client = handle.clone();
    let pending = tokio::spawn(async move {
        client
            .edit(
                1,
                MetadataEdit::RenameOutput {
                    bus: 0,
                    address: 0,
                    name: "Kitchen".into(),
                },
                context(1),
                deadline(),
            )
            .await
    });
    hold.entered().await;
    assert_eq!(handle.snapshot().unwrap().phase, Phase::Writing);
    assert!(
        handle
            .edit(1, MetadataEdit::Description(None), context(1), deadline())
            .await
            .is_err(),
        "save allowed another edit"
    );
    pending.abort();
    let _ = pending.await;
    wait_for(&handle, |s| matches!(s.phase, Phase::Uncertain(_))).await;
    hold.release.store(true, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert!(
        matches!(handle.snapshot().unwrap().phase, Phase::Uncertain(_)),
        "late filesystem completion reopened service routing"
    );
    let recovered = handle.recover(context(1), deadline()).await.unwrap();
    assert_eq!(
        recovered.model.inventory().revision,
        2,
        "recovery lost durable mapping"
    );
    assert_eq!(
        recovered.model.inventory().buses[0].outputs[0].name,
        "Kitchen",
        "changed:false recovery skipped runtime activation"
    );
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| b
                .received_on("DALI/State/Controller/__inventory")
                .iter()
                .any(
                    |m| m.user_property("target-generation") == context(2).generation()
                ))
            .await,
        "recovered inventory was not queued with new generation"
    );
    assert!(
        handle
            .edit(
                2,
                MetadataEdit::RenameOutput {
                    bus: 0,
                    address: 0,
                    name: "Bad".into()
                },
                context(1),
                deadline()
            )
            .await
            .is_err(),
        "recovery left retired generation routable"
    );
    assert!(running.stop().await);
}

#[tokio::test]
async fn runtime_cas_failure_stays_frozen_and_recovery_never_adopts_a_foreign_authority() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let (mut service, handle) = fixture.service();
    let once = Arc::new(AtomicBool::new(false));
    service.activation_hook = Some(Arc::new(move |reporter, expected| {
        if !once.swap(true, Ordering::SeqCst) {
            let mut foreign = expected.clone();
            foreign.instance = "foreign".into();
            reporter.replace_generations(expected, foreign).unwrap();
        }
    }));
    let running = start(service, &handle, &broker).await;
    assert!(matches!(
        handle
            .edit(
                1,
                MetadataEdit::Description(Some("saved".into())),
                context(1),
                deadline()
            )
            .await,
        Err(ServiceError::Uncertain(_))
    ));
    assert_eq!(fixture.disk().revision, 2);
    assert_eq!(
        handle.snapshot().unwrap().model.inventory().revision,
        1,
        "failed runtime CAS exposed durable candidate"
    );
    assert!(
        matches!(
            handle.recover(context(1), deadline()).await,
            Err(ServiceError::Uncertain(_))
        ),
        "recovery adopted foreign runtime authority"
    );
    assert!(matches!(
        handle.snapshot().unwrap().phase,
        Phase::Uncertain(_)
    ));
    assert!(running.stop().await);
}

#[tokio::test]
async fn instance_mismatch_is_refused_without_changing_persisted_or_legacy_identity() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let (service, handle) = DaliService::new(fixture.paths.clone(), "Different".into());
    assert!(Bridge::new("DALI")
        .instance("Different")
        .broker(&broker.address())
        .start_managed(service)
        .await
        .is_err());
    assert_eq!(fixture.disk().name, "Controller");
    assert!(handle.permit.is_closed());
}

#[tokio::test]
async fn reservation_failure_restores_only_the_prior_mapping_and_sync_failure_requires_recovery() {
    for at in [
        Step::BeforeRename(Stage::Reservation),
        Step::DirectorySync(Stage::Mapping),
    ] {
        let fixture = Fixture::new();
        let broker = FakeBroker::start().await;
        let (mut service, handle) = fixture.service();
        let once = Arc::new(AtomicBool::new(false));
        service.hooks = Hooks {
            callback: Some(Arc::new(move |step| {
                if step == at && !once.swap(true, Ordering::SeqCst) {
                    Err("injected failure".into())
                } else {
                    Ok(())
                }
            })),
        };
        let running = start(service, &handle, &broker).await;
        let result = handle
            .edit(
                1,
                MetadataEdit::RenameOutput {
                    bus: 0,
                    address: 0,
                    name: "Kitchen".into(),
                },
                context(1),
                deadline(),
            )
            .await;
        if at == Step::BeforeRename(Stage::Reservation) {
            assert!(matches!(result, Err(ServiceError::Rejected(_))));
            assert_eq!(handle.snapshot().unwrap().phase, Phase::Ready);
            assert_eq!(fixture.disk().revision, 1);
        } else {
            assert!(matches!(result, Err(ServiceError::Uncertain(_))));
            assert!(
                matches!(handle.snapshot().unwrap().phase, Phase::Uncertain(_)),
                "failed sync admitted routing"
            );
            assert_eq!(handle.snapshot().unwrap().model.inventory().revision, 1);
            assert_eq!(
                handle
                    .recover(context(1), deadline())
                    .await
                    .unwrap()
                    .model
                    .inventory()
                    .revision,
                2
            );
        }
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn no_op_metadata_notifications_and_snapshot_reads_do_not_invent_versions() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(service, &handle, &broker).await;
    let mut updates = handle.subscribe();
    let no_op = handle
        .edit(
            1,
            MetadataEdit::RenameOutput {
                bus: 0,
                address: 0,
                name: "Light1".into(),
            },
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(no_op.model.inventory().revision, 1);
    for i in 0..10 {
        let revision = handle.snapshot().unwrap().model.inventory().revision;
        handle
            .edit(
                revision,
                MetadataEdit::Description(Some(format!("Description{i}"))),
                context(revision),
                deadline(),
            )
            .await
            .unwrap();
    }
    assert!(
        matches!(
            updates.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_))
        ),
        "updates did not expose bounded notification lag"
    );
    let snapshot = handle.snapshot().unwrap();
    assert_eq!(snapshot.model.inventory().revision, 11);
    assert_eq!(
        snapshot.model.inventory().description.as_deref(),
        Some("Description9")
    );
    assert_eq!(
        routing::generations(&snapshot.model).targets["Bus1.Outputs.Light1"],
        context(1).generation().unwrap()
    );
    assert!(running.stop().await);
}

#[tokio::test]
async fn managed_stop_does_not_claim_a_blocked_configuration_owner_has_ended() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    let hold = Hold::new(Step::AfterRename(Stage::Mapping));
    let (mut service, handle) = fixture.service();
    service.hooks = hold.hooks.clone();
    let running = start(service, &handle, &broker).await;
    let client = handle.clone();
    let pending = tokio::spawn(async move {
        client
            .edit(
                1,
                MetadataEdit::Description(Some("saved".into())),
                context(1),
                Instant::now() + Duration::from_secs(8),
            )
            .await
    });
    hold.entered().await;
    let stopped = running.stop().await;
    hold.release.store(true, Ordering::SeqCst);
    assert!(
        !stopped,
        "managed stop claimed a blocked configuration owner had ended"
    );
    assert!(matches!(handle.snapshot().unwrap().phase, Phase::Stopping));
    assert!(handle.permit.is_closed());
    let _ = pending.await;
    // Let actual runtime teardown release its writer before the fixture goes.
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(store) = crate::config_store::ConfigStore::open(
                fixture.paths.clone(),
                "Controller".into(),
                deadline(),
            )
            .await
            {
                assert!(store.close(deadline()).await);
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
