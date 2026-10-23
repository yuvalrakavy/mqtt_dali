//! Literal native oracle; no production codec or emulator constructors.
use super::*;

fn relay_probe(
    first: &str,
    next: &[&str],
    positive: bool,
    min: u8,
    features: &str,
    thresholds: [&str; 4],
) -> Vec<(String, String)> {
    let mut script = probe(min, 254, 7);
    script[2].1 = "JFE\n".into();
    script[3].1 = "JFE\n".into();
    script[9].1 = first.into();
    let closing = script.pop().unwrap();
    script.extend(next.iter().map(|reply| ("h01A7\n".into(), (*reply).into())));
    if positive {
        for (opcode, reply) in [
            ("F0", features),
            ("F1", "JFF\n"),
            ("F2", thresholds[0]),
            ("F3", thresholds[1]),
            ("F4", thresholds[2]),
            ("F5", thresholds[3]),
            ("F7", "JE2\n"),
            ("FF", "J01\n"),
        ] {
            script.push(("hC107\n".into(), "N\n".into()));
            script.push((format!("h01{opcode}\n"), reply.into()));
        }
    }
    script.push(closing);
    script
}
const DEFAULT: [&str; 4] = ["J01\n", "JFF\n", "JFF\n", "J00\n"];

#[tokio::test]
async fn managed_dt7_settings_literal_missing_physical_stays_partial() {
    let fixture = Fixture::new();
    let mut script = relay_probe("J07\n", &[], true, 254, "J08\n", DEFAULT);
    script[3].1 = "N\n".into();
    let peer = Peer::new(script);
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(&peer.path, RepeatPolicy::OneTerminal),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let r = handle
        .read_output_settings(TARGET, context(1), limit())
        .await
        .unwrap();
    let v = r.settings.unwrap();
    assert_eq!(
        v.quality, "partial",
        "DT7 virtual floor fabricated a complete missing-physical reading"
    );
    assert_eq!(v.physical_minimum, None);
    assert_eq!(v.power_on_level, Some(255));
    assert!(running.stop().await);
}

