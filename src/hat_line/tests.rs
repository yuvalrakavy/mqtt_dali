//! The HAT's loops against fake byte sources (Store no-hang §14, mqtt_dali, findings C-M2/X-3):
//! a bus that never goes quiet, a reply that never ends or trickles, a UART that fails, and
//! replies cut short. Each call runs on a thread of its own under an external deadline, so a
//! loop that never returns fails the test instead of hanging the suite.

use std::collections::VecDeque;
use std::io;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::*;

/// How long a call may take before it counts as never returning.
const WITHIN: Duration = Duration::from_secs(10);
/// Scheduling slack on a shared machine, past a deadline the code itself enforces.
const SLACK: Duration = Duration::from_secs(1);

/// Runs `f` on its own thread; panics if it does not return within [`WITHIN`] or if it panics.
fn bounded<T: Send + 'static>(what: &str, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(WITHIN) {
        Ok(value) => value,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            panic!("{what} did not return within {WITHIN:?}: an unbounded wait on the UART")
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            panic!("{what} panicked instead of returning an error")
        }
    }
}

/// A bus that never goes quiet: a byte every millisecond (19200 baud sends one every ~0.5 ms).
struct Chattering(u8);

impl ByteSource for Chattering {
    fn read_byte(&mut self, timeout: Duration) -> io::Result<Option<u8>> {
        std::thread::sleep(timeout.min(Duration::from_millis(1)));
        Ok(Some(self.0))
    }
}

/// A byte every `every`, never a newline.
struct Trickle {
    byte: u8,
    every: Duration,
}

impl ByteSource for Trickle {
    fn read_byte(&mut self, timeout: Duration) -> io::Result<Option<u8>> {
        if self.every > timeout {
            std::thread::sleep(timeout);
            return Ok(None);
        }
        std::thread::sleep(self.every);
        Ok(Some(self.byte))
    }
}

/// A UART whose reads fail.
struct Failing;

impl ByteSource for Failing {
    fn read_byte(&mut self, _timeout: Duration) -> io::Result<Option<u8>> {
        Err(io::Error::other("the UART went away"))
    }
}

/// These bytes, then silence.
struct Script(VecDeque<u8>);

impl Script {
    fn new(bytes: &[u8]) -> Script {
        Script(bytes.iter().copied().collect())
    }
}

impl ByteSource for Script {
    fn read_byte(&mut self, _timeout: Duration) -> io::Result<Option<u8>> {
        Ok(self.0.pop_front())
    }
}

#[test]
fn a_bus_that_never_goes_quiet_ends_the_idle_wait_with_an_error() {
    let (result, took) = bounded("wait_for_idle on a bus that never goes quiet", || {
        let start = Instant::now();
        let result = wait_for_idle(&mut Chattering(0x55), Duration::from_millis(10));
        (result, start.elapsed())
    });
    assert!(
        matches!(result, Err(HatError::NeverIdle { .. })),
        "the idle wait on a chattering bus did not end with NeverIdle: {result:?}"
    );
    assert!(
        took >= IDLE_DEADLINE && took < IDLE_DEADLINE + SLACK,
        "the idle wait gave up after {took:?}, not at its {IDLE_DEADLINE:?} deadline"
    );
}

#[test]
fn a_reply_that_never_ends_is_cut_at_its_length_limit() {
    let (result, took) = bounded("read_line on a reply that never ends", || {
        let start = Instant::now();
        let result = read_line(&mut Chattering(b'J'), &REPLY);
        (result, start.elapsed())
    });
    assert!(
        matches!(result, Err(HatError::LineTooLong { max_len }) if max_len == REPLY.max_len),
        "a reply that never ends was not cut at its length limit: {result:?}"
    );
    assert!(
        took < REPLY.total,
        "the length limit took {took:?} to cut the line"
    );
}

