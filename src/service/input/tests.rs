use super::*;
use crate::inventory::Edit;

fn model() -> Model {
    let mut inventory = super::super::tests::model().inventory().clone();
    inventory.buses[0].inputs = serde_json::from_value(json!([
        {"short_address":0,"name":"Panel","instances":[
            {"number":0,"name":"First","instance_type":1},
            {"number":1,"name":"Second","instance_type":1},
            {"number":2,"name":"Position","instance_type":2},
            {"number":3,"name":"Occupancy","instance_type":3},
            {"number":4,"name":"Light","instance_type":4},
            {"number":5,"name":"Unknown","instance_type":255}]}]))
    .unwrap();
    Model::new(inventory).unwrap()
}
fn stamp() -> Stamp {
    Stamp {
        session: 1,
        actor: 1,
        bus: 1,
        observed_bus: 1,
    }
}
fn router() -> Router {
    let mut router = Router::default();
    router.sync(&model());
    let cuts = router.pending(|_| Some(stamp()));
    assert_eq!(
        cuts.len(),
        6,
        "named instances did not await their receive boundary"
    );
    router.install(&cuts, 10, |_| Some(stamp()));
    router
}

#[test]
fn input_restoration_cut_never_reauthorizes_queued_notices_after_fresh_activity() {
    let mut inventory = model().inventory().clone();
    inventory.buses[0].inputs.push(serde_json::from_value(json!({"short_address":1,"name":"Other","instances":[{"number":0,"name":"Button","instance_type":1}]})).unwrap());
    let model = Model::new(inventory).unwrap();
    let mut router = Router::default();
    router.sync(&model);
    let cuts = router.pending(|_| Some(stamp()));
    router.install(&cuts, 10, |_| Some(stamp()));
    let target = "Bus1.Inputs.Panel.Instances.First";
    let other = "Bus1.Inputs.Other.Instances.Button";
    router.observe(0, 10, 0x008001, Some(stamp()));
    router.observe(0, 11, 0x028001, Some(stamp()));
    let old = router.take_notices().0;
    router.cut_for_restore(0, 0);
    let cuts = router.pending(|_| Some(stamp()));
    assert_eq!(cuts.len(), 6);
    router.install(&cuts, 20, |_| Some(stamp()));
    router.observe(0, 20, 0x008000, Some(stamp()));
    let fresh = router.take_notices().0;
    let mut snapshot = super::super::tests::snapshot(model, Phase::Ready);
    snapshot.inputs = router.observations.clone();
    snapshot.input_listeners = router.listeners(|_| Some(stamp()));
    assert!(fresh
        .iter()
        .find(|n| n.target == target)
        .unwrap()
        .current(&snapshot));
    assert!(
        old.iter()
            .find(|n| n.target == other)
            .unwrap()
            .current(&snapshot),
        "restoration cut revoked an unaffected input route"
    );
    assert!(
        !old.iter()
            .find(|n| n.target == target)
            .unwrap()
            .current(&snapshot),
        "fresh activity reauthorized a queued pre-restoration input action"
    );
}

#[test]
fn input_native_sequence_restarts_at_a_new_transport_boundary() {
    let mut router = router();
    let target = "Bus1.Inputs.Panel.Instances.First";
    router.observe(0, 100, 0x008001, Some(stamp()));
    assert_eq!(router.native_sequence(target), 100);
    let next = Stamp {
        session: 2,
        ..stamp()
    };
    let cuts = router.pending(|_| Some(next));
    router.install(&cuts, 1, |_| Some(next));
    assert_eq!(
        router.native_sequence(target),
        0,
        "new transport retained an old native sequence"
    );
    router.observe(0, 2, 0x008000, Some(next));
    let mut query = Observation {
        context: router.observations[target].context.clone(),
        evidence: Evidence::Query(readback::ReadValue {
            reported_type: Some(1),
            resolution: Some(protocol::Resolution::new(1).unwrap()),
            value: Some(
                protocol::InputValue::decode(protocol::Resolution::new(1).unwrap(), &[255])
                    .unwrap(),
            ),
            quality: readback::Quality::Current,
            reason: None,
        }),
        bus: 0,
        stamp: next,
        stale: false,
    };
    router.accept_query(target, 0, &mut query);
    assert!(
        matches!(
            query.evidence,
            Evidence::Event {
                payload: Payload::Button(ButtonAction::Released),
                ..
            }
        ),
        "old sequence epoch hid a newer release"
    );
}

