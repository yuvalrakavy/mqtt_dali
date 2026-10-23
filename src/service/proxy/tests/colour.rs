use super::*;
use crate::config::ColorRepresentation;

const TARGET: &str = "Bus1.Outputs.Light1";
const STATE: &str = "DALI/State/Controller/Bus1.Outputs.Light1";
const TRACE: &str = "00-11111111111111111111111111111111-2222222222222222-01";

#[tokio::test]
async fn mqtt_colour_selection_change_retires_old_family_intent_through_reconnect() {
    let (_fixture, emulator, broker, running, handle) =
        boot(ColorRepresentation::Temperature).await;
    desired(&broker, "/ColorTemperature", json!(2700));
    values(&broker, |v| {
        v["ColorAvailable"] == true && v["ColorTemperature"] == json!(1_000_000.0 / 370.0)
    })
    .await;
    let retained = SendOptions::default()
        .expiry(3600)
        .user_property("target-generation", context(2).generation().unwrap());
    broker.retain(
        "DALI/Desired/Controller/Bus1.Outputs.Light1/ColorTemperature",
        "3000",
        &retained,
    );
    handle
        .edit(
            2,
            MetadataEdit::OutputColorRepresentation {
                bus: 0,
                address: 0,
                representation: Some(ColorRepresentation::Xy),
            },
            context(2),
            crate::service::tests::deadline(),
        )
        .await
        .unwrap();
    desired(&broker, "/ColorTemperature", json!(3000)); // Retired generation.
    assert!(broker.publish(
        "DALI/Desired/Controller/Bus1.Outputs.Light1/ColorTemperature",
        "3000",
        &SendOptions::default()
            .user_property("target-generation", context(3).generation().unwrap())
    ));
    // A rejected request and a valid scalar request sent together can form one
    // invalid SDK batch. Observe rejection before testing the next valid request.
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on("DALI/Error/Controller")
                .iter()
                .any(|m| {
                    m.user_property("target-generation") == context(3).generation()
                        && serde_json::from_slice::<Value>(&m.payload).is_ok_and(|v| {
                            v["property"] == "ColorTemperature" && v["reason"] == "rejected"
                        })
                }))
            .await,
        "wrong-family color request did not report its rejection"
    );
    assert!(broker.publish(
        "DALI/Desired/Controller/Bus1.Outputs.Light1/ColorX",
        "0.5",
        &SendOptions::default()
            .user_property("target-generation", context(3).generation().unwrap())
    ));
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| b
                .received_on(STATE)
                .last()
                .is_some_and(|m| {
                    m.user_property("target-generation") == context(3).generation()
                        && serde_json::from_slice::<Value>(&m.payload).is_ok_and(|v| {
                            v["ColorAvailable"] == true
                                && v["ColorX"] == 0.5
                                && v["ColorTemperature"].is_null()
                        })
                }))
            .await,
        "old family desire poisoned the new selected-family scalar request; last={:?}; errors={:?}; sample={:?}",
        broker.received_on(STATE).last().map(|m| String::from_utf8_lossy(&m.payload).into_owned()),
        broker.received_on("DALI/Error/Controller").iter().map(|m| String::from_utf8_lossy(&m.payload).into_owned()).collect::<Vec<_>>(),
        handle.snapshot().unwrap().outputs.get(TARGET)
    );
    let connections = broker
        .events()
        .iter()
        .filter(|event| matches!(event, mqtt_test_broker::BrokerEvent::Connected))
        .count();
    broker.drop_connection();
    assert!(
        broker
            .wait_until(Duration::from_secs(8), |b| b
                .events()
                .iter()
                .filter(|event| matches!(event, mqtt_test_broker::BrokerEvent::Connected))
                .count()
                > connections)
            .await
    );
    assert!(refresh(&broker, "test/after-family-reconnect")
        .await
        .get("error")
        .is_none());
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    let actual = &physical["buses"][0]["outputs"][0]["colour_state"]["actual"];
    assert_eq!(
        actual["active"], "xy",
        "old retained family changed active representation after reconnect"
    );
    assert_eq!(
        actual["temperature"]["value"], 370,
        "retired or wrong-family intent changed native temperature"
    );
    assert_eq!(actual["xy"], json!([32768, 9029]));
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[test]
fn mqtt_colour_parser_rejects_whole_invalid_maps_and_accepts_scalar_subsets() {
    use ColorRepresentation::{Rgbwaf, Temperature, Xy};
    for (representation, payload) in [
        (None, json!({"ColorTemperature":3000})),
        (
            Some(Temperature),
            json!({"PowerState":false,"ColorRed":0.5}),
        ),
        (
            Some(Temperature),
            json!({"ColorTemperature":3000,"ColorX":0.3}),
        ),
        (Some(Temperature), json!({"ColorTemperature":0})),
        (Some(Temperature), json!({"ColorTemperature":1000001})),
        (Some(Temperature), json!({"ColorTemperature":null})),
        (Some(Temperature), json!({"ColorTemperature":"3000"})),
        (Some(Xy), json!({"PowerState":true,"ColorX":1})),
        (Some(Xy), json!({"ColorY":-0.1})),
        (Some(Rgbwaf), json!({"ColorGreen":true})),
        (Some(Rgbwaf), json!({"ColorRed":1.01})),
        (Some(Rgbwaf), json!({"ColorWhite":null})),
        (
            Some(Rgbwaf),
            json!({"PowerState":false,"Brightness":0.4,"ColorRed":1}),
        ),
        (
            Some(Temperature),
            json!({"Brightness":0.5,"ColorAvailable":true}),
        ),
    ] {
        assert!(
            super::super::state::intent(
                &serde_json::from_value(payload.clone()).unwrap(),
                representation
            )
            .is_err(),
            "invalid color map was partially accepted: {payload}"
        );
    }
    for (representation, payload) in [
        (Temperature, json!({"ColorTemperature":2700})),
        (Xy, json!({"ColorX":0})),
        (Xy, json!({"ColorY":65534.0/65536.0})),
        (Rgbwaf, json!({"ColorGreen":0,"ColorFree":1})),
    ] {
        assert!(
            matches!(
                super::super::state::intent(
                    &serde_json::from_value(payload).unwrap(),
                    Some(representation)
                )
                .unwrap(),
                output::WriteIntent::Colour(_)
            ),
            "valid scalar color subset failed or invented a base intent"
        );
    }
}

