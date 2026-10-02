//! The DALI HAT's serial protocol, apart from the UART it runs over.
//!
//! `dali_atx` reads the HAT through the Pi's UART, which exists only on Linux. The loops that
//! wait on it live here, over a [`ByteSource`], so that they compile and are tested on any host
//! against a fake byte source (Store no-hang §14, mqtt_dali).

use std::io;
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::dali_manager::DaliBusResult;

/// Where the HAT's bytes come from: the UART on the Pi, a fake in the tests.
pub trait ByteSource {
    /// One byte, waiting at most about `timeout` for it; `None` when none came.
    fn read_byte(&mut self, timeout: Duration) -> io::Result<Option<u8>>;
}

#[derive(Debug, Error)]
pub enum HatError {
    #[error("UART read failed: {0}")]
    Read(#[source] io::Error),

    #[error("Invalid hex digit {0}")]
    InvalidHexDigit(u8),

    #[error("Reply from unexpected bus (expected {0}, reply from {1})")]
    UnexpectedBus(usize, usize),

    #[error("Unexpected DALI HAT reply: {0}")]
    UnexpectedReply(u8),

    #[error("the DALI bus never went quiet: {discarded} bytes discarded in {waited_ms} ms")]
    NeverIdle { discarded: usize, waited_ms: u64 },

    #[error("a reply line reached {max_len} bytes without its newline")]
    LineTooLong { max_len: usize },

    #[error("a reply line did not end within {waited_ms} ms ({got} bytes)")]
    LineDeadline { got: usize, waited_ms: u64 },

    #[error("a reply line stopped after {got} bytes, without its newline")]
    Truncated { got: usize },

    #[error("reply too short: {0:?}")]
    ShortReply(String),
}

/// How long the bus may keep sending before `wait_for_idle` gives up on it.
pub const IDLE_DEADLINE: Duration = Duration::from_secs(1);

/// How a reply line is read.
pub struct LineLimits {
    /// How long to wait for each byte; no byte before the first one is no reply.
    pub byte_timeout: Duration,
    /// How long the whole line may take.
    pub total: Duration,
    /// How long the line may be, its newline included.
    pub max_len: usize,
}

/// A reply to a bus command (`[bus]` type value, at most 9 bytes).
pub const REPLY: LineLimits = LineLimits {
    byte_timeout: Duration::from_millis(100),
    total: Duration::from_secs(1),
    max_len: 32,
};

/// The reply to the version query at start-up (`Vxxyyzz`).
pub const VERSION: LineLimits = LineLimits {
    byte_timeout: Duration::from_secs(5),
    total: Duration::from_secs(6),
    max_len: 32,
};

/// Reads and discards until no byte arrives for `quiet`. A bus that keeps sending for
/// [`IDLE_DEADLINE`] is an error, not a reason to wait on: each read waits at most about `quiet`,
/// so this returns within the deadline and one read.
pub fn wait_for_idle<S: ByteSource + ?Sized>(src: &mut S, quiet: Duration) -> Result<(), HatError> {
    tracing::debug!("Start Waiting for idle");
    let start = Instant::now();
    let mut discarded = 0usize;
    loop {
        // WAIT: dali-uart-idle
        match src.read_byte(quiet).map_err(HatError::Read)? {
            None => {
                // If timeout, we're idle
                tracing::debug!("bus is idle");
                return Ok(());
            }
            Some(byte) => {
                discarded += 1;
                tracing::debug!("Not idle, Got byte {byte}");
            }
        }
        let waited = start.elapsed();
        if waited >= IDLE_DEADLINE {
            return Err(HatError::NeverIdle {
                discarded,
                waited_ms: waited.as_millis() as u64,
            });
        }
    }
}

/// One line, up to and including its newline; `None` when nothing came at all. A line longer
/// than `limits.max_len`, or slower than `limits.total`, or one that stops before its newline is
/// an error: this returns within `limits.total` and one byte's wait.
pub fn read_line<S: ByteSource + ?Sized>(
    src: &mut S,
    limits: &LineLimits,
) -> Result<Option<Vec<u8>>, HatError> {
    let start = Instant::now();
    let mut line = Vec::new();
    loop {
        // WAIT: dali-uart-line
        match src.read_byte(limits.byte_timeout).map_err(HatError::Read)? {
            None if line.is_empty() => return Ok(None),
            None => return Err(HatError::Truncated { got: line.len() }),
            Some(byte) => {
                line.push(byte);
                if byte == b'\n' {
                    return Ok(Some(line));
                }
                if line.len() >= limits.max_len {
                    return Err(HatError::LineTooLong {
                        max_len: limits.max_len,
                    });
                }
            }
        }
        let waited = start.elapsed();
        if waited >= limits.total {
            return Err(HatError::LineDeadline {
                got: line.len(),
                waited_ms: waited.as_millis() as u64,
            });
        }
    }
}

fn digit(b: u8) -> Result<u8, HatError> {
    match b as char {
        'A'..='F' => Ok(b - (b'A') + 10),
        'a'..='f' => Ok(b - (b'a') + 10),
        '0'..='9' => Ok(b - (b'0')),
        _ => Err(HatError::InvalidHexDigit(b)),
    }
}

fn short(reply: &[u8]) -> HatError {
    HatError::ShortReply(String::from_utf8_lossy(reply).into_owned())
}

/// The first `n` bytes of `hex`, or a short-reply error naming all of it.
fn digits(hex: &[u8], n: usize) -> Result<&[u8], HatError> {
    hex.get(..n).ok_or_else(|| short(hex))
}

/// Two hex digits.
pub fn byte_value(hex: &[u8]) -> Result<u8, HatError> {
    let hex = digits(hex, 2)?;
    Ok(digit(hex[0])? * 16 + digit(hex[1])?)
}

fn value16(hex: &[u8]) -> Result<u16, HatError> {
    let hex = digits(hex, 4)?;
    Ok((byte_value(&hex[0..=1])? as u16) << 8 | byte_value(&hex[2..=3])? as u16)
}

fn value24(hex: &[u8]) -> Result<u32, HatError> {
    let hex = digits(hex, 6)?;
    Ok((byte_value(&hex[0..=1])? as u32) << 16
        | (byte_value(&hex[2..=3])? as u32) << 8
        | byte_value(&hex[4..=5])? as u32)
}

/// The reply to the version query: hardware version, firmware version, number of buses.
pub fn parse_version(line: &[u8]) -> Result<(u8, u8, usize), HatError> {
    let hex = line.get(1..).ok_or_else(|| short(line))?;
    let hex = digits(hex, 6).map_err(|_| short(line))?;
    Ok((
        byte_value(&hex[0..=1])?,
        byte_value(&hex[2..=3])?,
        byte_value(&hex[4..=5])? as usize,
    ))
}

/// A reply line: an optional bus digit, the reply type, and its value in hex.
pub fn parse_reply(line: &[u8], expected_bus: usize) -> Result<DaliBusResult, HatError> {
    let at = |i: usize| line.get(i).copied().ok_or_else(|| short(line));
    let mut i = 0;

    let (bus, reply_type) = {
        if (b'1'..=b'3').contains(&at(i)?) {
            let bus_number = at(i)? - b'0';
            i += 1;

            let reply_type = at(i)?;
            i += 1;

            (bus_number as usize, reply_type)
        } else {
            let reply_type = at(i)?;
            i += 1;

            (0, reply_type)
        }
    };

    if bus == expected_bus {
        match reply_type {
            b'H' => Ok(DaliBusResult::Value16(value16(&line[i..])?)),
            b'J' | b'D' => Ok(DaliBusResult::Value8(byte_value(&line[i..])?)),
            b'L' | b'V' => Ok(DaliBusResult::Value24(value24(&line[i..])?)),
            b'X' => Ok(DaliBusResult::ReceiveCollision),
            b'Z' => Ok(DaliBusResult::TransmitCollision),
            b'N' => Ok(DaliBusResult::None),

            _ => Err(HatError::UnexpectedReply(reply_type)),
        }
    } else {
        Err(HatError::UnexpectedBus(expected_bus, bus))
    }
}

#[cfg(test)]
mod tests;