#[tokio::test]
async fn managed_dt7_settings_literal_type_prefixes_keep_malformed_unknown() {
    for (first, next, expected) in [
        ("J07\n", vec![], Some(true)),
        ("J06\n", vec![], Some(false)),
        ("JFE\n", vec![], Some(false)),
        ("N\n", vec![], None),
        ("X4\n", vec![], None),
        ("Z8\n", vec![], None),
        ("JFF\n", vec!["JFE\n"], None),
        ("JFF\n", vec!["J06\n", "JFE\n"], None),
        ("JFF\n", vec!["J00\n", "J06\n", "JFE\n"], Some(false)),
        ("JFF\n", vec!["J00\n", "J06\n", "J07\n"], Some(true)),
        ("JFF\n", vec!["J06\n", "J06\n"], None),
        ("JFF\n", vec!["J06\n", "J05\n"], None),
        ("JFF\n", vec!["J00\n", "N\n"], None),
        ("JFF\n", vec!["J08\n"], Some(false)),
        (
            "JFF\n",
            vec![
                "J00\n", "J01\n", "J02\n", "J03\n", "J04\n", "J05\n", "J06\n", "J07\n",
            ],
            Some(true),
        ),
    ] {
        let fixture = Fixture::new();
        let peer = Peer::new(relay_probe(
            first,
            &next,
            expected == Some(true),
            254,
            "J88\n",
            DEFAULT,
        ));
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(&peer.path, RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let r = handle
            .read_output_settings(TARGET, context(1), limit())
            .await
            .unwrap();
        let v = serde_json::to_value(r.settings.unwrap()).unwrap();
        assert_eq!(
            v["switching_support"],
            json!(expected),
            "DT7 type prefix evidence was misclassified: {first:?}/{next:?}"
        );
        assert_eq!(v.get("switching").is_some(), expected == Some(true));
        if expected == Some(true) {
            assert_eq!(v["switching"]["features_raw"], 136);
            assert_eq!(v["switching"]["physical_selection"], true);
            assert_eq!(v["switching"]["switch_status_raw"], 255);
            assert_eq!(v["switching"]["gear_type_raw"], 226);
        }
        assert!(writes(&peer).is_empty());
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn managed_dt7_settings_literal_whole_mixed_patch_requires_requested_support() {
    for mode in [
        "fixed",
        "missing_features",
        "missing_last",
        "invalid_upward",
        "unknown_type",
    ] {
        let fixture = Fixture::new();
        let mut thresholds = DEFAULT;
        let features = if mode == "fixed" {
            "J00\n"
        } else if mode == "missing_features" {
            "N\n"
        } else {
            "J08\n"
        };
        if mode == "missing_last" {
            thresholds[3] = "Z9\n";
        }
        if mode == "invalid_upward" {
            thresholds[0] = "J00\n";
        }
        let peer = Peer::new(relay_probe(
            if mode == "unknown_type" {
                "N\n"
            } else {
                "J07\n"
            },
            &[],
            mode != "unknown_type",
            254,
            features,
            thresholds,
        ));
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(&peer.path, RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let mut patch = serde_json::from_value::<Patch>(
            json!({"power_on_level":0,"switching":{"upward_on":10,"downward_off":5}}),
        )
        .unwrap();
        if mode == "missing_last" {
            patch.minimum_level = Some(1);
        }
        let r = handle
            .configure_output(TARGET, 1, patch, context(1), limit())
            .await;
        assert!(
            writes(&peer).is_empty(),
            "DT7 whole-patch preflight sent a partial native mutation: {mode}"
        );
        assert!(
            r.is_err() || r.as_ref().unwrap().outcome == "rejected",
            "DT7 preflight accepted missing requested support: {mode}: {r:?}"
        );
        assert!(running.stop().await);
    }
}

#[tokio::test]
async fn managed_dt7_settings_literal_exact_plans_and_requested_readback() {
    for mode in [
        "one",
        "two",
        "ignored",
        "lost_last",
        "lost_support",
        "partial_unrequested",
    ] {
        let fixture = Fixture::new();
        let mut initial = DEFAULT;
        if mode == "partial_unrequested" {
            initial[2] = "N\n";
        }
        let mut script = relay_probe("J07\n", &[], true, 254, "J08\n", initial);
        let native = [
            "hA301\n", "t012B\n", "hA337\n", "hC107\n", "t01E1\n", "hA3FF\n", "hC107\n", "t01E4\n",
        ];
        for cmd in native {
            script.push((
                cmd.into(),
                if mode == "two" && cmd.starts_with('t') {
                    "N\nN\n"
                } else {
                    "N\n"
                }
                .into(),
            ));
        }
        let mut after = [
            "J37\n",
            "JFF\n",
            if mode == "partial_unrequested" {
                "N\n"
            } else {
                "JFF\n"
            },
            "JFF\n",
        ];
        if mode == "ignored" {
            after[0] = "J01\n";
        }
        if mode == "lost_last" {
            after[3] = "N\n";
        }
        script.extend(relay_probe(
            "J07\n",
            &[],
            true,
            1,
            if mode == "lost_support" {
                "N\n"
            } else {
                "J08\n"
            },
            after,
        ));
        let peer = Peer::new(script);
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(
                &peer.path,
                if mode == "two" {
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
        let r = handle
            .configure_output(
                TARGET,
                1,
                serde_json::from_value(
                    json!({"minimum_level":1,"switching":{"upward_on":55,"downward_off":255}}),
                )
                .unwrap(),
                context(1),
                limit(),
            )
            .await
            .unwrap();
        assert_eq!(
            r.outcome,
            if ["ignored", "lost_last", "lost_support"].contains(&mode) {
                "unconfirmed"
            } else {
                "configured"
            },
            "DT7 exact native plans or requested readback failed: {mode}: {r:?}"
        );
        let actual = peer.commands.lock().unwrap().clone();
        assert!(
            native.iter().all(|c| actual.iter().any(|a| a == c)),
            "DT7 exact native setter plan was not executed"
        );
        assert!(running.stop().await);
    }
}