#[test]
fn mqtt_colour_projection_never_normalizes_inactive_stale_or_unknown_evidence() {
    use crate::dali::colour::{self as codec, ByteValue, Reply, ValueQuery, WordValue};
    use output::colour::{Observation, Quality, Value as Sample};
    let mut sample = Observation {
        dt8: Some(true),
        features: Some(Reply::Value(codec::Features::decode(0xc3))),
        status: Some(Reply::Value(codec::Status::decode(0x20))),
        gear_features: Some(Reply::Value(1)),
        control: Some(Reply::Value(0x80)),
        assignments: (1..=6)
            .map(|a| Reply::Value(codec::AssignedColour::decode(a)))
            .collect(),
        quality: Quality::Current,
        ..Observation::default()
    };
    sample.values.insert(
        ValueQuery::Temperature,
        Reply::Value(Sample::Word(WordValue::Known(250))),
    );
    sample.values.insert(
        ValueQuery::Warmest,
        Reply::Value(Sample::Word(WordValue::Known(400))),
    );
    sample.values.insert(
        ValueQuery::PhysicalWarmest,
        Reply::Value(Sample::Word(WordValue::Mask)),
    );
    let mut outer = output::Observation {
        context: context(2),
        actual_level: None,
        status: None,
        colour: Some(sample),
        device_types: vec![],
        quality: output::Quality::Current,
        reason: None,
        stamp: output::Stamp {
            session: 1,
            actor: 1,
            bus: 1,
            observed_bus: 1,
        },
        bus: 0,
    };
    let project = |observed: Option<&output::Observation>, selection, live| {
        let mut values = PropertyMap::from_iter([("Available".into(), Value::Bool(true))]);
        super::super::colour::project(&mut values, selection, observed, live);
        assert_eq!(
            values["Available"], true,
            "color availability changed base availability"
        );
        values
    };
    let current = project(Some(&outer), Some(ColorRepresentation::Temperature), true);
    assert_eq!(current["ColorTemperature"], 4000.0);
    assert_eq!(
        current["ColorCapabilities"]["temperature_limits"]["warmest_kelvin"],
        2500.0
    );
    assert!(
        current["ColorCapabilities"]["temperature_limits"]["physical_warmest_kelvin"].is_null()
    );
    let mut shuffled = outer.clone();
    shuffled.colour.as_mut().unwrap().assignments = [3, 1, 2, 0, 255, 42]
        .into_iter()
        .map(|a| Reply::Value(codec::AssignedColour::decode(a)))
        .collect();
    let assigned = project(
        Some(&shuffled),
        Some(ColorRepresentation::Temperature),
        true,
    );
    assert_eq!(
        assigned["ColorCapabilities"]["rgbwaf_assignments"],
        json!(["blue", "red", "green", null, null, null]),
        "physical color roles were inferred from slot order or unknown bytes"
    );
    for case in [
        "mismatch",
        "partial",
        "mode",
        "unknown_mode",
        "stale",
        "bus",
        "absent",
        "unconfigured",
        "invalid_features",
        "unsupported",
    ] {
        let mut changed = outer.clone();
        let color = changed.colour.as_mut().unwrap();
        let mut selection = Some(ColorRepresentation::Temperature);
        let mut live = true;
        let expected = match case {
            "mismatch" => {
                color.status = Some(Reply::Value(codec::Status::decode(0x10)));
                "representation_mismatch"
            }
            "partial" => {
                color.quality = Quality::Partial;
                color
                    .values
                    .insert(ValueQuery::Temperature, Reply::ReceiveCollision(Some(3)));
                "partial"
            }
            "mode" | "unknown_mode" => {
                selection = Some(ColorRepresentation::Rgbwaf);
                color.status = Some(Reply::Value(codec::Status::decode(0x80)));
                color.control = if case == "mode" {
                    Some(Reply::Value(0x40))
                } else {
                    Some(Reply::NoResponse)
                };
                color.values.insert(
                    ValueQuery::Red,
                    Reply::Value(Sample::Byte(ByteValue::Known(100))),
                );
                "unsupported_mode"
            }
            "stale" => {
                changed.quality = output::Quality::Stale;
                "stale"
            }
            "bus" => {
                live = false;
                "stale"
            }
            "absent" => {
                changed.colour = None;
                "unobserved"
            }
            "unconfigured" => {
                selection = None;
                "unconfigured"
            }
            "invalid_features" => {
                color.features = Some(Reply::Value(codec::Features::decode(0xff)));
                color.quality = Quality::Partial;
                "partial"
            }
            "unsupported" => {
                *color = Observation {
                    dt8: Some(false),
                    quality: Quality::Unsupported,
                    ..Observation::default()
                };
                "unsupported"
            }
            _ => unreachable!(),
        };
        let values = project(Some(&changed), selection, live);
        assert_eq!(
            values["ColorQuality"], expected,
            "incorrect color quality for {case}"
        );
        assert_eq!(values["ColorAvailable"], false);
        for field in [
            "ColorTemperature",
            "ColorX",
            "ColorY",
            "ColorRed",
            "ColorGreen",
            "ColorBlue",
            "ColorWhite",
            "ColorAmber",
            "ColorFree",
        ] {
            assert!(
                values[field].is_null(),
                "unusable {case} leaked normalized {field}"
            );
        }
        if matches!(case, "stale" | "bus" | "absent") {
            assert!(values["ColorCapabilities"].is_null());
        }
        if case == "invalid_features" {
            assert!(
                values["ColorCapabilities"]["supported_representations"].is_null(),
                "invalid feature counts fabricated supported formats"
            );
        }
        if case == "unsupported" {
            assert_eq!(
                values["ColorCapabilities"]["supported_representations"],
                json!([])
            );
        }
    }
    let sample = outer.colour.as_mut().unwrap();
    sample.features = Some(Reply::Value(codec::Features::decode(7)));
    sample.status = Some(Reply::Value(codec::Status::decode(0x40)));
    sample.quality = Quality::Partial;
    let values = project(Some(&outer), Some(ColorRepresentation::Temperature), true);
    assert_eq!(values["ActiveColorRepresentation"], "primary_n");
    assert_eq!(
        values["ColorCapabilities"]["supported_representations"],
        json!(["temperature", "xy", "primary_n"])
    );
}

