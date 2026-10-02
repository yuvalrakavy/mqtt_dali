//! The MQTT session under saturation (Store no-hang §14.3, mqtt_dali): the task that polls
//! rumqttc's event loop must never wait on rumqttc's request channel, which only polling drains.
//!
//! Every session here runs as `main` runs it: on a thread of its own (`spawn`), against a fake
//! broker on 127.0.0.1. A session that blocks synchronously holds only that thread, so each test's
//! own deadlines still run (finding C-34).

use std::time::{Duration, Instant};

use mqtt_test_broker::FakeBroker;
use tokio::sync::{oneshot, watch};

use super::{broker_host_port, spawn, Backlog, Bridge, Outage, OutageLog, GOODBYE_BOUND, HIGH_WATER, LOW_WATER, OUTAGE_WARN_AFTER};
use crate::config_payload::{BusConfig, BusStatus, Channel, DaliConfig};
use crate::dali_commands;
use crate::dali_emulator::DaliControllerEmulator;
use crate::dali_manager::{self, DaliBusResult, DaliController, BUS_LIMITS};
use crate::test_log::capture;
use crate::Config;
use tracing::Level;

const NAME: &str = "Saturation";

fn config_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!("mqtt-dali-{tag}-{}.json", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

fn one_bus() -> DaliConfig {
    let mut dali_config = DaliConfig::new(NAME);
    dali_config.buses.push(BusConfig::new(0, BusStatus::Active));
    dali_config
}

fn command_topic() -> String {
    format!("DALI/Controllers/{NAME}/Command")
}

fn status_topic() -> String {
    format!("DALI/Status/{NAME}")
}

fn config_topic() -> String {
    format!("DALI/Config/{NAME}")
}

/// The bridge's session on its own thread, through `spawn`, on the test's runtime.
struct Rig {
    stop: watch::Sender<bool>,
    done: oneshot::Receiver<super::Result<()>>,
}

impl Rig {
    /// `dali_config` describes the emulated bus; `emulated` may add lights the config lacks.
    fn start(broker: &FakeBroker, config_filename: String, dali_config: DaliConfig) -> Rig {
        Rig::start_emulating(broker, config_filename, dali_config, |_| {})
    }

    fn start_emulating(broker: &FakeBroker, config_filename: String, dali_config: DaliConfig, after: impl FnOnce(&mut DaliConfig)) -> Rig {
        Rig::start_at(broker.address(), config_filename, dali_config, after)
    }

    fn start_at(broker: String, config_filename: String, mut dali_config: DaliConfig, after: impl FnOnce(&mut DaliConfig)) -> Rig {
        let controller = DaliControllerEmulator::try_new(&mut dali_config).expect("the emulator");
        after(&mut dali_config);
        Rig::start_with(broker, config_filename, dali_config, controller)
    }

    fn start_with(broker: String, config_filename: String, dali_config: DaliConfig, controller: Box<dyn DaliController>) -> Rig {
        let (stop, stop_rx) = watch::channel(false);
        let bridge = Bridge { config: Config { config_filename }, controller, dali_config, broker };
        let done = spawn(tokio::runtime::Handle::current(), bridge, stop_rx).expect("the session thread");
        Rig { stop, done }
    }

    /// Where the session is, for a failure message.
    fn state(&mut self) -> String {
        match self.done.try_recv() {
            Ok(result) => format!("the session ended: {result:?}"),
            Err(oneshot::error::TryRecvError::Empty) => "the session is still running".to_owned(),
            Err(oneshot::error::TryRecvError::Closed) => "the session thread panicked".to_owned(),
        }
    }

    /// Asks the session to stop and waits for it, for at most `within`.
    async fn stop(&mut self, within: Duration) -> Result<super::Result<()>, String> {
        let _ = self.stop.send(true);
        match tokio::time::timeout(within, &mut self.done).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(_)) => Err("the session thread panicked".to_owned()),
            Err(_) => Err(format!("the session did not end within {within:?} of its stop")),
        }
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}

