use super::*;

#[test]
fn literal_unaddressed_input_queries_keep_device_and_instance_bytes() {
    assert_eq!(query_unaddressed(DeviceQuery::NumberOfInstances), 0xfdfe35);
    assert_eq!(query_unaddressed(DeviceQuery::Status), 0xfdfe30);
    assert_eq!(
        query_unaddressed_instance(Instance::new(0).unwrap(), InstanceQuery::Type),
        0xfd0080
    );
    assert_eq!(
        query_unaddressed_instance(Instance::new(31).unwrap(), InstanceQuery::Type),
        0xfd1f80
    );
    assert!(Instance::new(32).is_err());
}

fn event(raw: u32) -> Event {
    decode(raw)
        .expect("standard event frame rejected")
        .expect("standard input event was not decoded")
}

#[test]
fn all_standard_input_source_schemes_match_literal_frames() {
    let address = Address::new(1).unwrap();
    let instance = Instance::new(2).unwrap();
    let group = Group::new(3).unwrap();
    for (raw, source) in [
        (
            0x020401,
            Source::Device {
                address,
                kind: InstanceType::Button,
            },
        ),
        (0x028801, Source::DeviceInstance { address, instance }),
        (
            0x828801,
            Source::Instance {
                instance,
                kind: InstanceType::Button,
            },
        ),
        (
            0x860401,
            Source::DeviceGroup {
                group,
                kind: InstanceType::Button,
            },
        ),
        (
            0xc60401,
            Source::InstanceGroup {
                group,
                kind: InstanceType::Button,
            },
        ),
    ] {
        let decoded = event(raw);
        assert_eq!(
            decoded.source(),
            source,
            "input event source scheme/address bits changed"
        );
        assert_eq!(decoded.raw(), raw);
        assert_eq!(decoded.information(), 1);
    }
    let extreme = event(0x7effff);
    assert_eq!(
        extreme.source(),
        Source::DeviceInstance {
            address: Address::new(63).unwrap(),
            instance: Instance::new(31).unwrap()
        }
    );
    assert_eq!(
        extreme.information(),
        1023,
        "high event information bits were lost"
    );
}

#[test]
fn input_commands_and_reserved_sources_never_become_events() {
    for command in [0x010080, 0x030401, 0x7ffe35, 0xc130ff] {
        assert_eq!(
            decode(command).unwrap(),
            None,
            "a monitored input command became an event"
        );
    }
    assert_eq!(
        decode(0xc08001),
        Err(Error::SourceScheme(0xc08001)),
        "reserved source became a valid input action"
    );
    assert_eq!(
        decode(0x1000000),
        Err(Error::FrameWidth(0x1000000)),
        "over-width input frame was accepted"
    );
}

#[test]
fn native_button_actions_match_standard_codes_without_synthesized_gestures() {
    for (information, action) in [
        (0, ButtonAction::Released),
        (1, ButtonAction::Pressed),
        (2, ButtonAction::ShortPress),
        (5, ButtonAction::DoublePress),
        (9, ButtonAction::LongPressStart),
        (11, ButtonAction::LongPressRepeat),
        (12, ButtonAction::LongPressStop),
        (14, ButtonAction::Free),
        (15, ButtonAction::Stuck),
    ] {
        assert_eq!(
            event(0x020400 | information).payload(None).unwrap(),
            Payload::Button(action),
            "native button action code changed"
        );
    }
    for information in [3, 4, 6, 7, 8, 10, 13, 0x101, 0x201, 0x3ff] {
        assert_eq!(
            event(0x020400 | information).payload(None).unwrap(),
            Payload::Unknown {
                kind: InstanceType::Button,
                information: information as u16
            },
            "unknown/high button data was mistaken for a standard action"
        );
    }
}

#[test]
fn device_instance_type_requires_discovery_and_cannot_override_reported_type() {
    let unresolved = event(0x028802);
    assert_eq!(
        unresolved.payload(None).unwrap(),
        Payload::Unresolved,
        "device/instance event guessed a button type"
    );
    assert_eq!(unresolved.information(), 2);
    assert_eq!(
        unresolved.payload(Some(InstanceType::Button)).unwrap(),
        Payload::Button(ButtonAction::ShortPress)
    );
    assert_eq!(
        event(0x020401).payload(Some(InstanceType::Occupancy)),
        Err(Error::TypeMismatch {
            reported: 1,
            discovered: 3
        }),
        "discovered type overwrote an explicit event type"
    );
    assert_eq!(
        event(0x027c02).payload(None).unwrap(),
        Payload::Unknown {
            kind: InstanceType::Unknown(31),
            information: 2
        }
    );
}