#[tokio::test]
async fn mqtt_colour_invalid_combined_requests_do_not_mutate_base_or_colour() {
    let (_fixture, emulator, broker, running, _handle) =
        boot(ColorRepresentation::Temperature).await;
    for (index, payload) in [
        json!({"PowerState":false,"ColorTemperature":3000,"ColorRed":1}),
        json!({"PowerState":false,"ColorTemperature":0}),
        json!({"PowerState":false,"ColorTemperature":3000,"ColorY":0.3}),
    ]
    .into_iter()
    .enumerate()
    {
        desired(&broker, "", payload);
        assert!(refresh(&broker, &format!("test/invalid-color/{index}"))
            .await
            .get("error")
            .is_none());
    }
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        physical["buses"][0]["outputs"][0]["level"], 100,
        "invalid complete color request partially changed base power"
    );
    assert_eq!(
        physical["buses"][0]["outputs"][0]["colour_state"]["actual"]["temperature"]["value"], 250,
        "invalid complete color request partially changed color"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn mqtt_colour_bus_invalidation_clears_values_in_real_merged_state() {
    let (_fixture, emulator, broker, running, _handle) =
        boot(ColorRepresentation::Temperature).await;
    assert!(refresh(&broker, "test/before-color-bus-loss")
        .await
        .get("error")
        .is_none());
    values(&broker, |v| {
        v["ColorAvailable"] == true && v["ColorTemperature"] == 4000.0
    })
    .await;
    emulator
        .emulator
        .control(
            Control::BusPower {
                bus: 0,
                powered: false,
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    assert!(
        broker
            .wait_until(Duration::from_secs(2), |b| b
                .received_on(STATE)
                .last()
                .is_some_and(|m| {
                    serde_json::from_slice::<Value>(&m.payload).is_ok_and(|v| {
                        v["ColorQuality"] == "stale"
                            && v["ColorAvailable"] == false
                            && v["ColorTemperature"].is_null()
                            && v["ColorCapabilities"].is_null()
                    })
                }))
            .await,
        "stale MQTT color data survived merged State invalidation"
    );
    assert_eq!(
        broker
            .received_on(STATE)
            .last()
            .unwrap()
            .user_property("traceparent"),
        None
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

async fn boot(
    representation: ColorRepresentation,
) -> (
    Fixture,
    pty::PtyEmulator,
    FakeBroker,
    mqtt_bridge_kit::Running,
    ServiceHandle,
) {
    let fixture = Fixture::new();
    let mut scenario: Value =
        serde_json::from_str(include_str!("../../../../examples/emulator/colour-i4.json")).unwrap();
    scenario["buses"][0]["outputs"][0]["short_address"] = json!(0);
    scenario["buses"][0]["outputs"][0]["colour"]["auto_activation"] = json!(true);
    let emulator = pty::spawn(serde_json::from_value(scenario).unwrap())
        .await
        .unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(emulator.path.clone(), RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
            .await
    );
    assert!(
        broker
            .wait_for_subscription("DALI/Desired/Controller/+", Duration::from_secs(2))
            .await
    );
    assert!(
        broker
            .wait_for_subscription("DALI/Desired/Controller/+/+", Duration::from_secs(2))
            .await
    );
    handle
        .edit(
            1,
            MetadataEdit::OutputColorRepresentation {
                bus: 0,
                address: 0,
                representation: Some(representation),
            },
            context(1),
            crate::service::tests::deadline(),
        )
        .await
        .unwrap();
    (fixture, emulator, broker, running, handle)
}

async fn values(broker: &FakeBroker, predicate: impl Fn(&Value) -> bool) -> Value {
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| b
                .received_on(STATE)
                .last()
                .is_some_and(|r| {
                    serde_json::from_slice::<Value>(&r.payload).is_ok_and(|v| predicate(&v))
                }))
            .await,
        "selected-family MQTT color state never became current"
    );
    let messages = broker.received_on(STATE);
    assert_eq!(
        messages.last().unwrap().user_property("target-generation"),
        context(2).generation()
    );
    serde_json::from_slice(&messages.last().unwrap().payload).unwrap()
}

fn desired(broker: &FakeBroker, suffix: &str, payload: Value) {
    let topic = format!("DALI/Desired/Controller/{TARGET}{suffix}");
    assert!(broker.publish(
        &topic,
        payload.to_string(),
        &SendOptions::default()
            .user_property("target-generation", context(2).generation().unwrap())
            .user_property("traceparent", TRACE)
    ));
}

#[tokio::test]
async fn mqtt_colour_first_refresh_discovers_all_formats_and_combined_tc_reports_clamped_actual() {
    let (_fixture, emulator, broker, running, _handle) =
        boot(ColorRepresentation::Temperature).await;
    assert!(refresh(&broker, "test/colour-initial")
        .await
        .get("error")
        .is_none());
    let state = values(&broker, |v| {
        v["ColorAvailable"] == true && v["ColorTemperature"] == 4000.0
    })
    .await;
    assert_eq!(state["ColorRepresentation"], "temperature");
    assert_eq!(state["ActiveColorRepresentation"], "temperature");
    assert_eq!(
        state["ColorCapabilities"]["supported_representations"],
        json!(["temperature", "xy", "rgbwaf"]),
        "initial configured color read did not discover supported formats"
    );
    assert_eq!(state["ColorCapabilities"]["automatic_activation"], true);
    assert_eq!(
        state["ColorCapabilities"]["rgbwaf_assignments"],
        json!(["red", "green", "blue", "white", "amber", "free"])
    );
    assert_eq!(
        state["ColorCapabilities"]["temperature_limits"]["warmest_kelvin"],
        2500.0
    );
    desired(
        &broker,
        "",
        json!({"PowerState":true,"Brightness":0.5,"ColorTemperature":2000}),
    );
    let state = values(&broker, |v| {
        v["ColorAvailable"] == true && v["ColorTemperature"] == 2500.0 && v["Brightness"] == 0.5
    })
    .await;
    assert!(state["ColorX"].is_null());
    assert!(state["ColorRed"].is_null());
    assert_eq!(
        broker
            .received_on(STATE)
            .last()
            .unwrap()
            .user_property("traceparent"),
        Some(TRACE),
        "MQTT color actuals lost original write trace"
    );
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        physical["buses"][0]["outputs"][0]["colour_state"]["actual"]["temperature"]["value"], 400,
        "MQTT color State did not come from physical clamped Tc"
    );
    assert_eq!(
        physical["buses"][0]["outputs"][0]["level"], 127,
        "combined MQTT color request lost brightness"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn mqtt_colour_scalar_xy_preserves_other_coordinate_and_master_level() {
    let (_fixture, emulator, broker, running, _handle) = boot(ColorRepresentation::Xy).await;
    assert!(refresh(&broker, "test/xy-initial")
        .await
        .get("error")
        .is_none());
    let state = values(&broker, |v| v["ColorQuality"] == "representation_mismatch").await;
    assert_eq!(state["ColorAvailable"], false);
    desired(&broker, "/ColorX", json!(0.25));
    values(&broker, |v| {
        v["ColorAvailable"] == true && v["ColorX"] == 0.25 && v["ColorY"] == json!(9029.0 / 65536.0)
    })
    .await;
    desired(&broker, "/ColorY", json!(0.75));
    let state = values(&broker, |v| {
        v["ColorAvailable"] == true && v["ColorX"] == 0.25 && v["ColorY"] == 0.75
    })
    .await;
    assert!(state["ColorTemperature"].is_null());
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        physical["buses"][0]["outputs"][0]["colour_state"]["actual"]["xy"],
        json!([16384, 49152]),
        "scalar xy replaced an unspecified coordinate"
    );
    assert_eq!(
        physical["buses"][0]["outputs"][0]["level"], 100,
        "color-only xy changed master brightness"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}

#[tokio::test]
async fn mqtt_colour_scalar_rgb_zero_preserves_other_channels_and_combines_with_off() {
    let (_fixture, emulator, broker, running, _handle) = boot(ColorRepresentation::Rgbwaf).await;
    desired(&broker, "/ColorGreen", json!(0));
    values(&broker, |v| {
        v["ColorAvailable"] == true
            && v["ColorGreen"] == 0.0
            && v["ColorWhite"] == json!(4.0 / 254.0)
    })
    .await;
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        physical["buses"][0]["outputs"][0]["colour_state"]["actual"]["rgbwaf"],
        json!([1, 0, 3, 4, 5, 6]),
        "scalar RGB reset other channels or rejected zero"
    );
    assert_eq!(physical["buses"][0]["outputs"][0]["level"], 100);
    desired(&broker, "", json!({"PowerState":false,"ColorRed":1.0}));
    let state = values(&broker, |v| {
        v["ColorAvailable"] == true && v["ColorRed"] == 1.0 && v["PowerState"] == false
    })
    .await;
    assert!(state["ColorX"].is_null());
    let physical = emulator
        .emulator
        .control(Control::Snapshot, Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(
        physical["buses"][0]["outputs"][0]["colour_state"]["actual"]["rgbwaf"],
        json!([254, 0, 3, 4, 5, 6])
    );
    assert_eq!(
        physical["buses"][0]["outputs"][0]["level"], 0,
        "combined RGB/off was not applied physically"
    );
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
