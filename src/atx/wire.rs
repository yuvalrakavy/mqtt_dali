//! ATX I2/I4 newline framing, independent of the UART and DALI device model.

use std::time::{Duration, Instant};

pub const MAX_LINE_BYTES: usize = 64;
pub const LINE_DEADLINE: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub hardware: String,
    pub firmware: String,
    pub variant: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaliFrame {
    Bits8(u8),
    Bits16(u16),
    Bits24(u32),
    Bits25(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Dali(DaliFrame),
    NoResponse,
    ReceiveCollision(Option<u8>),
    UnattributedCollision,
    TransmitCollision(Option<u8>),
    Version(Version),
    Boot(Version),
    BusStatus { status: u8, channels: u8 },
    Power { raw: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub bus: u8,
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Send {
        bus: u8,
        frame: DaliFrame,
        repeat: bool,
    },
    Version,
    BusStatus {
        bus: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("invalid ATX line: {0}")]
    Malformed(&'static str),
    #[error("ATX line exceeded {MAX_LINE_BYTES} bytes")]
    LineTooLong,
    #[error("ATX partial line exceeded its 1 s deadline")]
    LineDeadline,
}

pub fn decode(mut line: &[u8]) -> Result<Packet, DecodeError> {
    if line.is_empty() {
        return Ok(Packet {
            bus: 0,
            kind: Kind::UnattributedCollision,
        });
    }
    let bus = match line.first() {
        Some(b'0'..=b'3') => {
            let bus = line[0] - b'0';
            line = &line[1..];
            bus
        }
        _ => 0,
    };
    let Some((&header, payload)) = line.split_first() else {
        return Err(DecodeError::Malformed("missing header after bus prefix"));
    };
    let value = |width| hex(payload, width);
    let version = || -> Result<Version, DecodeError> {
        value(6)?;
        Ok(Version {
            hardware: String::from_utf8_lossy(&payload[..2]).into_owned(),
            firmware: String::from_utf8_lossy(&payload[2..4]).into_owned(),
            variant: String::from_utf8_lossy(&payload[4..6]).into_owned(),
        })
    };
    let kind = match header {
        b'H' => Kind::Dali(DaliFrame::Bits16(value(4)? as u16)),
        b'J' => Kind::Dali(DaliFrame::Bits8(value(2)? as u8)),
        b'L' => Kind::Dali(DaliFrame::Bits24(value(6)?)),
        b'M' => {
            let value = value(8)?;
            if value >= 1 << 25 {
                return Err(DecodeError::Malformed("25-bit value out of range"));
            }
            Kind::Dali(DaliFrame::Bits25(value))
        }
        b'N' if payload.is_empty() => Kind::NoResponse,
        b'X' | b'Z' => {
            let code = if payload.is_empty() {
                None
            } else if payload.len() <= 2 {
                Some(value(payload.len())? as u8)
            } else {
                return Err(DecodeError::Malformed("collision code too long"));
            };
            if header == b'X' {
                Kind::ReceiveCollision(code)
            } else {
                Kind::TransmitCollision(code)
            }
        }
        b'V' => Kind::Version(version()?),
        b'B' => Kind::Boot(version()?),
        b'D' => {
            let value = value(2)? as u8;
            Kind::BusStatus {
                status: value >> 4,
                channels: value & 15,
            }
        }
        b'P' => Kind::Power { raw: value(7)? },
        _ => return Err(DecodeError::Malformed("unknown header or invalid payload")),
    };
    Ok(Packet { bus, kind })
}

fn hex(bytes: &[u8], width: usize) -> Result<u32, DecodeError> {
    if bytes.len() != width || !bytes.iter().all(u8::is_ascii_hexdigit) {
        return Err(DecodeError::Malformed("wrong width or non-hex payload"));
    }
    bytes.iter().try_fold(0, |value, byte| {
        let nibble = char::from(*byte)
            .to_digit(16)
            .ok_or(DecodeError::Malformed("non-hex digit"))?;
        Ok((value << 4) | nibble)
    })
}

impl Command {
    pub fn encode(&self) -> Result<Vec<u8>, DecodeError> {
        let (bus, body) = match self {
            Self::Version => (0, "v".into()),
            Self::BusStatus { bus } => (*bus, "d".into()),
            Self::Send { bus, frame, repeat } => {
                if *repeat && !matches!(frame, DaliFrame::Bits16(_)) {
                    return Err(DecodeError::Malformed("repeat requires a 16-bit frame"));
                }
                let body = match frame {
                    DaliFrame::Bits8(value) => format!("j{value:02X}"),
                    DaliFrame::Bits16(value) => {
                        format!("{}{value:04X}", if *repeat { 't' } else { 'h' })
                    }
                    DaliFrame::Bits24(value) if *value < 1 << 24 => format!("l{value:06X}"),
                    DaliFrame::Bits25(value) if *value < 1 << 25 => format!("m{value:08X}"),
                    _ => return Err(DecodeError::Malformed("frame value out of range")),
                };
                (*bus, body)
            }
        };
        if bus > 3 {
            return Err(DecodeError::Malformed("bus must be in 0..3"));
        }
        Ok(format!(
            "{}{body}\n",
            if bus == 0 {
                String::new()
            } else {
                bus.to_string()
            }
        )
        .into_bytes())
    }
}

#[derive(Default)]
pub struct Framer {
    line: Vec<u8>,
    started: Option<Instant>,
    discarding: bool,
}

impl Framer {
    pub fn feed(&mut self, bytes: &[u8], now: Instant) -> Vec<Result<Packet, DecodeError>> {
        let mut packets = Vec::new();
        if let Some(error) = self.expire(now) {
            packets.push(Err(error));
        }
        for &byte in bytes {
            if self.discarding {
                if byte == b'\n' {
                    self.discarding = false;
                }
                continue;
            }
            if byte == b'\n' {
                if self.line.last() == Some(&b'\r') {
                    self.line.pop();
                }
                packets.push(decode(&self.line));
                self.line.clear();
                self.started = None;
                continue;
            }
            self.started.get_or_insert(now);
            if self.line.len() >= MAX_LINE_BYTES - 1 {
                self.line.clear();
                self.started = None;
                self.discarding = true;
                packets.push(Err(DecodeError::LineTooLong));
            } else {
                self.line.push(byte);
            }
        }
        packets
    }
    pub fn expire(&mut self, now: Instant) -> Option<DecodeError> {
        if self.deadline().is_some_and(|deadline| now >= deadline) {
            self.line.clear();
            self.started = None;
            self.discarding = true;
            Some(DecodeError::LineDeadline)
        } else {
            None
        }
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.started.map(|start| start + LINE_DEADLINE)
    }

    pub(crate) fn has_incomplete_line(&self) -> bool {
        self.started.is_some()
    }
}

#[cfg(test)]
mod tests;
