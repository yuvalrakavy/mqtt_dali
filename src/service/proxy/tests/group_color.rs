use super::*;
use crate::service::tests::deadline;
const TARGET: &str = "Bus1.Groups.All";
const STATE: &str = "DALI/State/Controller/Bus1.Groups.All";
const TRACE: &str = "00-11111111111111111111111111111111-2222222222222222-01";

#[test]
fn group_color_metadata_selection_prepares_existing_fenced_metadata_operation() {
    let snapshot = crate::service::tests::snapshot(crate::service::tests::model(), Phase::Ready);
    let call = CommandCall {
        command: "set_color_representation".into(),
        params: PropertyMap::from_iter([
            ("expected_revision".into(), json!(1)),
            ("color_representation".into(), json!("temperature")),
        ]),
    };
    assert!(
        PreparedCommand::new(&snapshot, Some("Bus1.Groups.All".into()), &call, context(1)).is_ok(),
        "group color representation command is rejected before metadata preparation"
    );
    assert!(
        PreparedCommand::new(&snapshot, Some("Bus1.Groups.All".into()), &call, context(2)).is_err(),
        "retired group authority prepared a representation change"
    );
}

async fn current(broker: &FakeBroker, predicate: impl Fn(&Value) -> bool) -> Value {
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| b
                .received_on(STATE)
                .last()
                .is_some_and(
                    |m| serde_json::from_slice::<Value>(&m.payload).is_ok_and(|v| predicate(&v))
                ))
            .await,
        "native group color Desired/readback did not reach actual MQTT State; states={:?}; errors={:?}", broker.received_on(STATE), broker.received_on("DALI/Error/Controller")
    );
    serde_json::from_slice(&broker.received_on(STATE).last().unwrap().payload).unwrap()
}

