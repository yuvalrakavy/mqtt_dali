//! An address, group or bus from an MQTT command is checked before anything reaches the DALI bus,
//! and refused with an error, never a panic (finding C-35): the session runs these calls, so a
//! panic in one ends the bridge.

use std::sync::mpsc;
use std::time::Duration;

use super::*;
use crate::config_payload::{BusConfig, BusStatus, DaliConfig};
use crate::dali_emulator::DaliControllerEmulator;

/// Runs `f` on its own thread: a panic there, or no return within 10 s, is an `Err` naming it.
fn on_its_own_thread<T: Send + 'static>(
    what: &str,
    f: impl FnOnce() -> T + Send + 'static,
) -> std::result::Result<T, String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(Duration::from_secs(10)) {
        Ok(value) => Ok(value),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(format!("{what} did not return within 10 s")),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err(format!("{what} panicked instead of returning an error"))
        }
    }
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime")
        .block_on(future)
}

/// A bus that records what it was sent and answers nothing.
#[derive(Default)]
struct Recorder {
    sent: Vec<(usize, u8, u8)>,
}

impl DaliController for Recorder {
    fn send_2_bytes(&mut self, bus: usize, b1: u8, b2: u8) -> Result<DaliBusResult> {
        self.sent.push((bus, b1, b2));
        Ok(DaliBusResult::None)
    }

    fn send_2_bytes_repeat(&mut self, bus: usize, b1: u8, b2: u8) -> Result<DaliBusResult> {
        self.send_2_bytes(bus, b1, b2)
    }

    fn get_bus_status(&mut self, _bus: usize) -> Result<BusStatus> {
        Ok(BusStatus::Active)
    }
}

type Case = fn(&mut DaliManager) -> Result<DaliBusResult>;

#[test]
fn an_address_or_group_out_of_range_is_refused_before_the_bus() {
    let cases: [(&str, Case); 8] = [
        ("SetLightBrightness to address 64", |m| {
            block_on(m.set_light_brightness_async(0, 64, 100))
        }),
        ("SetGroupBrightness to group 16", |m| {
            block_on(m.set_group_brightness_async(0, 16, 100))
        }),
        ("set_light_brightness to address 200", |m| {
            m.set_light_brightness(0, 200, 100)
        }),
        ("set_group_brightness to group 255", |m| {
            m.set_group_brightness(0, 255, 100)
        }),
        // ADD_TO_GROUP0 + 16 is REMOVE_FROM_GROUP0: unchecked, it removed the light from group 0.
        ("AddToGroup group 16", |m| m.add_to_group(0, 16, 1)),
        ("RemoveFromGroup group 16", |m| {
            m.remove_from_group(0, 16, 1)
        }),
        ("AddToGroup group 20 (and verify)", |m| {
            m.add_to_group_and_verify(0, 20, 1)
        }),
        ("RemoveFromGroup group 31 (and verify)", |m| {
            m.remove_from_group_and_verify(0, 31, 1)
        }),
    ];
    let mut failures = Vec::new();
    for (what, case) in cases {
        let outcome = on_its_own_thread(what, move || {
            let mut bus = Recorder::default();
            let result = case(&mut DaliManager::new(&mut bus))
                .map(|_| ())
                .map_err(|e| e.to_string());
            (result, bus.sent)
        });
        match outcome {
            Err(failure) => failures.push(failure),
            Ok((Ok(()), sent)) => failures.push(format!(
                "{what} was accepted (sent {sent:?} to the DALI bus)"
            )),
            Ok((Err(_), sent)) if !sent.is_empty() => {
                failures.push(format!("{what} sent {sent:?} to the DALI bus"))
            }
            Ok((Err(_), _)) => {}
        }
    }
    assert!(
        failures.is_empty(),
        "out-of-range addresses were not refused:\n{}",
        failures.join("\n")
    );
}

/// Scheduling slack on a shared machine, past a bound the code itself enforces.
const SLACK: Duration = Duration::from_secs(1);

/// DALI's WITHDRAW, as `broadcast_command` sends it.
const WITHDRAW: (u8, u8) = ((dali_commands::DALI_WITHDRAW & 0xff) as u8, 0);

/// A bus on which something answers WITHDRAW, which expects no answer (a misbehaving device, or
/// line noise read as a reply), each send taking `delay`. Everything else gets no answer. Past
/// `cap` answers it fails the send instead, so that a regressed retry loop ends with an error
/// rather than spinning for the rest of the test run.
struct AnswersWithdraw {
    withdraws: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    delay: Duration,
    cap: usize,
}

impl AnswersWithdraw {
    fn new(delay: Duration) -> AnswersWithdraw {
        AnswersWithdraw { withdraws: Default::default(), delay, cap: 10_000 }
    }
}

impl DaliController for AnswersWithdraw {
    fn send_2_bytes(&mut self, _bus: usize, b1: u8, b2: u8) -> Result<DaliBusResult> {
        if (b1, b2) != WITHDRAW {
            return Ok(DaliBusResult::None);
        }
        let sent = self.withdraws.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        std::thread::sleep(self.delay);
        if sent > self.cap {
            return Err(Report::new(DaliManagerError::Context(
                "the test's bus stopped answering: WITHDRAW's retries ran away".to_owned(),
            )));
        }
        Ok(DaliBusResult::Value8(0xff))
    }

    fn send_2_bytes_repeat(&mut self, bus: usize, b1: u8, b2: u8) -> Result<DaliBusResult> {
        self.send_2_bytes(bus, b1, b2)
    }

