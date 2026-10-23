use super::*;
#[test]
fn sensor_parameter_literal_maps_ranges_and_units() {
    use InstanceType::*;
    use Parameter::*;
    for (kind, p, set, query, value, millis) in [
        (Absolute, Report, 0x10, 0x1e, 255, Some(255000)),
        (Absolute, Deadtime, 0x11, 0x1d, 255, Some(12750)),
        (Occupancy, Hold, 0x21, 0x2d, 254, Some(2540000)),
        (Occupancy, Report, 0x22, 0x2e, 255, Some(255000)),
        (Occupancy, Deadtime, 0x23, 0x2c, 255, Some(12750)),
        (Light, Report, 0x30, 0x3e, 255, Some(255000)),
        (Light, Hysteresis, 0x31, 0x3f, 25, None),
        (Light, Deadtime, 0x32, 0x3d, 255, Some(12750)),
        (Light, HysteresisMinimum, 0x33, 0x3c, 255, None),
    ] {
        let v = Value::new(kind, p, value).unwrap();
        assert_eq!(v.raw(), value);
        assert_eq!(v.parameter(), p);
        assert_eq!(
            v.duration(),
            millis.map(Duration::from_millis),
            "sensor parameter duration units changed"
        );
        assert!(parameters(kind).unwrap().contains(&p));
        for (a, i, prefix) in [(0, 0, 0x010000), (63, 31, 0x7f1f00)] {
            let a = Address::new(a).unwrap();
            let i = Instance::new(i).unwrap();
            assert_eq!(
                p.query(kind, a, i).unwrap(),
                prefix | query,
                "sensor query opcode or scope changed"
            );
            assert_eq!(
                v.plan(a, i),
                [
                    Step {
                        frame: 0xc13000 | u32::from(value),
                        repeat: false
                    },
                    Step {
                        frame: prefix | set,
                        repeat: true
                    }
                ],
                "sensor setting opcode, scope or repeat changed"
            );
        }
    }
    assert_eq!(
        Value::new(Occupancy, Hold, 0).unwrap().duration(),
        Some(Duration::from_secs(1)),
        "hold zero must mean one second"
    );
    for kind in [Absolute, Occupancy, Light] {
        for p in [Report, Deadtime] {
            assert_eq!(
                Value::new(kind, p, 0).unwrap().duration(),
                Some(Duration::ZERO)
            );
        }
    }
}
#[test]
fn sensor_parameter_rejects_wrong_family_unknown_type_and_reserved_values() {
    use InstanceType::*;
    use Parameter::*;
    for kind in [Button, Unknown(31), Unknown(2)] {
        assert!(parameters(kind).is_err());
        for p in [Report, Deadtime, Hold, Hysteresis, HysteresisMinimum] {
            assert!(Value::new(kind, p, 0).is_err());
        }
    }
    for (kind, p) in [
        (Absolute, Hold),
        (Absolute, Hysteresis),
        (Occupancy, HysteresisMinimum),
        (Light, Hold),
    ] {
        assert!(
            Value::new(kind, p, 0).is_err(),
            "wrong sensor parameter family accepted"
        );
        assert!(p
            .query(kind, Address::new(0).unwrap(), Instance::new(0).unwrap())
            .is_err());
    }
    for raw in 0..=255 {
        assert_eq!(
            Value::new(Occupancy, Hold, raw).is_ok(),
            raw != 255,
            "unsupported hold MASK became writable"
        );
        assert_eq!(Value::new(Light, Hysteresis, raw).is_ok(), raw <= 25);
    }
}
