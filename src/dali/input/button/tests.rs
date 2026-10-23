use super::*;

#[test]
fn button_timer_literal_plans_queries_and_units() {
    for (timer, raw, set, query, minimum, millis) in [
        (Timer::Short, 255, 0, 0x0a, Some(0x0b), 5100),
        (Timer::Double, 100, 1, 0x0c, Some(0x0d), 2000),
        (Timer::Repeat, 5, 2, 0x0e, None, 100),
        (Timer::Stuck, 255, 3, 0x0f, None, 255000),
    ] {
        let value = Value::new(timer, raw).unwrap();
        assert_eq!(value.timer(), timer);
        assert_eq!(value.raw(), raw);
        assert_eq!(
            value.duration(),
            Duration::from_millis(millis),
            "button timer unit is wrong"
        );
        for (address, instance, prefix) in [(0, 0, 0x010000), (63, 31, 0x7f1f00)] {
            let address = Address::new(address).unwrap();
            let instance = Instance::new(instance).unwrap();
            assert_eq!(
                value.plan(address, instance),
                [
                    Step {
                        frame: 0xc13000 | u32::from(raw),
                        repeat: false
                    },
                    Step {
                        frame: prefix | set,
                        repeat: true
                    },
                ],
                "button timer plan has wrong address/opcode/DTR/repeat"
            );
            assert_eq!(timer.query(address, instance), prefix | query);
            assert_eq!(
                timer.minimum_query(address, instance),
                minimum.map(|op| prefix | op)
            );
        }
    }
    assert_eq!(
        Value::new(Timer::Double, 0).unwrap().duration(),
        Duration::ZERO
    );
}

#[test]
fn button_timer_checks_static_ranges_and_device_specific_minima() {
    for timer in [Timer::Short, Timer::Double, Timer::Repeat, Timer::Stuck] {
        for raw in 0..=255 {
            let expected = match timer {
                Timer::Short => raw >= 10,
                Timer::Double => raw == 0 || (10..=100).contains(&raw),
                Timer::Repeat => (5..=100).contains(&raw),
                Timer::Stuck => raw >= 5,
            };
            assert_eq!(
                Value::new(timer, raw).is_ok(),
                expected,
                "button timer static range changed"
            );
        }
    }
    for (timer, raw, minimum) in [(Timer::Short, 25, 30), (Timer::Double, 10, 20)] {
        assert!(
            matches!(
                Value::new(timer, raw).unwrap().check_minimum(minimum),
                Err(Error::BelowMinimum { .. })
            ),
            "below-device-minimum timer was accepted"
        );
    }
    for (timer, raw, minimum) in [
        (Timer::Short, 255, 255),
        (Timer::Short, 30, 30),
        (Timer::Double, 100, 100),
        (Timer::Double, 0, 100),
    ] {
        assert!(Value::new(timer, raw)
            .unwrap()
            .check_minimum(minimum)
            .is_ok());
    }
    for (timer, raw, minimum) in [
        (Timer::Short, 25, 0),
        (Timer::Short, 25, 9),
        (Timer::Double, 0, 0),
        (Timer::Double, 0, 101),
        (Timer::Double, 0, 255),
    ] {
        assert!(
            matches!(
                Value::new(timer, raw).unwrap().check_minimum(minimum),
                Err(Error::Minimum { .. })
            ),
            "invalid minimum reply was accepted"
        );
    }
    for timer in [Timer::Repeat, Timer::Stuck] {
        assert_eq!(
            Value::new(timer, 5).unwrap().check_minimum(5),
            Err(Error::NoMinimum(timer))
        );
    }
}