#[tokio::test]
async fn group_color_native_tcp_temperature_desired_reports_uniform_mixed_and_stale_actuals() {
    use crate::dali::colour::{Intent, Temperature};
    use crate::emulator::{scenario::Scenario, session::Emulator};
    let fixture = Fixture::new();
    fixture.add_output(1, "Light2");
    fixture.add_output(2, "Outside");
    let mut inventory = serde_json::to_value(fixture.disk()).unwrap();
    inventory["buses"][0]["groups"][0]["color_representation"] = json!("temperature");
    inventory["buses"][0]["groups"][0]["members"] = json!([0, 1]);
    fixture.write_inventory(&serde_json::from_value(inventory).unwrap());
    let saved = fixture.current_bytes();
    let gear = |id, address, level, groups, value| {
        json!({"id":id,"short_address":address,"level":level,"groups":groups,"device_type":8,
        "colour":{"active":"temperature","temperature":{"value":value,"coolest":200,"warmest":if address==1{400}else{500},"physical_coolest":150,"physical_warmest":550}}})
    };
    let scenario: Scenario = serde_json::from_value(
        json!({"version":1,"clock":"frozen","buses":[{"number":0,"outputs":[
        gear("first",0,80,1,300),gear("second",1,110,1,350),gear("outside",2,200,0,450)]}]}),
    )
    .unwrap();
    let (emulator, address) = Emulator::listen_tcp("127.0.0.1:0".parse().unwrap(), scenario)
        .await
        .unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_hat_tcp(address, RepeatPolicy::OneTerminal),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Desired/Controller/+/+", Duration::from_secs(2))
            .await
    );
    assert!(broker.send_with(
        &format!("DALI/Desired/Controller/{TARGET}"),
        r#"{"ColorTemperature":4000}"#,
        &SendOptions::default()
            .user_property("target-generation", context(1).generation().unwrap())
            .user_property("traceparent", TRACE)
    ));
    let state = current(&broker, |v| {
        v["ColorAvailable"] == true && v["ColorTemperature"].as_f64() == Some(4000.0)
    })
    .await;
    assert_eq!(state["ColorTemperatureMixed"], false);
    assert_eq!(state["ColorXMixed"], Value::Null);
    assert_eq!(
        state["ColorCapabilities"]["temperature_limits"]["warmest_kelvin"].as_f64(),
        Some(2500.0)
    );
    assert_eq!(state["Available"], true);
    assert_eq!(state["BrightnessMixed"], true);
    assert_eq!(state["Brightness"], Value::Null);
    let native = emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    let outputs = native["buses"][0]["outputs"].as_array().unwrap();
    assert_eq!(
        outputs
            .iter()
            .map(|o| o["level"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![80, 110, 200],
        "pure group color changed brightness"
    );
    assert_eq!(
        outputs[0]["colour_state"]["actual"]["temperature"]["value"],
        250
    );
    assert_eq!(
        outputs[1]["colour_state"]["actual"]["temperature"]["value"],
        250
    );
    assert_eq!(
        outputs[2]["colour_state"]["actual"]["temperature"]["value"], 450,
        "native color group affected a nonmember"
    );
    assert!(
        broker
            .received_on(STATE)
            .iter()
            .any(|m| m.user_property("traceparent") == Some(TRACE)
                && serde_json::from_slice::<Value>(&m.payload)
                    .is_ok_and(|v| v["ColorTemperature"].as_f64() == Some(4000.0))),
        "group color lost original request cause"
    );
    handle
        .apply_colour(
            "Bus1.Outputs.Light2",
            Intent::temperature(Temperature::new(300).unwrap()),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    let stale = current(&broker, |v| v["ColorQuality"] == "stale").await;
    assert_eq!(stale["ColorTemperature"], Value::Null);
    assert_eq!(stale["ColorTemperatureMixed"], Value::Null);
    handle
        .read_group_output(TARGET, context(1), deadline())
        .await
        .unwrap();
    let mixed = current(&broker, |v| {
        v["ColorAvailable"] == true && v["ColorTemperatureMixed"] == true
    })
    .await;
    assert_eq!(mixed["ColorTemperature"], Value::Null);
    assert_eq!(mixed["ActiveColorRepresentation"], "temperature");
    assert_eq!(mixed["ActiveColorRepresentationMixed"], false);
    let clamped = handle
        .set_group_color(
            TARGET,
            1,
            group_output::ColorIntent {
                color: Intent::temperature(Temperature::new(500).unwrap()),
                base: None,
            },
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(clamped.outcome, "sent");
    let actual = emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        actual["buses"][0]["outputs"][0]["colour_state"]["actual"]["temperature"]["value"],
        500
    );
    assert_eq!(
        actual["buses"][0]["outputs"][1]["colour_state"]["actual"]["temperature"]["value"], 400,
        "group color echoed intent instead of independently clamped actual"
    );
    let clamped = current(&broker, |v| {
        v["ColorAvailable"] == true && v["ColorTemperatureMixed"] == true
    })
    .await;
    assert_eq!(clamped["ColorTemperature"], Value::Null);
    assert_eq!(
        fixture.current_bytes(),
        saved,
        "group color wrote saved names or assignments"
    );
    assert!(running.stop().await);
    emulator.close().await.unwrap();
}

#[tokio::test]
async fn group_color_preflights_actual_members_and_never_partially_changes_power_or_color() {
    use crate::{
        dali::{
            colour::{Intent, Temperature},
            gear,
        },
        emulator::{scenario::Scenario, session::Emulator},
        service::group_output::ColorIntent,
    };
    for missing in [false, true] {
        let fixture = Fixture::new();
        fixture.add_output(1, "Light2");
        let mut inventory = serde_json::to_value(fixture.disk()).unwrap();
        inventory["buses"][0]["groups"][0]["color_representation"] = json!("temperature");
        // Light2 is deliberately absent from saved membership, but present in hardware.
        fixture.write_inventory(&serde_json::from_value(inventory).unwrap());
        let scenario:Scenario=serde_json::from_value(json!({"version":1,"clock":"frozen","buses":[{"number":0,"outputs":[
            {"id":"color","short_address":0,"level":80,"groups":1,"device_type":8,"colour":{"active":"temperature","temperature":{"value":300,"coolest":200,"warmest":500,"physical_coolest":150,"physical_warmest":550}}},
            {"id":"plain","short_address":if missing{None}else{Some(1)},"groups":1,"level":110}
        ]}]})).unwrap();
        let (emulator, address) = Emulator::listen_tcp("127.0.0.1:0".parse().unwrap(), scenario)
            .await
            .unwrap();
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_hat_tcp(address, RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let mut before = emulator
            .control(Control::Snapshot, Duration::from_secs(1))
            .await
            .unwrap();
        let report = handle
            .set_group_color(
                TARGET,
                1,
                ColorIntent {
                    color: Intent::temperature(Temperature::new(250).unwrap()),
                    base: Some(gear::Intent::new(Some(false), None).unwrap()),
                },
                context(1),
                deadline(),
            )
            .await
            .unwrap();
        assert_eq!(report.outcome, "unconfirmed");
        let mut after = emulator
            .control(Control::Snapshot, Duration::from_secs(1))
            .await
            .unwrap();
        // Queries legitimately change scratch DTRs and the last REPORT snapshot.
        for snapshot in [&mut before, &mut after] {
            for output in snapshot["buses"][0]["outputs"].as_array_mut().unwrap() {
                output.as_object_mut().unwrap().remove("dtr");
                if let Some(color) = output
                    .get_mut("colour_state")
                    .and_then(Value::as_object_mut)
                {
                    color.remove("report");
                }
            }
        }
        assert_eq!(
            after["buses"], before["buses"],
            "refused group preflight changed native output state"
        );
        let report = handle
            .read_group_color(TARGET, context(1), deadline())
            .await
            .unwrap();
        assert_eq!(report.operation, "read_group_color");
        let state = current(&broker, |v| {
            v["ColorQuality"] == if missing { "partial" } else { "unsupported" }
        })
        .await;
        assert_eq!(state["ColorAvailable"], false);
        assert_eq!(state["ColorTemperature"], Value::Null);
        assert_eq!(state["ColorTemperatureMixed"], Value::Null);
        if !missing {
            assert_eq!(
                state["ColorCapabilities"]["supported_representations"],
                json!([])
            );
            assert_eq!(state["MembershipMatches"], false);
        }
        assert!(running.stop().await);
        emulator.close().await.unwrap();
    }
}

#[test]
fn group_color_commands_are_strict_and_require_original_selected_authority() {
    let mut snapshot =
        crate::service::tests::snapshot(crate::service::tests::model(), Phase::Ready);
    let original = CommandCall {
        command: "read_group_color".into(),
        params: PropertyMap::new(),
    };
    assert!(PreparedCommand::new(&snapshot, Some(TARGET.into()), &original, context(1)).is_ok());
    let edit = crate::inventory::Edit::GroupColorRepresentation {
        bus: 0,
        address: 0,
        representation: Some(crate::config::ColorRepresentation::Temperature),
    };
    snapshot.model = snapshot.model.edit(1, &[edit]).unwrap().into();
    for (command, params, authority, valid) in [
        (
            "set_group_color",
            json!({"expected_revision":2,"values":{"ColorTemperature":4000}}),
            2,
            true,
        ),
        (
            "set_group_color",
            json!({"expected_revision":2,"values":{"ColorTemperature":4000}}),
            1,
            false,
        ),
        (
            "set_group_color",
            json!({"expected_revision":1,"values":{"ColorTemperature":4000}}),
            2,
            false,
        ),
        (
            "set_group_color",
            json!({"expected_revision":2,"values":{"Brightness":0.5}}),
            2,
            false,
        ),
        (
            "set_group_color",
            json!({"expected_revision":2,"values":{"ColorRed":1}}),
            2,
            false,
        ),
        (
            "set_group_color",
            json!({"expected_revision":2,"values":{"ColorTemperature":4000,"Available":true}}),
            2,
            false,
        ),
        (
            "set_group_output",
            json!({"expected_revision":2,"ColorTemperature":4000}),
            2,
            false,
        ),
        ("read_group_color", json!({"extra":true}), 2, false),
    ] {
        let call = CommandCall {
            command: command.into(),
            params: serde_json::from_value(params.clone()).unwrap(),
        };
        assert_eq!(
            PreparedCommand::new(&snapshot, Some(TARGET.into()), &call, context(authority)).is_ok(),
            valid,
            "{command}: {params}"
        );
    }
}

#[tokio::test]
async fn group_color_native_xy_and_rgbwaf_partial_commands_preserve_unrequested_values() {
    use crate::{
        dali::{
            colour::{Channel, Coordinate, Intent},
            gear,
        },
        emulator::{scenario::Scenario, session::Emulator},
        service::group_output::ColorIntent,
    };
    for rgb in [false, true] {
        let fixture = Fixture::new();
        fixture.add_output(1, "Light2");
        fixture.add_output(2, "Outside");
        let mut inventory = serde_json::to_value(fixture.disk()).unwrap();
        inventory["buses"][0]["groups"][0]["color_representation"] =
            json!(if rgb { "rgbwaf" } else { "xy" });
        fixture.write_inventory(&serde_json::from_value(inventory).unwrap());
        let gear = |id, address, groups, y, green| json!({"id":id,"short_address":address,"groups":groups,"level":90,"device_type":8,"colour":{"active":if rgb{"rgbwaf"}else{"xy"},"xy":[10000,y],"rgbwaf":[50,green,70,80]}});
        let scenario:Scenario=serde_json::from_value(json!({"version":1,"clock":"frozen","repeat":if rgb{"two_terminals"}else{"one_terminal"},"buses":[{"number":0,"outputs":[gear("first",0,1,20000,60),gear("second",1,1,30000,90),gear("outside",2,0,40000,100)]}]})).unwrap();
        let (emulator, address) = Emulator::listen_tcp("127.0.0.1:0".parse().unwrap(), scenario)
            .await
            .unwrap();
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_hat_tcp(
                address,
                if rgb {
                    RepeatPolicy::TwoTerminals
                } else {
                    RepeatPolicy::OneTerminal
                },
            ),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let intent = if rgb {
            Intent::rgbwaf([
                Some(Channel::new(127).unwrap()),
                None,
                None,
                None,
                None,
                None,
            ])
            .unwrap()
        } else {
            Intent::xy_parts(Some(Coordinate::new(32768).unwrap()), None).unwrap()
        };
        let report = handle
            .set_group_color(
                TARGET,
                1,
                ColorIntent {
                    color: intent,
                    base: Some(gear::Intent::new(Some(false), None).unwrap()),
                },
                context(1),
                deadline(),
            )
            .await
            .unwrap();
        assert_eq!(report.outcome, "sent", "{report:?}");
        let state = current(&broker, |v| v["ColorAvailable"] == true).await;
        assert_eq!(state["PowerState"], false);
        assert_eq!(state["MembershipMatches"], false);
        let (field, mixed) = if rgb {
            ("ColorRed", "ColorGreen")
        } else {
            ("ColorX", "ColorY")
        };
        assert_eq!(state[field].as_f64(), Some(0.5));
        assert_eq!(state[format!("{field}Mixed")], false);
        assert_eq!(state[mixed], Value::Null);
        assert_eq!(state[format!("{mixed}Mixed")], true);
        let actual = emulator
            .control(Control::Snapshot, Duration::from_secs(1))
            .await
            .unwrap();
        let outputs = actual["buses"][0]["outputs"].as_array().unwrap();
        for (index, output) in outputs.iter().enumerate() {
            assert_eq!(output["level"], if index < 2 { 0 } else { 90 });
            let color = &output["colour_state"]["actual"];
            assert_eq!(
                color["xy"][1],
                if index == 0 {
                    20000
                } else if index == 1 {
                    30000
                } else {
                    40000
                }
            );
            assert_eq!(
                color["rgbwaf"][1],
                if index == 0 {
                    60
                } else if index == 1 {
                    90
                } else {
                    100
                }
            );
            if rgb {
                assert_eq!(color["rgbwaf"][0], if index < 2 { 127 } else { 50 });
            } else {
                assert_eq!(color["xy"][0], if index < 2 { 32768 } else { 10000 });
            }
        }
        assert!(running.stop().await);
        emulator.close().await.unwrap();
    }
}
