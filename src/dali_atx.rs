use log::{debug,info, log_enabled, trace, Level::Trace};
use rppal::{uart, uart::Uart};
use std::ascii::escape_default;
use std::str;
use std::time::Duration;
use thiserror::Error;

use crate::config_payload::{BusConfig, BusStatus, DaliConfig};
use crate::{dali_manager, get_version};
use crate::dali_manager::{DaliBusResult, DaliController, DaliManagerError};

#[derive(Debug, Error)]
pub enum DaliAtxError {
    #[error("UART error: {0}")]
    UartError(
        #[from]
        #[source]
        uart::Error,
    ),

    #[error("Invalid hex digit {0}")]
    InvalidHexDigit(u8),

    #[error("Reply from unexpected bus (expected {0}, reply from {1})")]
    UnexpectedBus(usize, usize),

    #[error("Unexpected DALI HAT reply: {0}")]
    UnexpectedReply(u8),

    #[error("Unexpected bus result {0:?}")]
    UnexpectedBusResult(DaliBusResult),

    #[error("Unexpected bus status: {0}")]
    UnexpectedBusStatus(u8),

    #[error("Configured for {0} while hardware reports {1}")]
    MismatchBusCount(usize, usize),
}

impl From<DaliAtxError> for DaliManagerError {
    fn from(e: DaliAtxError) -> Self {
        DaliManagerError::DaliInterfaceError(Box::new(e))
    }
}

impl From<uart::Error> for DaliManagerError {
    fn from(e: uart::Error) -> Self {
        DaliManagerError::DaliInterfaceError(Box::new(DaliAtxError::UartError(e)))
    }
}

pub type Result<T> = std::result::Result<T, DaliAtxError>;

pub struct DaliAtx {
    uart: Uart,
    debug_write_buffer: Vec<u8>,
}

impl DaliController for DaliAtx {
    fn send_2_bytes(&mut self, bus: usize, b1: u8, b2: u8) -> dali_manager::Result<DaliBusResult> {
        self.wait_for_idle(Duration::from_millis(DaliAtx::IDLE_TIME_MILLISECONDS));
        self.send_command(bus, 'h')?;
        self.send_byte_value(b1)?;
        self.send_byte_value(b2)?;
        self.send_nl()?;
        self.receive_reply(bus)
            .map_err(|e| DaliManagerError::DaliInterfaceError(Box::new(e)))
    }

    fn send_2_bytes_repeat(
        &mut self,
        bus: usize,
        b1: u8,
        b2: u8,
    ) -> dali_manager::Result<DaliBusResult> {
        self.wait_for_idle(Duration::from_millis(DaliAtx::IDLE_TIME_MILLISECONDS));
        self.send_command(bus, 't')?;
        self.send_byte_value(b1)?;
        self.send_byte_value(b2)?;
        self.send_nl()?;
        self.receive_reply(bus)
            .map_err(|e| DaliManagerError::DaliInterfaceError(Box::new(e)))
    }

    fn get_bus_status(&mut self, bus: usize) -> dali_manager::Result<BusStatus> {
        self.wait_for_idle(Duration::from_millis(DaliAtx::IDLE_TIME_MILLISECONDS));
        self.send_command(bus, 'd')?;
        self.send_nl()?;

        let bus_result = self.receive_reply(bus)?;

        if let DaliBusResult::Value8(v) = bus_result {
            match v >> 4 {
                0 => Ok(BusStatus::NoPower),
                1 => Ok(BusStatus::Overloaded),
                2 => Ok(BusStatus::Active),
                s => Err(DaliManagerError::DaliInterfaceError(Box::new(
                    DaliAtxError::UnexpectedBusStatus(s),
                ))),
            }
        } else {
            Err(DaliManagerError::DaliInterfaceError(Box::new(
                DaliAtxError::UnexpectedBusResult(bus_result),
            )))
        }
    }
}

impl DaliAtx {
    const IDLE_TIME_MILLISECONDS: u64 = 10;

    pub fn try_new(dali_config: &mut DaliConfig) -> dali_manager::Result<Box<dyn DaliController>> {
        let mut uart = Uart::with_path("/dev/serial0", 19200, rppal::uart::Parity::None, 8, 1)?;
        let mut buffer = [0u8; 8];

        // Read any pending characters
        uart.set_read_mode(0, Duration::from_millis(0))?;
        uart.read(&mut buffer)?;

        // Send v\n command to get board hardware version, firmware version and number of DALI buses
        // Expected reply is Vxxyyzz\n where:
        //  xx = HW version
        //  yy = FW version
        //  zz = 01, 02, 04 (number of buses)
        uart.set_read_mode(8, Duration::from_secs(5))?;
        uart.write("v\n".as_bytes())?;
        uart.read(&mut buffer)?;

        let hardware_version = DaliAtx::get_byte_value(&buffer[1..=2])?;
        let firmware_version = DaliAtx::get_byte_value(&buffer[3..=4])?;
        let bus_count = DaliAtx::get_byte_value(&buffer[5..=6])? as usize;

        println!("{}", get_version());
        println!(
            "ATX DALI Pi Hat: Hardware version {}, Firmware version {}, {}",
            hardware_version,
            firmware_version,
            DaliAtx::to_bus_count_string(bus_count)
        );

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
            return Err(DaliManagerError::DaliInterfaceError(Box::new(
                DaliAtxError::MismatchBusCount(dali_config.buses.len(), bus_count),
            )));
        }