/// Runs `driver` for at most `within`, failing at once if the session ends meanwhile (a panic on
/// its thread, say) — a session that must outlive the driver.
async fn alive<T>(rig: &mut Rig, within: Duration, driver: impl std::future::Future<Output = T>) -> T {
    tokio::select! {
        result = tokio::time::timeout(within, driver) => {
            result.unwrap_or_else(|_| panic!("the driver did not finish within {within:?}"))
        }
        ended = &mut rig.done => match ended {
            Ok(result) => panic!("the session ended: {result:?}"),
            Err(_) => panic!("the session thread panicked"),
        },
    }
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

/// A session that ends with commands still queued (the connection failed under a backlog) drops
/// them: the episode must still close, or its WARN stands forever and the next session's backlog
/// starts from a stale depth (finding C-6).
#[test]
fn a_session_ending_with_commands_unread_closes_its_backlog_episode() {
    let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let backlog = std::sync::Arc::new(Backlog::default());
    let incoming = super::Incoming { rx, backlog: backlog.clone() };
    for _ in 0..HIGH_WATER + 5 {
        backlog.pushed();
    }
    assert!(backlog.high_since.lock().unwrap().is_some(), "not flagged at the high-water mark");
    drop(incoming);
    assert!(
        backlog.high_since.lock().unwrap().is_none(),
        "the session ended with its backlog WARN standing: no drained INFO will ever close it"
    );
    assert_eq!(
        backlog.depth.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "the commands the session dropped are still counted"
    );
}

/// A subscriber that holds the thread inside its first event until released, as a synchronous
/// log writer can (tracing-init's console and file writers are synchronous).
struct Stall {
    entered: std::sync::mpsc::Sender<()>,
    release: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
}

impl tracing::Subscriber for Stall {
    fn register_callsite(&self, _: &'static tracing::Metadata<'static>) -> tracing::subscriber::Interest {
        tracing::subscriber::Interest::always()
    }
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn max_level_hint(&self) -> Option<tracing::level_filters::LevelFilter> {
        Some(tracing::level_filters::LevelFilter::TRACE)
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, _: &tracing::Event<'_>) {
        let _ = self.entered.send(());
        let _ = self.release.lock().unwrap().recv_timeout(Duration::from_secs(10));
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

/// The backlog's WARN is logged after its lock is released: a log write that stalls must not
/// hold the pump's next push (finding C-10).
#[test]
fn the_backlog_logs_after_releasing_its_lock() {
    use std::sync::mpsc;
    let backlog = std::sync::Arc::new(Backlog::default());
    for _ in 0..HIGH_WATER - 1 {
        backlog.pushed();
    }
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let logger = {
        let backlog = backlog.clone();
        std::thread::spawn(move || {
            let stall = Stall { entered: entered_tx, release: std::sync::Mutex::new(release_rx) };
            // Crosses the high-water mark: its WARN stalls inside the subscriber.
            tracing::subscriber::with_default(stall, || backlog.pushed());
        })
    };
    entered_rx.recv_timeout(Duration::from_secs(10)).expect("the high-water WARN was never logged");
    let (done_tx, done_rx) = mpsc::channel();
    {
        let backlog = backlog.clone();
        std::thread::spawn(move || {
            backlog.pushed();
            let _ = done_tx.send(());
        });
    }
    let pushed = done_rx.recv_timeout(Duration::from_secs(2));
    let _ = release_tx.send(());
    let _ = logger.join();
    assert!(pushed.is_ok(), "a push waited on the backlog's lock while another thread was still logging under it");
}

/// The logging policy for an outage, across sessions (finding C-5, fleet class F2): INFO on its
/// first failed attempt, DEBUG for the later ones, one WARN once it has lasted OUTAGE_WARN_AFTER,
/// and an INFO with its length and attempts once a connection has held; the next outage warns
/// again.
#[test]
fn a_broker_outage_warns_once_and_reports_its_length_on_recovery() {
    let s = Duration::from_secs;
    let t0 = Instant::now();
    let t1 = t0 + s(100);
    let mut outage = Outage::default();
    let mut logged = Vec::new();
    let events = capture(|| {
        logged.push(outage.connected(t0));
        for at in [1, 11, 21, 31, 41] {
            logged.push(outage.failed(t0 + s(at), "refused"));
        }
        logged.push(outage.connected(t0 + s(50)));
        logged.push(outage.held());
        logged.push(outage.held());
        logged.push(outage.failed(t1, "reset"));
        logged.push(outage.failed(t1 + OUTAGE_WARN_AFTER, "reset"));
    });
    let mut logged = logged.into_iter();
    let mut next = || logged.next().expect("one result per call");
    assert_eq!(next(), OutageLog::Connected);
    assert_eq!(next(), OutageLog::Lost);
    assert_eq!(next(), OutageLog::Retry);
    assert_eq!(next(), OutageLog::Retry);
    assert_eq!(next(), OutageLog::Warned);
    assert_eq!(next(), OutageLog::Retry, "warned twice in one outage");
    assert_eq!(next(), OutageLog::Reconnected, "a ConnAck during the outage ended it");
    assert_eq!(next(), OutageLog::Recovered { down_for: s(49), attempts: 5 });
    assert_eq!(next(), OutageLog::Quiet, "recovered twice");
    assert_eq!(next(), OutageLog::Lost, "the next outage did not start with its INFO");
    assert_eq!(next(), OutageLog::Warned, "the next outage did not warn");

    let recovered: Vec<_> = events.iter().filter(|e| e.is(Level::INFO, "external_recovered")).collect();
    assert_eq!(recovered.len(), 1, "{events:#?}");
    assert_eq!(recovered[0].field("down_for_ms"), Some("49000"));
    assert_eq!(recovered[0].field("attempts"), Some("5"));
    let warned: Vec<_> = events.iter().filter(|e| e.is(Level::WARN, "external_failure")).collect();
    assert_eq!(warned.len(), 2, "{events:#?}");
    assert_eq!((warned[0].field("down_for_ms"), warned[0].field("attempts")), (Some("30000"), Some("4")));
    let retries = events.iter().filter(|e| e.level == Level::DEBUG && e.message.contains("reconnect failed")).count();
    assert_eq!(retries, 3, "the later attempts are not DEBUG: {events:#?}");
}

/// A link the broker accepts and then drops, again and again (two bridges with one client id take
/// the session from each other), is one outage (fleet class F2): one INFO `connection_lost`, DEBUG
/// for every later failure, and ONE WARN once it has lasted 30 s, with `attempts` and
/// `down_for_ms`. A ConnAck alone proves nothing, so it ends nothing.
#[test]
fn a_link_that_connects_and_drops_is_one_outage_that_warns() {
    let t0 = Instant::now();
    let events = capture(|| {
        let mut outage = Outage::default();
        // The first connection: no outage.
        outage.connected(t0);
        // Dropped at 1 s, accepted again a second later, dropped ten seconds on: for a minute.
        for cycle in 0..6u64 {
            let at = t0 + Duration::from_secs(1 + cycle * 10);
            outage.failed(at, "the session was taken over");
            outage.connected(at + Duration::from_secs(1));
        }
    });
    let recovered = events.iter().filter(|e| e.kind() == Some("external_recovered")).count();
    assert_eq!(recovered, 0, "a bare ConnAck ended the outage {recovered} times: {events:#?}");
    let lost = events.iter().filter(|e| e.is(Level::INFO, "connection_lost")).count();
    assert_eq!(lost, 1, "one outage logged connection_lost at INFO {lost} times: {events:#?}");
    let warned: Vec<_> = events.iter().filter(|e| e.is(Level::WARN, "external_failure")).collect();
    assert_eq!(warned.len(), 1, "not one WARN in a minute of failures: {events:#?}");
    assert_eq!(warned[0].field("attempts"), Some("4"), "the WARN does not count the attempts: {:?}", warned[0]);
    assert_eq!(warned[0].field("down_for_ms"), Some("30000"), "the WARN came at the wrong time: {:?}", warned[0]);
    let chatter: Vec<_> = events.iter().filter(|e| e.level == Level::INFO && e.kind().is_none() && e.message.contains("connected")).collect();
    assert_eq!(chatter.len(), 1, "a ConnAck during the outage logged at INFO: {chatter:#?}");
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
    let mut rig = Rig::start(&broker, config_path("burst"), one_bus());
    let driver = async {
        assert!(broker.wait_for_subscription(&command_topic(), Duration::from_secs(10)).await, "the session never subscribed");
        let initial = broker.received_on(&config_topic()).len();
        broker.hold_acks();
        for _ in 0..COMMANDS {
            assert!(broker.send(&command_topic(), r#"{"command":"UpdateBusStatus"}"#));
        }
        // Let the burst saturate: the session takes commands until its channel is full.
        tokio::time::sleep(Duration::from_secs(2)).await;
        broker.release_acks();
        let want = initial + COMMANDS;
        let done = broker.wait_until(Duration::from_secs(20), |b| b.received_on(&config_topic()).len() >= want).await;
        (done, broker.received_on(&config_topic()).len() - initial)
    };
    // The driver's own waits are bounded; this bounds the whole, broker calls included.
    let (done, arrived) = alive(&mut rig, Duration::from_secs(60), driver).await;
    assert!(
        done,
        "{arrived} of {COMMANDS} config publishes arrived after the broker recovered — the session's poller waited on its own \
         request channel, or the session is stuck ({})",
        rig.state()
    );
    let _ = std::fs::remove_file(config_path("burst"));
}

/// Active=true says the bridge is listening, so it goes out only after the command subscription
/// (fleet class F4). The broker takes one QoS 1 publish and withholds its ack, so rumqttc sends
/// nothing more (a SUBSCRIBE does not count against that limit): whatever the bridge queued
/// before its first QoS 1 publish is all the broker sees. In the old order that publish was
/// Active=true, with no subscription behind it.
#[tokio::test(flavor = "multi_thread")]
async fn active_is_announced_only_after_the_command_subscription() {
    let broker = FakeBroker::start_with_receive_max(1).await;
    broker.hold_acks();
    let mut rig = Rig::start(&broker, config_path("order"), one_bus());
    let active = format!("DALI/Active/{NAME}");
    let driver = async {
        let published = broker.wait_until(Duration::from_secs(10), |b| !b.received().is_empty()).await;
        assert!(published, "the session published nothing");
        // A moment for anything else the broker would take (it takes nothing more).
        tokio::time::sleep(Duration::from_millis(300)).await;
        let first = (broker.subscriptions(), broker.received().into_iter().map(|r| r.topic).collect::<Vec<_>>());
        broker.release_acks();
        let announced = broker
            .wait_until(Duration::from_secs(10), |b| b.received_on(&active).iter().any(|r| r.payload.as_ref() == b"true" && r.retain))
            .await;
        (first, announced)
    };
    let ((subscribed, published), announced) = alive(&mut rig, Duration::from_secs(30), driver).await;
    assert!(
        subscribed.contains(&command_topic()),
        "the first QoS 1 publish went out before the command subscription: published {published:?}, subscribed {subscribed:?}"
    );
    assert!(!published.contains(&active), "Active went out before the rest of the model: published {published:?}");
    assert!(announced, "Active=true was never published: {:?}", broker.received_on(&active));
    let _ = std::fs::remove_file(config_path("order"));
}

/// A stop between commands: the session publishes a retained Active=false, disconnects, and ends
/// (finding C-7).
#[tokio::test(flavor = "multi_thread")]
async fn a_stop_says_goodbye_and_ends_the_session() {
    let broker = FakeBroker::start().await;
    let mut rig = Rig::start(&broker, config_path("stop"), one_bus());
    let active = format!("DALI/Active/{NAME}");
    assert!(broker.wait_for_subscription(&command_topic(), Duration::from_secs(10)).await, "the session never subscribed");
    let before = broker.received_on(&active).len();
    let ended = rig.stop(Duration::from_secs(5)).await;
    assert!(matches!(ended, Ok(Ok(()))), "the stop did not end the session cleanly: {ended:?}");
    let said = broker
        .wait_until(Duration::from_secs(5), |b| {
            b.received_on(&active)[before..].iter().any(|r| r.payload.as_ref() == b"false" && r.retain)
        })
        .await;
    assert!(said, "the session ended without a retained Active=false: {:?}", &broker.received_on(&active)[before..]);
    let _ = std::fs::remove_file(config_path("stop"));
}

/// A stop while the broker withholds its acks: the broker's receive window is full, so rumqttc's
/// event loop takes no request and the goodbye's DISCONNECT is never written. The session must
/// still end, within GOODBYE_BOUND (no-hang §14.3: shutdown is bounded as a whole).
#[tokio::test(flavor = "multi_thread")]
async fn a_stop_against_a_stalled_broker_is_still_bounded() {
    let broker = FakeBroker::start_with_receive_max(2).await;
    let mut rig = Rig::start(&broker, config_path("stalled-stop"), one_bus());
    assert!(broker.wait_for_subscription(&command_topic(), Duration::from_secs(10)).await, "the session never subscribed");
    broker.hold_acks();
    // The commands' QoS 1 publishes (a status, configs) fill the two-message receive window; the
    // rest wait in rumqttc's queue.
    for _ in 0..3 {
        assert!(broker.send(&command_topic(), r#"{"command":"UpdateBusStatus"}"#));
    }
    assert!(
        broker.wait_until(Duration::from_secs(10), |b| b.held_acks() >= 2).await,
        "the receive window never filled ({} acks held)",
        broker.held_acks()
    );
    // Let the session finish the commands and come back to wait for the next.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let start = Instant::now();
    let ended = rig.stop(GOODBYE_BOUND + Duration::from_secs(3)).await;
    assert!(matches!(ended, Ok(Ok(()))), "a stop against a stalled broker did not end the session: {ended:?}");
    assert!(start.elapsed() >= GOODBYE_BOUND / 2, "the goodbye was not waited for ({:?})", start.elapsed());
    let _ = std::fs::remove_file(config_path("stalled-stop"));
}

/// A stop while the broker is unreachable: `run` is waiting out its reconnect delay, and the stop
/// ends it at once rather than after the delay.
#[tokio::test(flavor = "multi_thread")]
async fn a_stop_during_a_broker_outage_ends_the_session_at_once() {
    // A port nothing listens on: every connection is refused.
    let port = std::net::TcpListener::bind("127.0.0.1:0").expect("a port").local_addr().expect("its address").port();
    let mut rig = Rig::start_at(format!("127.0.0.1:{port}"), config_path("outage"), one_bus(), |_| {});
    // Let the first connection fail, so `run` is in its reconnect delay.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let ended = rig.stop(Duration::from_secs(3)).await;
    assert!(matches!(ended, Ok(Ok(()))), "a stop during an outage did not end the session promptly: {ended:?}");
    let _ = std::fs::remove_file(config_path("outage"));
}

/// The command succeeds and the config is published, but its file cannot be written: reported on
/// the status topic, never a panic of the session (finding C-35).
#[tokio::test(flavor = "multi_thread")]
async fn a_config_that_cannot_be_saved_is_reported_not_a_panic() {
    let broker = FakeBroker::start().await;
    let missing = std::env::temp_dir().join(format!("mqtt-dali-missing-{}", std::process::id()));
    let mut rig = Rig::start(&broker, missing.join("dali.json").to_string_lossy().into_owned(), one_bus());
    let driver = async {
        assert!(broker.wait_for_subscription(&command_topic(), Duration::from_secs(10)).await, "the session never subscribed");
        assert!(broker.send(&command_topic(), r#"{"command":"RenameBus","bus":0,"name":"Kitchen"}"#));
        broker
            .wait_until(Duration::from_secs(10), |b| {
                b.received_on(&status_topic()).iter().any(|r| String::from_utf8_lossy(&r.payload).contains("saving the configuration"))
            })
            .await
    };
    let reported = alive(&mut rig, Duration::from_secs(30), driver).await;
    assert!(reported, "the failed save was not reported on {}: {:?}", status_topic(), broker.received_on(&status_topic()));
    assert_eq!(rig.state(), "the session is still running");
}

/// A FIFO at a temp path, standing in for a file on a filesystem that stopped answering: opening
/// it to write waits until someone opens it to read. Dropped, it is opened to read (without
/// waiting), which releases a writer stuck in its open, and removed.
struct Fifo(std::path::PathBuf);

impl Fifo {
    fn new(tag: &str) -> Fifo {
        let path = std::env::temp_dir().join(format!("mqtt-dali-{tag}-{}.fifo", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let made = std::process::Command::new("mkfifo").arg(&path).status().expect("mkfifo");
        assert!(made.success(), "mkfifo {path:?} failed");
        Fifo(path)
    }

    fn path(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl Drop for Fifo {
    fn drop(&mut self) {
        use std::os::unix::fs::OpenOptionsExt;
        #[cfg(target_os = "linux")]
        const O_NONBLOCK: i32 = 0o4000;
        #[cfg(not(target_os = "linux"))]
        const O_NONBLOCK: i32 = 0x0004;
        let reader = std::fs::OpenOptions::new().read(true).custom_flags(O_NONBLOCK).open(&self.0);
        std::thread::sleep(Duration::from_millis(100));
        drop(reader);
        let _ = std::fs::remove_file(&self.0);
    }
}

/// The configuration file is written after every command that changes the config. A write that
/// stalls (a filesystem that stopped answering; here a FIFO nobody reads) must not hold the
/// session: the failed save is reported on the status topic within its bound, and the next command
/// is handled (fleet class F1: no unbounded wait on the filesystem on a path everything waits on).
/// Synchronous in the session, the write held every later command while the pump kept Active true.
#[tokio::test(flavor = "multi_thread")]
async fn a_config_save_that_stalls_does_not_hold_the_session() {
    let broker = FakeBroker::start().await;
    let fifo = Fifo::new("stalled-save");
    let mut dali_config = one_bus();
    dali_config.buses[0].channels.push(Channel { short_address: 1, description: "Light 1".into() });
    let mut rig = Rig::start(&broker, fifo.path(), dali_config);
    let reply_topic = format!("DALI/Reply/QueryLightStatus/{NAME}/Bus_0/Address_1");
    let driver = async {
        assert!(broker.wait_for_subscription(&command_topic(), Duration::from_secs(10)).await, "the session never subscribed");
        assert!(broker.send(&command_topic(), r#"{"command":"RenameBus","bus":0,"name":"Kitchen"}"#));
        let reported = broker
            .wait_until(Duration::from_secs(10), |b| {
                b.received_on(&status_topic()).iter().any(|r| String::from_utf8_lossy(&r.payload).contains("saving the configuration"))
            })
            .await;
        assert!(broker.send(&command_topic(), r#"{"command":"QueryLightStatus","bus":0,"address":1}"#));
        let answered = broker.wait_until(Duration::from_secs(10), |b| !b.received_on(&reply_topic).is_empty()).await;
        // Another change while the first write is still stuck: refused at once, not a second
        // thread stuck behind the first. (Picked by content: the query's own "OK" status may
        // arrive after its reply.)
        let saves = |b: &FakeBroker| {
            b.received_on(&status_topic())
                .iter()
                .map(|r| String::from_utf8_lossy(&r.payload).into_owned())
                .filter(|status| status.contains("saving the configuration"))
                .collect::<Vec<_>>()
        };
        assert!(broker.send(&command_topic(), r#"{"command":"RenameBus","bus":0,"name":"Hall"}"#));
        broker.wait_until(Duration::from_secs(10), |b| saves(b).len() >= 2).await;
        let second = saves(&broker).get(1).cloned();
        (reported, answered, second)
    };
    let (reported, answered, second) = alive(&mut rig, Duration::from_secs(40), driver).await;
    assert!(
        reported,
        "a save into a stalled file was never reported: the session is stuck in the write ({}); status: {:?}",
        rig.state(),
        broker.received_on(&status_topic())
    );
    assert!(answered, "the session did not handle the next command after a save stalled");
    let second = second.unwrap_or_default();
    assert!(
        second.contains("an earlier write"),
        "a save behind a stuck one started another write instead of being refused: {second:?}"
    );
    assert_eq!(rig.state(), "the session is still running");
}

/// FindNewLights on a bus whose 64 short addresses are all taken finds a light it cannot address:
/// reported, never a panic of the session (finding C-35).
#[tokio::test(flavor = "multi_thread")]
async fn a_new_light_on_a_full_bus_is_reported_not_a_panic() {
    let broker = FakeBroker::start().await;
    let mut dali_config = DaliConfig::new(NAME);
    let mut bus = BusConfig::new(0, BusStatus::Active);
    for short_address in 0..64u8 {
        bus.channels.push(Channel { short_address, description: format!("Light {short_address}") });
    }
    // An unaddressed light the emulator puts on the bus beside them; the config does not list it.
    bus.channels.push(Channel { short_address: 0xff, description: "New".into() });
    dali_config.buses.push(bus);
    let mut rig = Rig::start_emulating(&broker, config_path("full"), dali_config, |c| c.buses[0].channels.retain(|c| c.short_address < 64));
    let driver = async {
        assert!(broker.wait_for_subscription(&command_topic(), Duration::from_secs(10)).await, "the session never subscribed");
        assert!(broker.send(&command_topic(), r#"{"command":"FindNewLights","bus":0}"#));
        broker
            .wait_until(Duration::from_secs(60), |b| {
                b.received_on(&status_topic()).iter().any(|r| String::from_utf8_lossy(&r.payload).contains("no free short address"))
            })
            .await
    };
    let reported = alive(&mut rig, Duration::from_secs(90), driver).await;
    assert!(reported, "the full bus was not reported on {}: {:?}", status_topic(), broker.received_on(&status_topic()));
    assert_eq!(rig.state(), "the session is still running");
    let _ = std::fs::remove_file(config_path("full"));
}

/// The emulated bus, except that something answers WITHDRAW (which expects no answer): a
/// misbehaving device, or line noise read as a reply. Past `cap` answers it lets WITHDRAW through
/// unanswered, so a regressed retry loop ends instead of spinning for the rest of the test run.
struct AnswersWithdraw {
    bus: Box<dyn DaliController>,
    answered: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    cap: usize,
}

impl DaliController for AnswersWithdraw {
    fn send_2_bytes(&mut self, bus: usize, b1: u8, b2: u8) -> dali_manager::Result<DaliBusResult> {
        let reply = self.bus.send_2_bytes(bus, b1, b2)?;
        if (b1, b2) == ((dali_commands::DALI_WITHDRAW & 0xff) as u8, 0)
            && self.answered.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < self.cap
        {
            return Ok(DaliBusResult::Value8(0xff));
        }
        Ok(reply)
    }

    fn send_2_bytes_repeat(&mut self, bus: usize, b1: u8, b2: u8) -> dali_manager::Result<DaliBusResult> {
        self.send_2_bytes(bus, b1, b2)
    }

    fn get_bus_status(&mut self, bus: usize) -> dali_manager::Result<BusStatus> {
        self.bus.get_bus_status(bus)
    }
}

/// FindNewLights on a bus where WITHDRAW keeps being answered: the command fails on the status
/// topic within its bound, and the session goes on to the next command. Unbounded, the WITHDRAW
/// loop held the session for ever while the pump kept Active true (finding Q1).
#[tokio::test(flavor = "multi_thread")]
async fn find_lights_on_a_bus_that_answers_withdraw_fails_and_the_session_goes_on() {
    let broker = FakeBroker::start().await;
    let mut dali_config = one_bus();
    // One unaddressed light for the emulator; the config does not list it.
    dali_config.buses[0].channels.push(Channel { short_address: 0xff, description: "New".into() });
    let controller = DaliControllerEmulator::try_new(&mut dali_config).expect("the emulator");
    dali_config.buses[0].channels.clear();
    let answered = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let controller = Box::new(AnswersWithdraw { bus: controller, answered: answered.clone(), cap: 200 });
    let mut rig = Rig::start_with(broker.address(), config_path("withdraw"), dali_config, controller);
    let driver = async {
        assert!(broker.wait_for_subscription(&command_topic(), Duration::from_secs(10)).await, "the session never subscribed");
        assert!(broker.send(&command_topic(), r#"{"command":"FindNewLights","bus":0}"#));
        // The command's outcome is its first status: an error, or "OK".
        let ended = broker.wait_until(Duration::from_secs(30), |b| !b.received_on(&status_topic()).is_empty()).await;
        assert!(ended, "FindNewLights never ended ({})", answered.load(std::sync::atomic::Ordering::SeqCst));
        let failed = broker.received_on(&status_topic()).iter().any(|r| String::from_utf8_lossy(&r.payload).contains("WITHDRAW"));
        let published = broker.received_on(&config_topic()).len();
        assert!(broker.send(&command_topic(), r#"{"command":"RenameBus","bus":0,"name":"Kitchen"}"#));
        let went_on = broker.wait_until(Duration::from_secs(10), |b| b.received_on(&config_topic()).len() > published).await;
        (failed, went_on)
    };
    let (failed, went_on) = alive(&mut rig, Duration::from_secs(60), driver).await;
    let answered = answered.load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        answered <= BUS_LIMITS.withdraw_attempts as usize,
        "WITHDRAW was answered {answered} times and sent again each time: its retry loop has no cap ({} allowed)",
        BUS_LIMITS.withdraw_attempts
    );
    assert!(failed, "FindNewLights was not reported failed on {}: {:?}", status_topic(), broker.received_on(&status_topic()));
    assert!(went_on, "the session did not handle the next command");
    let _ = std::fs::remove_file(config_path("withdraw"));
}

/// A light or group out of range is refused on the status topic, the session lives on, and the
/// config is left as it was: no group is created for a command that is refused (finding C-35).
#[tokio::test(flavor = "multi_thread")]
async fn a_group_out_of_range_leaves_the_config_alone() {
    let broker = FakeBroker::start().await;
    let mut dali_config = one_bus();
    dali_config.buses[0].channels.push(Channel { short_address: 1, description: "Light 1".into() });
    let mut rig = Rig::start(&broker, config_path("group"), dali_config);
    let driver = async {
        assert!(broker.wait_for_subscription(&command_topic(), Duration::from_secs(10)).await, "the session never subscribed");
        for command in [
            r#"{"command":"SetLightBrightness","bus":0,"address":64,"value":100}"#,
            r#"{"command":"SetGroupBrightness","bus":0,"group":16,"value":100}"#,
            r#"{"command":"AddToGroup","bus":0,"group":20,"address":1}"#,
            r#"{"command":"MatchGroup","bus":0,"group":17,"pattern":"Light"}"#,
        ] {
            let errors = broker.received_on(&status_topic()).len();
            assert!(broker.send(&command_topic(), command));
            let refused = broker.wait_until(Duration::from_secs(10), |b| b.received_on(&status_topic()).len() > errors).await;
            assert!(refused, "{command} was not refused on {}", status_topic());
        }
        let published = broker.received_on(&config_topic()).len();
        assert!(broker.send(&command_topic(), r#"{"command":"RenameBus","bus":0,"name":"Kitchen"}"#));
        assert!(
            broker.wait_until(Duration::from_secs(10), |b| b.received_on(&config_topic()).len() > published).await,
            "the config was not republished"
        );
        broker.received_on(&config_topic()).last().cloned().expect("a config")
    };
    let last = alive(&mut rig, Duration::from_secs(60), driver).await;
    let last: serde_json::Value = serde_json::from_slice(&last.payload).expect("the config's JSON");
    let groups = last["buses"][0]["groups"].clone();
    assert_eq!(groups, serde_json::json!([]), "a refused group was kept in the config: {groups}");
    assert_eq!(rig.state(), "the session is still running");
    let _ = std::fs::remove_file(config_path("group"));
}