#[test]
fn live_input_routes_exact_instances_and_preserves_ambiguous_sources() {
    let mut router = router();
    for raw in [0x000401, 0x828801, 0x860401, 0xC60401, 0x010080] {
        router.observe(0, 10, raw, Some(stamp()));
    }
    assert!(
        router.take_notices().0.is_empty(),
        "ambiguous source or command acquired a button target"
    );
    router.observe(0, 11, 0x008001, Some(stamp()));
    router.observe(0, 12, 0x008400, Some(stamp()));
    // A device/type source is unique for the saved module's one absolute input.
    router.observe(0, 13, 0x000A55, Some(stamp()));
    let (events, lost) = router.take_notices();
    assert_eq!(lost, 0);
    assert_eq!(events.len(), 3, "resolved native events were lost");
    assert_eq!(events[0].target, "Bus1.Inputs.Panel.Instances.First");
    assert_eq!(events[1].target, "Bus1.Inputs.Panel.Instances.Second");
    assert_eq!(events[2].target, "Bus1.Inputs.Panel.Instances.Position");
    assert!(events.iter().all(|e| e.context.traceparent().is_none()));
}

#[test]
fn live_input_actions_are_native_and_never_coalesced_into_one_snapshot() {
    let mut router = router();
    for (code, action) in [
        (0, "released"),
        (1, "pressed"),
        (2, "short_press"),
        (5, "double_press"),
        (9, "long_press_start"),
        (11, "long_press_repeat"),
        (12, "long_press_stop"),
        (14, "free"),
        (15, "stuck"),
    ] {
        router.observe(0, 10, 0x008000 + code, Some(stamp()));
        let (events, lost) = router.take_notices();
        assert_eq!(lost, 0);
        assert_eq!(events.len(), 1, "native action was lost or duplicated");
        assert_eq!(
            events[0].value()["action"],
            action,
            "native action meaning changed"
        );
    }
    router.observe(0, 11, 0x008001, Some(stamp()));
    router.observe(0, 12, 0x008000, Some(stamp()));
    let events = router.take_notices().0;
    assert_eq!(events.len(), 2, "press/release were coalesced");
    assert_eq!(events[0].value()["action"], "pressed");
    assert_eq!(events[1].value()["action"], "released");
}

#[test]
fn live_input_cutover_cannot_retoken_old_frames_or_stale_boundary_completions() {
    let mut router = Router::default();
    let first = model();
    router.sync(&first);
    let cuts = router.pending(|_| Some(stamp()));
    router.observe(0, 10, 0x008001, Some(stamp()));
    assert!(
        router.take_notices().0.is_empty(),
        "pending mapping admitted an input"
    );
    router.install(&cuts, 20, |_| Some(stamp()));
    router.observe(0, 19, 0x008001, Some(stamp()));
    assert!(
        router.take_notices().0.is_empty(),
        "pre-boundary partial/queued frame acquired a token"
    );
    router.observe(0, 20, 0x008001, Some(stamp()));
    let renamed = first
        .edit(
            1,
            &[Edit::RenameInputInstance {
                bus: 0,
                address: 0,
                instance: 0,
                name: "Door".into(),
            }],
        )
        .unwrap();
    router.sync(&renamed);
    router.install(&cuts, 10, |_| Some(stamp()));
    router.observe(0, 30, 0x008001, Some(stamp()));
    let old = router.take_notices().0;
    assert_eq!(
        old.len(),
        1,
        "retired mapping accepted an old boundary completion"
    );
    assert_eq!(old[0].target, "Bus1.Inputs.Panel.Instances.First");
    assert_eq!(
        old[0].context.generation(),
        Some("0123456789abcdef0123456789abcdef.1")
    );
    let fresh = router.pending(|_| Some(stamp()));
    assert_eq!(fresh.len(), 1, "unchanged mappings lost their boundary");
    router.install(&fresh, 31, |_| Some(stamp()));
    router.observe(0, 30, 0x008001, Some(stamp()));
    router.observe(0, 31, 0x008001, Some(stamp()));
    let events = router.take_notices().0;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].target, "Bus1.Inputs.Panel.Instances.Door");
    assert_eq!(
        events[0].context.generation(),
        Some("0123456789abcdef0123456789abcdef.2")
    );
    let back = renamed
        .edit(
            2,
            &[Edit::RenameInputInstance {
                bus: 0,
                address: 0,
                instance: 0,
                name: "First".into(),
            }],
        )
        .unwrap();
    router.sync(&back);
    router.install(&cuts, 10, |_| Some(stamp()));
    router.observe(0, 40, 0x008001, Some(stamp()));
    assert!(
        router.take_notices().0.is_empty(),
        "rename-back reused an old input boundary token"
    );
}