        Ok(Box::new(DaliAtx {
            uart,
            debug_write_buffer: Vec::new(),
        }))
    }

    fn wait_for_idle(&mut self, wait_period: Duration) {
        debug!("Start Waiting for idle");
        loop {
            self.uart.set_read_mode(0, wait_period).unwrap();
            let mut buffer = [0u8; 1];
            if self.uart.read(&mut buffer).unwrap() == 0 {
                // If timeout, we're idle
                debug!("bus is idle");
                break;
            } else {
                debug!("Not idle, Got byte {}", buffer[0]);
            }
        }
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
            self.uart.write(&[*c])?;
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

    fn get_digit(b: u8) -> Result<u8> {
        match b as char {
            'A'..='F' => Ok(b - (b'A') + 10),
            'a'..='f' => Ok(b - (b'a') + 10),
            '0'..='9' => Ok(b - (b'0')),
            _ => Err(DaliAtxError::InvalidHexDigit(b)),
        }
    }

    fn get_byte_value(buffer: &[u8]) -> Result<u8> {
        Ok(DaliAtx::get_digit(buffer[0])? * 16 + DaliAtx::get_digit(buffer[1])?)
    }

    fn send_command(&mut self, bus: usize, command: char) -> Result<usize> {
        if bus == 0 {
            let command_buffer = [command as u8];
            Ok(self.do_write(&command_buffer)?)
        } else {
            let command_buffer = [('0' as usize + bus) as u8, command as u8];
            Ok(self.do_write(&command_buffer)?)
        }
    }

    const HEX_DIGITS: &'static [u8; 16] = b"0123456789ABCDEF";

    #[allow(dead_code)]
    fn send_byte_value(&mut self, value: u8) -> Result<usize> {
        let buffer = [
            DaliAtx::HEX_DIGITS[(value >> 4) as usize],
            DaliAtx::HEX_DIGITS[(value & 0xf) as usize],
        ];

        Ok(self.do_write(&buffer)?)
    }

    fn send_nl(&mut self) -> Result<usize> {
        let buffer = [b'\n'];
        Ok(self.do_write(&buffer)?)
    }

    fn receive_value8(&self, buffer: &[u8]) -> Result<u8> {
        DaliAtx::get_byte_value(buffer)
    }

    fn receive_value16(&self, buffer: &[u8]) -> Result<u16> {
        Ok((DaliAtx::get_byte_value(&buffer[0..=1])? as u16) << 8
            | DaliAtx::get_byte_value(&buffer[2..=3])? as u16)
    }

    fn receive_value24(&self, buffer: &[u8]) -> Result<u32> {
        Ok((DaliAtx::get_byte_value(&buffer[0..=1])? as u32) << 16
            | (DaliAtx::get_byte_value(&buffer[2..=3])? as u32) << 8
            | DaliAtx::get_byte_value(&buffer[4..=5])? as u32)
    }

    fn get_line(&mut self, expected_bus: usize) -> Result<Vec<u8>> {
        let mut line = Vec::new();

        self.uart.set_read_mode(0, Duration::from_millis(100))?;

        Ok({
            let received_line = loop {
                let mut byte_buffer = [0u8];

                let bytes_read = self.uart.read(&mut byte_buffer)?;

                if bytes_read == 0 {
                    trace!("Wait for reply timeout - assuming no reply");
                    if expected_bus > 0 {
                        line.push(expected_bus as u8 + b'0');
                    }
                    line.push(b'N');
                    line.push(b'\n');
                    break line;
                } else {
                    line.push(byte_buffer[0]);

                    if byte_buffer[0] == b'\n' {
                        break line;
                    }
                }
            };

            trace!(
                "Got reply {}",
                DaliAtx::to_nice_string(received_line.as_slice())
            );
            received_line
        })
    }

    fn receive_reply(&mut self, expected_bus: usize) -> Result<DaliBusResult> {
        let line = self.get_line(expected_bus)?;
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
                b'H' => {
                    let v = self.receive_value16(&line[i..])?;
                    Ok(DaliBusResult::Value16(v))
                }
                b'J' | b'D' => {
                    let v = self.receive_value8(&line[i..])?;
                    Ok(DaliBusResult::Value8(v))
                }
                b'L' | b'V' => {
                    let v = self.receive_value24(&line[i..])?;
                    Ok(DaliBusResult::Value24(v))
                }
                b'X' => Ok(DaliBusResult::ReceiveCollision),
                b'Z' => Ok(DaliBusResult::TransmitCollision),
                b'N' => Ok(DaliBusResult::None),

                _ => Err(DaliAtxError::UnexpectedReply(reply_type)),
            }
        } else {
            Err(DaliAtxError::UnexpectedBus(expected_bus, bus))
        }
    }
}