#[test]
fn a_reply_that_trickles_without_ending_is_cut_at_its_deadline() {
    // 50 ms a byte reaches the 32-byte limit only after 1.6 s: the 1 s deadline comes first.
    let (result, took) = bounded("read_line on a trickling reply", || {
        let start = Instant::now();
        let result = read_line(
            &mut Trickle {
                byte: b'0',
                every: Duration::from_millis(50),
            },
            &REPLY,
        );
        (result, start.elapsed())
    });
    assert!(
        matches!(result, Err(HatError::LineDeadline { .. })),
        "a trickling reply was not cut at its deadline: {result:?}"
    );
    assert!(
        took >= REPLY.total && took < REPLY.total + SLACK,
        "the line gave up after {took:?}, not at its {:?} deadline",
        REPLY.total
    );
}

#[test]
fn a_read_error_is_an_error_not_a_panic() {
    let idle = bounded("wait_for_idle on a failing UART", || {
        wait_for_idle(&mut Failing, Duration::from_millis(10))
    });
    assert!(
        matches!(idle, Err(HatError::Read(_))),
        "the idle wait on a failing UART: {idle:?}"
    );
    let line = bounded("read_line on a failing UART", || {
        read_line(&mut Failing, &REPLY)
    });
    assert!(
        matches!(line, Err(HatError::Read(_))),
        "the reply on a failing UART: {line:?}"
    );
}

#[test]
fn a_reply_cut_short_is_an_error() {
    let line = bounded("read_line on a reply cut short", || {
        read_line(&mut Script::new(b"J4"), &REPLY)
    });
    assert!(
        matches!(line, Err(HatError::Truncated { got: 2 })),
        "a reply that stopped before its newline was not an error: {line:?}"
    );
}

#[test]
fn a_quiet_bus_is_idle_a_silent_one_is_no_reply_and_a_whole_line_is_read() {
    assert!(wait_for_idle(&mut Script::new(b""), Duration::from_millis(10)).is_ok());
    assert!(matches!(read_line(&mut Script::new(b""), &REPLY), Ok(None)));
    let mut src = Script::new(b"1J42\nZ");
    let line = read_line(&mut src, &REPLY)
        .expect("a whole line")
        .expect("a line");
    assert_eq!(line, b"1J42\n");
    assert_eq!(src.0, VecDeque::from(vec![b'Z']), "read past the newline");
    assert!(matches!(
        parse_reply(&line, 1),
        Ok(DaliBusResult::Value8(0x42))
    ));
    assert!(matches!(parse_reply(b"N\n", 0), Ok(DaliBusResult::None)));
    assert!(matches!(
        parse_reply(b"H12aB\n", 0),
        Ok(DaliBusResult::Value16(0x12ab))
    ));
    assert!(matches!(
        parse_reply(b"2L123456\n", 2),
        Ok(DaliBusResult::Value24(0x123456))
    ));
    assert!(matches!(
        parse_reply(b"1N\n", 2),
        Err(HatError::UnexpectedBus(2, 1))
    ));
    assert!(matches!(parse_version(b"V010204\n"), Ok((1, 2, 4))));
}

#[test]
fn a_short_or_garbled_reply_is_an_error_not_a_panic() {
    let replies: [(&[u8], usize); 9] = [
        (b"", 0),
        (b"1", 1),
        (b"1H\n", 1),
        (b"J4", 0),
        (b"H12\n", 0),
        (b"3L1234\n", 3),
        (b"D\n", 0),
        (b"1HZZZZ\n", 1),
        (b"\n", 0),
    ];
    for (reply, bus) in replies {
        let shown = String::from_utf8_lossy(reply).into_owned();
        let owned = reply.to_vec();
        let result = bounded(&format!("parse_reply({shown:?})"), move || {
            parse_reply(&owned, bus).map_err(|e| e.to_string())
        });
        assert!(result.is_err(), "the reply {shown:?} parsed as {result:?}");
    }
    for version in [&b""[..], b"V", b"V0102\n"] {
        let shown = String::from_utf8_lossy(version).into_owned();
        let owned = version.to_vec();
        let result = bounded(&format!("parse_version({shown:?})"), move || {
            parse_version(&owned).map_err(|e| e.to_string())
        });
        assert!(
            result.is_err(),
            "the version reply {shown:?} parsed as {result:?}"
        );
    }
}