fn state(router: &Router, target: &str) -> PropertyMap {
    let mut snapshot = super::super::tests::snapshot(model(), Phase::Ready);
    snapshot.hardware = Hardware::Ready {
        version: crate::atx::wire::Version {
            hardware: "01".into(),
            firmware: "32".into(),
            variant: "04".into(),
        },
        buses: vec![super::super::BusHealth {
            bus: 0,
            status: 2,
            channels: 4,
        }],
    };
    snapshot.inputs = router.observations.clone();
    projections(&snapshot)
        .into_iter()
        .find(|s| s.state.target == target)
        .unwrap()
        .state
        .values
}

#[test]
fn live_input_state_is_nullable_and_unknown_data_cannot_preserve_pressed_state() {
    let mut router = router();
    let key = "Bus1.Inputs.Panel.Instances.First";
    let unseen = state(&router, key);
    assert_eq!(unseen["Quality"], "unobserved");
    assert_eq!(unseen["Available"], false);
    assert_eq!(
        unseen["Status"],
        Value::Null,
        "unseen button fabricated a release"
    );
    router.observe(0, 10, 0x008001, Some(stamp()));
    assert_eq!(state(&router, key)["Status"], true);
    router.observe(0, 11, 0x008002, Some(stamp()));
    let short = state(&router, key);
    assert_eq!(short["Available"], true);
    assert_eq!(short["Quality"], "partial");
    assert_eq!(
        short["Status"],
        Value::Null,
        "action-only event kept a stale press"
    );
    router.observe(0, 12, 0x008001, Some(stamp()));
    router.observe(0, 13, 0x008103, Some(stamp()));
    let unknown = state(&router, key);
    assert_eq!(unknown["Available"], false);
    assert_eq!(unknown["Quality"], "unsupported");
    assert_eq!(
        unknown["Status"],
        Value::Null,
        "unsupported button frame kept a stale press"
    );
    assert_eq!(unknown["EventInformation"], 259);
    router.observe(0, 14, 0x008001, Some(stamp()));
    router.invalidate(|_| None);
    let stale = state(&router, key);
    assert_eq!(stale["Quality"], "stale");
    assert_eq!(stale["Status"], Value::Null);
    let fresh_stamp = Stamp {
        session: 2,
        ..stamp()
    };
    router.observe(0, 15, 0x008001, Some(fresh_stamp));
    assert_eq!(
        state(&router, key)["Quality"],
        "stale",
        "new UART epoch bypassed input boundary"
    );
}

#[test]
fn live_input_sensor_state_preserves_raw_fields_without_invented_units() {
    let mut router = router();
    for raw in [0x008A01, 0x008C0F, 0x009309, 0x009555] {
        router.observe(0, 10, raw, Some(stamp()));
    }
    assert_eq!(
        state(&router, "Bus1.Inputs.Panel.Instances.Position")["PositionRaw"],
        513
    );
    let occupied = state(&router, "Bus1.Inputs.Panel.Instances.Occupancy");
    assert_eq!(occupied["Occupied"], true);
    assert_eq!(occupied["Movement"], true);
    let light = state(&router, "Bus1.Inputs.Panel.Instances.Light");
    assert_eq!(light["LightLevelRaw"], 777);
    assert!(
        !light.contains_key("Lux"),
        "relative event data invented lux units"
    );
    let unknown = state(&router, "Bus1.Inputs.Panel.Instances.Unknown");
    assert_eq!(unknown["EventInformation"], 341);
    assert_eq!(unknown["Quality"], "unsupported");
    let events = router.take_notices().0;
    assert_eq!(events.len(), 4);
    assert_eq!(
        events
            .iter()
            .find(|e| e.target.ends_with(".Position"))
            .unwrap()
            .value()["position_raw"],
        513
    );
    assert_eq!(
        events
            .iter()
            .find(|e| e.target.ends_with(".Light"))
            .unwrap()
            .value()["light_level_raw"],
        777
    );
    let occupancy = events
        .iter()
        .find(|e| e.target.ends_with(".Occupancy"))
        .unwrap()
        .value();
    assert_eq!(occupancy["occupied"], true);
    assert_eq!(occupancy["movement"], true);
    assert_eq!(occupancy["repeated"], true);
    assert_eq!(occupancy["sensor"], "movement");
    assert!(
        events.iter().all(|e| e.value().get("action").is_none()),
        "sensor invented a button action"
    );
}

#[test]
fn live_input_notice_batches_are_bounded_with_explicit_loss_count() {
    let mut router = router();
    for sequence in 10..140 {
        router.observe(0, sequence, 0x008001, Some(stamp()));
    }
    let (events, dropped) = router.take_notices();
    assert_eq!(events.len(), 128, "unbounded native input notice batch");
    assert_eq!(dropped, 2, "input notice loss was silent");
    assert_eq!(router.observations.len(), 1);
    assert_eq!(router.take_notices().1, 0);
}
