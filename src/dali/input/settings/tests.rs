use super::*;

#[test]
fn input_setting_plans_have_literal_dtr_order_and_only_repeat_the_addressed_frame() {
    let address = Address::new(63).unwrap();
    let instance = Instance::new(31).unwrap();
    let setting = Setting::Filter(EventFilter::new(0xabcdef).unwrap());
    assert_eq!(
        plan(address, instance, setting),
        vec![
            Step {
                frame: 0xc130ef,
                repeat: false
            },
            Step {
                frame: 0xc131cd,
                repeat: false
            },
            Step {
                frame: 0xc132ab,
                repeat: false
            },
            Step {
                frame: 0x7f1f68,
                repeat: true
            },
        ],
        "filter plan changed width, byte order or repeat scope"
    );
    for (setting, expected) in [
        (Setting::Enabled(true), vec![0x010062]),
        (Setting::Enabled(false), vec![0x010063]),
        (
            Setting::Priority(EventPriority::new(2).unwrap()),
            vec![0xc13002, 0x010061],
        ),
        (
            Setting::Priority(EventPriority::new(5).unwrap()),
            vec![0xc13005, 0x010061],
        ),
        (
            Setting::Scheme(EventScheme::DeviceInstance),
            vec![0xc13002, 0x010067],
        ),
        (
            Setting::Group(GroupSlot::Primary, Some(Group::new(0).unwrap())),
            vec![0xc13000, 0x010064],
        ),
        (
            Setting::Group(GroupSlot::Group1, Some(Group::new(31).unwrap())),
            vec![0xc1301f, 0x010065],
        ),
        (
            Setting::Group(GroupSlot::Group2, None),
            vec![0xc130ff, 0x010066],
        ),
    ] {
        let actual = plan(Address::new(0).unwrap(), Instance::new(0).unwrap(), setting);
        assert_eq!(actual.iter().map(|s| s.frame).collect::<Vec<_>>(), expected);
        assert_eq!(
            actual.iter().map(|s| s.repeat).collect::<Vec<_>>(),
            (0..actual.len())
                .map(|i| i + 1 == actual.len())
                .collect::<Vec<_>>(),
            "setting lost required adjacent repetition"
        );
    }
}

#[test]
fn input_setting_values_reject_reserved_codes_without_losing_raw_filter_data() {
    for raw in 0..=255 {
        assert_eq!(
            EventScheme::new(raw).map(EventScheme::raw).ok(),
            (raw <= 4).then_some(raw)
        );
        assert_eq!(
            EventPriority::new(raw).map(EventPriority::raw).ok(),
            (2..=5).contains(&raw).then_some(raw),
            "reserved event priority was accepted"
        );
    }
    for raw in [0, 255, 0xff0000, 0xffffff] {
        assert_eq!(EventFilter::new(raw).unwrap().raw(), raw);
    }
    assert!(EventFilter::new(0x1000000).is_err());
    for (kind, mask) in [
        (InstanceType::Button, 255),
        (InstanceType::Absolute, 1),
        (InstanceType::Occupancy, 31),
        (InstanceType::Light, 1),
    ] {
        assert_eq!(common_filter_mask(kind), Some(mask));
        assert!(EventFilter::new(mask).unwrap().validate_for(kind).is_ok());
        assert!(
            EventFilter::new(mask + 1)
                .unwrap()
                .validate_for(kind)
                .is_err(),
            "reserved common filter bit was accepted"
        );
    }
    assert_eq!(common_filter_mask(InstanceType::Unknown(31)), None);
    assert!(matches!(
        EventFilter::new(0xffffff)
            .unwrap()
            .validate_for(InstanceType::Unknown(31)),
        Err(Error::UnsupportedType(31))
    ));
}

#[test]
fn input_setting_query_frames_keep_checked_address_and_instance_boundaries() {
    use super::super::InstanceQuery::*;
    for (query, opcode) in [
        (EventPriority, 0x84),
        (Enabled, 0x86),
        (PrimaryGroup, 0x88),
        (Group1, 0x89),
        (Group2, 0x8a),
        (EventScheme, 0x8b),
        (EventFilterLow, 0x90),
        (EventFilterMiddle, 0x91),
        (EventFilterHigh, 0x92),
    ] {
        assert_eq!(
            Address::new(0)
                .unwrap()
                .query_instance(Instance::new(0).unwrap(), query),
            0x010000 | opcode
        );
        assert_eq!(
            Address::new(63)
                .unwrap()
                .query_instance(Instance::new(31).unwrap(), query),
            0x7f1f00 | opcode
        );
    }
}
