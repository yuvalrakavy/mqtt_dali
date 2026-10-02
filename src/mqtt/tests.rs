//! The MQTT session under saturation (Store no-hang §14.3, mqtt_dali): the task that polls
//! rumqttc's event loop must never wait on rumqttc's request channel, which only polling drains.

use std::time::Duration;

use mqtt_test_broker::FakeBroker;

use super::{broker_host_port, Backlog, MqttDali, HIGH_WATER, LOW_WATER};
use crate::config_payload::{BusConfig, BusStatus, DaliConfig};
use crate::dali_emulator::DaliControllerEmulator;
use crate::dali_manager::DaliManager;
use crate::Config;

const NAME: &str = "Saturation";

fn config_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!("mqtt-dali-{tag}-{}.json", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

/// The owner's overload ruling (no-hang §14.6): the forward queue drops nothing; past HIGH_WATER
/// unread commands it raises its WARN once, and clears it back under LOW_WATER.
#[test]
fn a_backlog_past_high_water_is_flagged_once_and_cleared_when_it_drains() {
    let backlog = Backlog::default();
    for _ in 0..HIGH_WATER - 1 {
        backlog.pushed();
    }
    assert!(backlog.high_since.lock().unwrap().is_none(), "flagged below the high-water mark");
    backlog.pushed();
    let since = *backlog.high_since.lock().unwrap();
    assert!(since.is_some(), "not flagged at the high-water mark");
    backlog.pushed();
    assert_eq!(*backlog.high_since.lock().unwrap(), since, "flagged again while it stood");
    while backlog.depth.load(std::sync::atomic::Ordering::SeqCst) > LOW_WATER + 1 {
        backlog.popped();
    }
    assert!(backlog.high_since.lock().unwrap().is_some(), "cleared above the low-water mark");
    backlog.popped();
    assert!(backlog.high_since.lock().unwrap().is_none(), "not cleared at the low-water mark");
}

#[test]
fn a_broker_address_may_carry_its_port() {
    assert_eq!(broker_host_port("10.0.0.5"), ("10.0.0.5", 1883));
    assert_eq!(broker_host_port("127.0.0.1:41883"), ("127.0.0.1", 41883));
    assert_eq!(broker_host_port("broker.local:x"), ("broker.local:x", 1883));
}

/// A burst of commands, each republishing the config at QoS 1, while the broker withholds its
/// acknowledgements: rumqttc's request channel fills. The acks are then released, and every config
/// publish must arrive. A session whose poller awaits its own publishes never reads those acks —
/// it is stuck in the publish — so nothing more ever arrives.
#[tokio::test(flavor = "multi_thread")]
async fn a_command_burst_against_a_stalled_broker_completes_once_it_recovers() {
    const COMMANDS: usize = 300;
    let broker = FakeBroker::start_with_receive_max(2).await;
    let config = Config { config_filename: config_path("burst") };
    let mut dali_config = DaliConfig::new(NAME);
    dali_config.buses.push(BusConfig::new(0, BusStatus::Active));
    let mut controller = DaliControllerEmulator::try_new(&mut dali_config).expect("the emulator");
    let mut dali_manager = DaliManager::new(controller.as_mut());
    let config_topic = format!("DALI/Config/{NAME}");
    let command_topic = format!("DALI/Controllers/{NAME}/Command");
    let address = broker.address();

    let session = MqttDali::run(&config, &mut dali_manager, &mut dali_config, &address);
    let driver = async {
        assert!(broker.wait_for_subscription(&command_topic, Duration::from_secs(10)).await, "the session never subscribed");
        let initial = broker.received_on(&config_topic).len();
        broker.hold_acks();
        for _ in 0..COMMANDS {
            assert!(broker.send(&command_topic, r#"{"command":"UpdateBusStatus"}"#));
        }
        // Let the burst saturate: the session takes commands until its channel is full.
        tokio::time::sleep(Duration::from_secs(2)).await;
        broker.release_acks();
        let want = initial + COMMANDS;
        let done = broker.wait_until(Duration::from_secs(20), |b| b.received_on(&config_topic).len() >= want).await;
        assert!(
            done,
            "{} of {COMMANDS} config publishes arrived after the broker recovered — the session's poller waited on its own \
             request channel",
            broker.received_on(&config_topic).len() - initial
        );
    };
    tokio::select! {
        r = session => panic!("the session ended: {r:?}"),
        () = driver => {}
    }
    let _ = std::fs::remove_file(config_path("burst"));
}
