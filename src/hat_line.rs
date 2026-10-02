//! The DALI HAT's serial protocol, apart from the UART it runs over.
//!
//! `dali_atx` reads the HAT through the Pi's UART, which exists only on Linux. The loops that
//! wait on it live here, over a [`ByteSource`], so that they compile and are tested on any host
//! against a fake byte source (Store no-hang §14, mqtt_dali).

use std::io;
use std::time::Duration;

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
}

/// How a reply line is read.
pub struct LineLimits {
    /// How long to wait for each byte; no byte before the first one is no reply.
    pub byte_timeout: Duration,
}

/// A reply to a bus command.
pub const REPLY: LineLimits = LineLimits {
    byte_timeout: Duration::from_millis(100),
};

/// Reads and discards until no byte arrives for `quiet`.
pub fn wait_for_idle<S: ByteSource + ?Sized>(src: &mut S, quiet: Duration) -> Result<(), HatError> {
    tracing::debug!("Start Waiting for idle");
    loop {
        match src.read_byte(quiet).unwrap() {
            None => {
                // If timeout, we're idle
                tracing::debug!("bus is idle");
                return Ok(());
            }
            Some(byte) => tracing::debug!("Not idle, Got byte {byte}"),
        }
    }
}

/// One line, up to and including its newline; `None` when nothing came at all.
pub fn read_line<S: ByteSource + ?Sized>(
    src: &mut S,
    limits: &LineLimits,
) -> Result<Option<Vec<u8>>, HatError> {
    let mut line = Vec::new();
    loop {
        match src.read_byte(limits.byte_timeout).map_err(HatError::Read)? {
            None if line.is_empty() => return Ok(None),
            None => {
                line.extend_from_slice(b"N\n");
                return Ok(Some(line));
            }
            Some(byte) => {
                line.push(byte);
                if byte == b'\n' {
                    return Ok(Some(line));
                }
            }
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

/// Two hex digits.
pub fn byte_value(hex: &[u8]) -> Result<u8, HatError> {
    Ok(digit(hex[0])? * 16 + digit(hex[1])?)
}

fn value16(hex: &[u8]) -> Result<u16, HatError> {
    Ok((byte_value(&hex[0..=1])? as u16) << 8 | byte_value(&hex[2..=3])? as u16)
}

fn value24(hex: &[u8]) -> Result<u32, HatError> {
    Ok((byte_value(&hex[0..=1])? as u32) << 16
        | (byte_value(&hex[2..=3])? as u32) << 8
        | byte_value(&hex[4..=5])? as u32)
}

/// The reply to the version query: hardware version, firmware version, number of buses.
pub fn parse_version(line: &[u8]) -> Result<(u8, u8, usize), HatError> {
    Ok((
        byte_value(&line[1..=2])?,
        byte_value(&line[3..=4])?,
        byte_value(&line[5..=6])? as usize,
    ))
}

/// A reply line: an optional bus digit, the reply type, and its value in hex.
pub fn parse_reply(line: &[u8], expected_bus: usize) -> Result<DaliBusResult, HatError> {
    let mut i = 0;

    let (bus, reply_type) = {
        if (b'1'..=b'3').contains(&line[i]) {
            let bus_number = line[i] - b'0';
            i += 1;

            let reply_type = line[i];
            i += 1;

            (bus_number as usize, reply_type)
        } else {
            let reply_type = line[i];
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
