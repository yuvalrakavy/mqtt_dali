use error_stack::{Report, ResultExt};
use log::{info, log_enabled, trace, Level::Trace};
use rppal::{uart, uart::Uart};
use std::ascii::escape_default;
use std::io;
use std::str;
use std::time::Duration;
use thiserror::Error;

use crate::config_payload::{BusConfig, BusStatus, DaliConfig};
use crate::dali_manager::{DaliBusResult, DaliController, DaliManagerError};
use crate::hat_line::{self, ByteSource, HatError};
use crate::{dali_manager, get_version};

#[derive(Debug, Error)]
pub enum DaliAtxError {
    #[error("UART error: {0}")]
    UartError(
        #[from]
        #[source]
        uart::Error,
    ),

    #[error(transparent)]
    Hat(#[from] HatError),

    #[error("Unexpected bus result {0:?}")]
    UnexpectedBusResult(DaliBusResult),

    #[error("Unexpected bus status: {0}")]
    UnexpectedBusStatus(u8),

    #[error("Configured for {0} while hardware reports {1}")]
    MismatchBusCount(usize, usize),

    #[error("The DALI HAT did not answer the version query")]
    NoVersionReply,

    #[error("In context of '{0}'")]
    Context(String),
}

//*REMOVE*/
// impl From<DaliAtxError> for DaliManagerError {
//     fn from(e: DaliAtxError) -> Self {
//         DaliManagerError::DaliInterfaceError(e.to_string())
//     }
// }

// impl From<uart::Error> for DaliManagerError {
//     fn from(e: uart::Error) -> Self {
//         DaliManagerError::DaliInterfaceError(e.to_string())
//     }
// }

pub type Result<T> = std::result::Result<T, Report<DaliAtxError>>;

pub struct DaliAtx {
    port: UartPort,
    debug_write_buffer: Vec<u8>,
}

/// The UART as the HAT protocol's byte source. It remembers the read mode it last set, since
/// setting one is a system call.
struct UartPort {
    uart: Uart,
    read_timeout: Option<Duration>,
}

impl ByteSource for UartPort {
    /// One termios read of one byte: `VMIN` 0 and `VTIME` the timeout, which rppal rounds down to
    /// tenths of a second (so under 100 ms it is a poll of the input queue).
    fn read_byte(&mut self, timeout: Duration) -> io::Result<Option<u8>> {
        if self.read_timeout != Some(timeout) {
            self.uart.set_read_mode(0, timeout).map_err(uart_io_error)?;
            self.read_timeout = Some(timeout);
        }
        let mut byte = [0u8; 1];
        match self.uart.read(&mut byte).map_err(uart_io_error)? {
            0 => Ok(None),
            _ => Ok(Some(byte[0])),
        }
    }
}

fn uart_io_error(e: uart::Error) -> io::Error {
    match e {
        uart::Error::Io(e) => e,
        other => io::Error::other(other.to_string()),
    }
}

impl DaliController for DaliAtx {
    fn send_2_bytes(&mut self, bus: usize, b1: u8, b2: u8) -> dali_manager::Result<DaliBusResult> {
        let into_context = || {
            DaliManagerError::Context(format!(
                "Sending 2 bytes DALI interface bus {bus} ({b1},{b2})"
            ))
        };

        self.wait_for_idle(Duration::from_millis(DaliAtx::IDLE_TIME_MILLISECONDS))
            .change_context_lazy(into_context)?;
        self.send_command(bus, 'h')
            .change_context_lazy(into_context)?;
        self.send_byte_value(b1).change_context_lazy(into_context)?;
        self.send_byte_value(b2).change_context_lazy(into_context)?;
        self.send_nl().change_context_lazy(into_context)?;
        self.receive_reply(bus).change_context_lazy(into_context)
    }