#[test]
fn occupancy_flags_and_reserved_bits_preserve_their_native_meaning() {
    for (raw, occupied, movement, repeated, sensor) in [
        (0x040c00, false, false, false, SensorKind::Presence),
        (0x040c01, false, true, false, SensorKind::Presence),
        (0x040c02, true, false, false, SensorKind::Presence),
        (0x040c04, false, false, true, SensorKind::Presence),
        (0x040c08, false, false, false, SensorKind::Movement),
        (0x040c0f, true, true, true, SensorKind::Movement),
    ] {
        assert_eq!(
            event(raw).payload(None).unwrap(),
            Payload::Occupancy {
                occupied,
                movement,
                repeated,
                sensor,
            },
            "occupancy sensor flag changed"
        );
    }
    for information in [0x10, 0x100, 0x200, 0x3ff] {
        assert_eq!(
            event(0x040c00 | information).payload(None).unwrap(),
            Payload::Unknown {
                kind: InstanceType::Occupancy,
                information: information as u16,
            },
            "reserved occupancy data fabricated a valid sensor state"
        );
    }
}

#[test]
fn absolute_and_light_events_keep_full_raw_values_without_invented_units() {
    assert_eq!(
        event(0x0e0a01).payload(None).unwrap(),
        Payload::Absolute { raw_10: 513 },
        "absolute position lost high data bits"
    );
    assert_eq!(
        event(0x121309).payload(None).unwrap(),
        Payload::Light { raw_10: 777 },
        "relative light data was rescaled or truncated"
    );
    assert_eq!(
        event(0x0213ff).payload(None).unwrap(),
        Payload::Light { raw_10: 1023 }
    );
}

#[test]
fn individual_input_queries_match_literal_device_and_instance_frames() {
    for (query, expected) in [
        (DeviceQuery::Status, 0x01fe30),
        (DeviceQuery::Version, 0x01fe34),
        (DeviceQuery::NumberOfInstances, 0x01fe35),
    ] {
        assert_eq!(
            Address::new(0).unwrap().query(query),
            expected,
            "device query address/FE selector/opcode changed"
        );
    }
    assert_eq!(
        Address::new(63)
            .unwrap()
            .query(DeviceQuery::NumberOfInstances),
        0x7ffe35
    );
    for (query, expected) in [
        (InstanceQuery::Type, 0x010080),
        (InstanceQuery::Resolution, 0x010081),
        (InstanceQuery::Error, 0x010082),
        (InstanceQuery::Status, 0x010083),
        (InstanceQuery::Enabled, 0x010086),
        (InstanceQuery::EventScheme, 0x01008b),
        (InstanceQuery::InputValue, 0x01008c),
        (InstanceQuery::InputValueLatch, 0x01008d),
        (InstanceQuery::EventFilterLow, 0x010090),
        (InstanceQuery::EventFilterMiddle, 0x010091),
        (InstanceQuery::EventFilterHigh, 0x010092),
    ] {
        assert_eq!(
            Address::new(0)
                .unwrap()
                .query_instance(Instance::new(0).unwrap(), query),
            expected,
            "instance query selector/opcode changed"
        );
    }
    assert_eq!(
        Address::new(63)
            .unwrap()
            .query_instance(Instance::new(31).unwrap(), InstanceQuery::InputValue),
        0x7f1f8c
    );
}

#[test]
fn input_address_spaces_have_their_own_validated_bounds() {
    assert!(Address::new(64).is_err());
    assert!(Instance::new(32).is_err());
    assert!(Group::new(32).is_err());
    assert_eq!(Address::new(63).unwrap().raw(), 63);
    assert_eq!(Instance::new(31).unwrap().raw(), 31);
    assert_eq!(Group::new(31).unwrap().raw(), 31);
}