    fn get_bus_status(&mut self, _bus: usize) -> Result<BusStatus> {
        Ok(BusStatus::Active)
    }
}

/// A bus that answers every send with a collision, each send taking `delay`.
struct Collides {
    sends: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    delay: Duration,
}

impl DaliController for Collides {
    fn send_2_bytes(&mut self, _bus: usize, _b1: u8, _b2: u8) -> Result<DaliBusResult> {
        self.sends.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        std::thread::sleep(self.delay);
        Ok(DaliBusResult::TransmitCollision)
    }

    fn send_2_bytes_repeat(&mut self, bus: usize, b1: u8, b2: u8) -> Result<DaliBusResult> {
        self.send_2_bytes(bus, b1, b2)
    }

    fn get_bus_status(&mut self, _bus: usize) -> Result<BusStatus> {
        Ok(BusStatus::Active)
    }
}

/// FindAllLights and FindNewLights program each light found, then WITHDRAW it, retrying while the
/// WITHDRAW is answered (it expects no answer). Something that keeps answering must end the
/// command with an error after a few tries, not hold the session for ever (finding Q1).
#[test]
fn a_withdraw_that_keeps_being_answered_fails_after_its_retry_cap() {
    let bus = AnswersWithdraw::new(Duration::ZERO);
    let withdraws = bus.withdraws.clone();
    let outcome = on_its_own_thread("program_short_address on a bus answering WITHDRAW", move || {
        let mut bus = bus;
        DaliManager::new(&mut bus)
            .program_short_address(0, 5)
            .map_err(|e| format!("{e:#}"))
    });
    let result = outcome.unwrap_or_else(|failure| panic!("{failure}"));
    let sent = withdraws.load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        sent <= BUS_LIMITS.withdraw_attempts as usize,
        "WITHDRAW was sent {sent} times while it was answered: its retry loop has no cap ({} allowed)",
        BUS_LIMITS.withdraw_attempts
    );
    let error =
        result.expect_err("programming succeeded although WITHDRAW was answered every time");
    assert!(error.contains("WITHDRAW"), "the error does not say what failed: {error}");
}

/// The same loop, on a slow bus: its total time is bounded too, not only its number of sends.
#[test]
fn a_withdraw_that_keeps_being_answered_fails_at_its_deadline() {
    let limits = BusLimits {
        withdraw_deadline: Duration::from_millis(300),
        ..BUS_LIMITS
    };
    let delay = Duration::from_millis(200);
    let started = std::time::Instant::now();
    let outcome = on_its_own_thread(
        "program_short_address on a slow bus answering WITHDRAW",
        move || {
            let mut bus = AnswersWithdraw::new(delay);
            DaliManager::with_limits(&mut bus, limits)
                .program_short_address(0, 5)
                .map_err(|e| format!("{e:#}"))
        },
    );
    let took = started.elapsed();
    let result = outcome.unwrap_or_else(|failure| panic!("{failure}"));
    assert!(
        result.is_err(),
        "programming succeeded although WITHDRAW was answered every time"
    );
    let bound = limits.withdraw_deadline + delay + SLACK;
    assert!(
        took <= bound,
        "WITHDRAW's retries took {took:?}, past their deadline of {:?} (allowed {bound:?})",
        limits.withdraw_deadline
    );
}

/// A broadcast the bus answers with a collision is sent again, up to a count; on a slow bus that
/// count alone allowed about ten minutes (300 sends of up to ~2 s each). Its total time is bounded
/// (finding Q1).
#[test]
fn a_broadcast_that_keeps_colliding_fails_at_its_deadline() {
    let limits = BusLimits {
        broadcast_deadline: Duration::from_millis(300),
        ..BUS_LIMITS
    };
    let delay = Duration::from_millis(20);
    let sends = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let bus = Collides { sends: sends.clone(), delay };
    let started = std::time::Instant::now();
    let outcome = on_its_own_thread("a broadcast on a bus that always collides", move || {
        let mut bus = bus;
        DaliManager::with_limits(&mut bus, limits)
            .set_dtr(0, 1)
            .map(|_| ())
            .map_err(|e| format!("{e:#}"))
    });
    let took = started.elapsed();
    let result = outcome.unwrap_or_else(|failure| panic!("{failure}"));
    let error = result.expect_err("a broadcast that always collided succeeded");
    let bound = limits.broadcast_deadline + delay + SLACK;
    assert!(
        took <= bound,
        "the broadcast kept resending for {took:?} ({} sends), past its deadline of {:?} (allowed {bound:?})",
        sends.load(std::sync::atomic::Ordering::SeqCst),
        limits.broadcast_deadline
    );
    assert!(error.contains("collision"), "the error does not say what failed: {error}");
}

#[test]
fn the_emulator_refuses_a_bus_it_does_not_have() {
    let result = on_its_own_thread("a command to bus 7 of a one-bus emulator", || {
        let mut dali_config = DaliConfig::new("Emulated");
        dali_config.buses.push(BusConfig::new(0, BusStatus::Active));
        let mut controller =
            DaliControllerEmulator::try_new(&mut dali_config).expect("the emulator");
        let mut manager = DaliManager::new(controller.as_mut());
        block_on(manager.set_light_brightness_async(7, 1, 100))
            .map(|_| ())
            .map_err(|e| e.to_string())
    });
    match result {
        Err(failure) => panic!("{failure}"),
        Ok(result) => assert!(
            result.is_err(),
            "a command to a bus the emulator does not have was accepted"
        ),
    }
}