    fn send_2_bytes_repeat(
        &mut self,
        bus: usize,
        b1: u8,
        b2: u8,
    ) -> dali_manager::Result<DaliBusResult> {
        let into_context = || {
            DaliManagerError::Context(format!(
                "Sending 2 bytes (repeat) DALI interface bus {bus} ({b1},{b2})"
            ))
        };

        self.wait_for_idle(Duration::from_millis(DaliAtx::IDLE_TIME_MILLISECONDS))
            .change_context_lazy(into_context)?;
        self.send_command(bus, 't')
            .change_context_lazy(into_context)?;
        self.send_byte_value(b1).change_context_lazy(into_context)?;
        self.send_byte_value(b2).change_context_lazy(into_context)?;
        self.send_nl().change_context_lazy(into_context)?;
        self.receive_reply(bus).change_context_lazy(into_context)
    }

    fn get_bus_status(&mut self, bus: usize) -> dali_manager::Result<BusStatus> {
        let into_context = || DaliManagerError::Context(format!("Getting status from bus {bus}"));

        self.wait_for_idle(Duration::from_millis(DaliAtx::IDLE_TIME_MILLISECONDS))
            .change_context_lazy(into_context)?;
        self.send_command(bus, 'd')
            .change_context_lazy(into_context)?;
        self.send_nl().change_context_lazy(into_context)?;

        let bus_result = self.receive_reply(bus).change_context_lazy(into_context)?;

        if let DaliBusResult::Value8(v) = bus_result {
            match v >> 4 {
                0 => Ok(BusStatus::NoPower),
                1 => Ok(BusStatus::Overloaded),
                2 => Ok(BusStatus::Active),
                s => Err(DaliAtxError::UnexpectedBusStatus(s)).change_context_lazy(into_context),
            }
        } else {
            Err(DaliAtxError::UnexpectedBusResult(bus_result)).change_context_lazy(into_context)
        }
    }
}

impl DaliAtx {
    const IDLE_TIME_MILLISECONDS: u64 = 10;

    pub fn try_new(dali_config: &mut DaliConfig) -> dali_manager::Result<Box<dyn DaliController>> {
        let into_context = || DaliManagerError::Context("Creating ATX controller".into());
        let uart = Uart::with_path("/dev/serial0", 19200, rppal::uart::Parity::None, 8, 1)
            .change_context_lazy(into_context)?;
        let mut port = UartPort { uart, read_timeout: None };

        // Discard any pending characters (a zero timeout polls the input queue).
        hat_line::wait_for_idle(&mut port, Duration::ZERO)
            .map_err(DaliAtxError::from)
            .change_context_lazy(into_context)?;

        // Send v\n command to get board hardware version, firmware version and number of DALI buses
        // Expected reply is Vxxyyzz\n where:
        //  xx = HW version
        //  yy = FW version
        //  zz = 01, 02, 04 (number of buses)
        // The reply is read under hat_line::VERSION's limits: a HAT that never answers fails the
        // start-up within seconds (the old read, VMIN 8 with VTIME, waited for its first byte
        // forever).
        port.uart.write("v\n".as_bytes())
            .change_context_lazy(into_context)?;
        let reply = hat_line::read_line(&mut port, &hat_line::VERSION)
            .map_err(DaliAtxError::from)
            .change_context_lazy(into_context)?
            .ok_or(DaliAtxError::NoVersionReply)
            .change_context_lazy(into_context)?;

        let (hardware_version, firmware_version, bus_count) = hat_line::parse_version(&reply)
            .map_err(DaliAtxError::from)
            .change_context_lazy(into_context)?;

        // Logged only, never printed: a stdout nobody drains must not hold the start (fleet
        // class B2).
        info!("Started: {}", get_version());
        info!(
            "ATX DALI Pi Hat: Hardware version {}, Firmware version {}, {}",
            hardware_version,
            firmware_version,
            DaliAtx::to_bus_count_string(bus_count)
        );

        if dali_config.buses.is_empty() {
            for bus_number in 0..bus_count {
                dali_config
                    .buses
                    .push(BusConfig::new(bus_number, BusStatus::Unknown));
            }
        } else if dali_config.buses.len() != bus_count {
            return Err(DaliAtxError::MismatchBusCount(
                dali_config.buses.len(),
                bus_count,
            ))
            .change_context_lazy(into_context);
        }

        Ok(Box::new(DaliAtx {
            port,
            debug_write_buffer: Vec::new(),
        }))
    }

