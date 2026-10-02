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
