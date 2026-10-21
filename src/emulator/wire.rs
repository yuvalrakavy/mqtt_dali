//! Emulator-side codec. Deliberately does not use the driver's ATX codec.
use std::time::Duration;
use tokio::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Version,
    Status(u8),
    Send {
        bus: u8,
        header: u8,
        value: u32,
        repeat: bool,
    },
}
pub fn parse(line: &[u8]) -> Result<Command, &'static str> {
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    let (bus, line) = if matches!(line.first(), Some(b'0'..=b'3')) {
        (line[0] - b'0', &line[1..])
    } else {
        (0, line)
    };
    let Some((&op, data)) = line.split_first() else {
        return Err("empty command");
    };
    match op {
        b'v' if bus == 0 && data.is_empty() => Ok(Command::Version),
        b'd' if data.is_empty() => Ok(Command::Status(bus)),
        b'h' | b't' | b'j' | b'l' | b'm' => {
            let width = match op {
                b'j' => 2,
                b'l' => 6,
                b'm' => 8,
                _ => 4,
            };
            if data.len() != width || !data.iter().all(u8::is_ascii_hexdigit) {
                return Err("bad frame width/hex");
            }
            let mut value = 0u32;
            for &byte in data {
                value = (value << 4) | (byte as char).to_digit(16).unwrap();
            }
            if op == b'm' && value >= 1 << 25 {
                return Err("25-bit overflow");
            }
            Ok(Command::Send {
                bus,
                header: if op == b't' {
                    b'H'
                } else {
                    op.to_ascii_uppercase()
                },
                value,
                repeat: op == b't',
            })
        }
        _ => Err("unknown command"),
    }
}
pub fn record(bus: u8, content: &str) -> Vec<u8> {
    if bus == 0 {
        format!("{content}\n").into_bytes()
    } else {
        format!("{bus}{content}\n").into_bytes()
    }
}
#[derive(Default)]
pub struct Framer {
    bytes: Vec<u8>,
    started: Option<Instant>,
    discard: bool,
}
impl Framer {
    pub fn deadline(&self) -> Option<Instant> {
        self.started.map(|at| at + Duration::from_secs(1))
    }
    pub fn feed(&mut self, bytes: &[u8], now: Instant) -> Vec<Result<Command, &'static str>> {
        let mut commands = Vec::new();
        if self.deadline().is_some_and(|deadline| now >= deadline) {
            self.bytes.clear();
            self.started = None;
            self.discard = true;
            commands.push(Err("partial command deadline"));
        }
        for &byte in bytes {
            if byte == b'\n' {
                if !self.discard {
                    commands.push(parse(&self.bytes));
                }
                self.bytes.clear();
                self.discard = false;
                self.started = None;
            } else if !self.discard {
                if self.bytes.len() >= 63 {
                    self.bytes.clear();
                    self.started = None;
                    self.discard = true;
                    commands.push(Err("command too long"));
                } else {
                    self.started.get_or_insert(now);
                    self.bytes.push(byte);
                }
            }
        }
        commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_literal_command_and_reply_vectors() {
        assert_eq!(
            parse(b"2h0790\r"),
            Ok(Command::Send {
                bus: 2,
                header: b'H',
                value: 0x0790,
                repeat: false
            })
        );
        assert_eq!(
            parse(b"tA5FF"),
            Ok(Command::Send {
                bus: 0,
                header: b'H',
                value: 0xa5ff,
                repeat: true
            })
        );
        assert_eq!(
            parse(b"3l020401"),
            Ok(Command::Send {
                bus: 3,
                header: b'L',
                value: 0x020401,
                repeat: false
            })
        );
        assert_eq!(parse(b"v"), Ok(Command::Version));
        assert_eq!(parse(b"1d"), Ok(Command::Status(1)));
        assert_eq!(record(2, "J42"), b"2J42\n");
        for bad in [
            b"4h0000".as_slice(),
            b"h000",
            b"m02000000",
            b"L010001",
            b"h00\xff0",
            b"vextra",
        ] {
            assert!(parse(bad).is_err());
        }
    }
    #[tokio::test(start_paused = true)]
    async fn emulator_framer_bounds_storage_and_total_partial_lifetime() {
        let mut framer = Framer::default();
        let at = Instant::now();
        assert!(framer.feed(b"h01", at).is_empty());
        assert_eq!(
            framer.feed(b"90", at + Duration::from_secs(1)),
            vec![Err("partial command deadline")]
        );
        assert_eq!(
            framer.feed(b"\nv\n", at + Duration::from_secs(1)),
            vec![Ok(Command::Version)]
        );
        assert_eq!(framer.feed(&[b'x'; 64], at), vec![Err("command too long")]);
        assert!(
            framer.bytes.is_empty(),
            "oversized emulator line retained bytes"
        );
        assert_eq!(framer.feed(b"\n2d\n", at), vec![Ok(Command::Status(2))]);
    }
}