    fn wait_for_idle(&mut self, wait_period: Duration) -> Result<()> {
        Ok(hat_line::wait_for_idle(&mut self.port, wait_period).map_err(DaliAtxError::from)?)
    }

    fn to_nice_string(bs: &[u8]) -> String {
        let mut visible = String::new();
        for &b in bs {
            let part: Vec<u8> = escape_default(b).collect();
            visible.push_str(str::from_utf8(&part).unwrap());
        }
        visible
    }

    fn flush_debug_write(&mut self) {
        trace!(
            "UART sent: {}",
            DaliAtx::to_nice_string(self.debug_write_buffer.as_slice())
        );
        self.debug_write_buffer.clear();
    }

    fn do_write(&mut self, buffer: &[u8]) -> rppal::uart::Result<usize> {
        if log_enabled!(Trace) {
            for b in buffer {
                self.debug_write_buffer.push(*b);
                if *b == b'\n' {
                    self.flush_debug_write();
                }
            }
        }

        for c in buffer {
            self.port.uart.write(&[*c])?;
        }
        Ok(buffer.len())
    }

    fn to_bus_count_string(n: usize) -> String {
        if n == 1 {
            "1 DALI bus".to_string()
        } else {
            format!("{} DALI buses", n)
        }
    }

    fn send_command(&mut self, bus: usize, command: char) -> Result<usize> {
        let into_context =
            || DaliAtxError::Context(format!("send_command({}, {})", bus, command));

        if bus == 0 {
            let command_buffer = [command as u8];
            Ok(self
                .do_write(&command_buffer)
                .map_err(DaliAtxError::from)
                .change_context_lazy(into_context)?)
        } else {
            let command_buffer = [('0' as usize + bus) as u8, command as u8];
            Ok(self
                .do_write(&command_buffer)
                .map_err(DaliAtxError::from)
                .change_context_lazy(into_context)?)
        }
    }

    const HEX_DIGITS: &'static [u8; 16] = b"0123456789ABCDEF";

    #[allow(dead_code)]
    fn send_byte_value(&mut self, value: u8) -> Result<usize> {
        let into_context =
            || DaliAtxError::Context(format!("Sending byte to DALI interface ({})", value));
        let buffer = [
            DaliAtx::HEX_DIGITS[(value >> 4) as usize],
            DaliAtx::HEX_DIGITS[(value & 0xf) as usize],
        ];

        self.do_write(&buffer).change_context_lazy(into_context)
    }

    fn send_nl(&mut self) -> Result<usize> {
        let into_context = || DaliAtxError::Context("Sending newline to DALI interface".to_string());
        let buffer = [b'\n'];
        self.do_write(&buffer).change_context_lazy(into_context)
    }

    fn get_line(&mut self, expected_bus: usize) -> Result<Option<Vec<u8>>> {
        let into_context =
            || DaliAtxError::Context(format!("Getting reply line from DALI bus {expected_bus}"));
        let line = hat_line::read_line(&mut self.port, &hat_line::REPLY)
            .map_err(DaliAtxError::from)
            .change_context_lazy(into_context)?;
        match &line {
            Some(line) => trace!("Got reply {}", DaliAtx::to_nice_string(line)),
            None => trace!("Wait for reply timeout - assuming no reply"),
        }
        Ok(line)
    }

    fn receive_reply(&mut self, expected_bus: usize) -> Result<DaliBusResult> {
        match self.get_line(expected_bus)? {
            // No reply within the byte timeout: the bus answered nothing.
            None => Ok(DaliBusResult::None),
            Some(line) => Ok(hat_line::parse_reply(&line, expected_bus).map_err(DaliAtxError::from)?),
        }
    }
}
