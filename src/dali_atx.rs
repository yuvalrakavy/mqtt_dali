use std::time::Duration;
use rppal::{uart, uart::Uart};

use crate::dali_manager::{DaliController, DaliBusResult};
use crate::config_payload::{Config, BusConfig, BusStatus};

#[derive(Debug)]
enum DaliAtxError {
    UartError(uart::Error),
    InvalidHexDigit(u8),
    UnexpectedBus(usize, usize),
    UnexpectedReply(char),
    UnexpectedBusResult(DaliBusResult),
    UnexpectedBusStatus(u8),
    MismatchBusCount(usize, usize),
}

impl std::fmt::Display for DaliAtxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {

        match self {
            DaliAtxError::InvalidHexDigit(d) => write!(f, "Invalid hex digit {}", d),
            DaliAtxError::MismatchBusCount(config_buses, hw_buses) => write!(f, "Configured for {} while hardware reports {}",
              DaliAtx::to_bus_count_string(*config_buses), DaliAtx::to_bus_count_string(*hw_buses)),
            DaliAtxError::UnexpectedBus(expected, actual) => write!(f, "Reply from unexpected bus (expected {}, reply from {}", expected, actual),
            DaliAtxError::UnexpectedReply(reply) => write!(f, "Unexpected DALI HAT reply: {}", reply),
            DaliAtxError::UnexpectedBusResult(bus_result) => write!(f, "Unexpected bus result {:?}", bus_result),
            DaliAtxError::UnexpectedBusStatus(status) => write!(f, "Unexpected bus status: {}", status),
            DaliAtxError::UartError(uart_error) => write!(f, "UART error {}", uart_error),
        }
    }
}

impl std::error::Error for DaliAtxError {}

impl From<uart::Error> for DaliAtxError {
    fn from(e: uart::Error) -> Self {
        DaliAtxError::UartError(e)
    }
}

pub struct DaliAtx {
    uart: Uart,
    #[allow(dead_code)]
    debug: bool,
}

impl DaliController for DaliAtx {
    fn send_2_bytes(&mut self, bus: usize, b1: u8, b2: u8) -> Result<DaliBusResult, Box<dyn std::error::Error>> {
        self.send_command(bus, 'h')?;
        self.send_byte_value(b1)?;
        self.send_byte_value(b2)?;
        self.send_nl()?;
        self.receive_reply(bus)
    }

    fn send_2_bytes_repeat(&mut self, bus: usize, b1: u8, b2: u8) -> Result<DaliBusResult, Box<dyn std::error::Error>> {
        self.send_command(bus, 't')?;
        self.send_byte_value(b1)?;
        self.send_byte_value(b2)?;
        self.send_nl()?;
        self.receive_reply(bus)
    }

    fn get_bus_status(&mut self, bus: usize) -> Result<BusStatus, Box<dyn std::error::Error>> {
        self.send_command(bus, 'd')?;
        self.send_nl()?;

        let bus_result = self.receive_reply(bus)?;

        if let DaliBusResult::Value8(v) = bus_result {
            match v >> 4 {
                0 => Ok(BusStatus::NoPower),
                1 => Ok(BusStatus::Overloaded),
                2 => Ok(BusStatus::Active),
                s => Err(Box::new(DaliAtxError::UnexpectedBusStatus(s))),
            }
        } else { Err(Box::new(DaliAtxError::UnexpectedBusResult(bus_result))) }
    }
}

impl DaliAtx {
    pub fn try_new(config: &mut Config, debug: bool) -> Result<Box<dyn DaliController>, Box<dyn std::error::Error>> {
        let mut uart = Uart::with_path("/dev/serial0", 19200, rppal::uart::Parity::None, 8, 1)?;
        let mut buffer = [0u8; 8];

        // Read any pending characters
        uart.set_read_mode(0,Duration::from_millis(0))?;
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

        println!("ATX DALI Pi Hat: Hardware version {}, Firmware version {}, {}", hardware_version, firmware_version, DaliAtx::to_bus_count_string(bus_count));

        if config.buses.is_empty() {
            for bus_number in 0..bus_count {
                config.buses.push(BusConfig::new(bus_number, BusStatus::Unknown));
            }
        } else if config.buses.len() != bus_count {
            return Err(Box::new(DaliAtxError::MismatchBusCount(config.buses.len(), bus_count)))
        }

        Ok(Box::new(DaliAtx { uart, debug }))
    }

    fn to_bus_count_string(n: usize) -> String {
        if n == 1 { "1 DALI bus".to_string() } else { format!("{} DALI buses", n)}
    }

    fn get_digit(b: u8) -> Result<u8, DaliAtxError> {
        match b as char {
            'A'..='F' => Ok(b - (b'A') + 10),
            'a'..='f' => Ok(b - (b'a') + 10),
            '0'..='9' => Ok(b - (b'0')),
            _ => Err(DaliAtxError::InvalidHexDigit(b))
        }
    }

    fn get_byte_value(buffer: &[u8]) -> Result<u8, DaliAtxError> {
        Ok(DaliAtx::get_digit(buffer[0])? * 16 + DaliAtx::get_digit(buffer[1])?)
    }

    fn send_command(&mut self, bus: usize, command: char) -> Result<usize, DaliAtxError>  {
        if bus == 0 {
            let command_buffer = [command as u8];
            Ok(self.uart.write(&command_buffer)?)
        } else {
            let command_buffer = [('0' as usize + bus) as u8, command as u8];
            Ok(self.uart.write(&command_buffer)?)
        }
    }

    const HEX_DIGITS: &'static [u8; 16] = b"0123456789abcdef";

    #[allow(dead_code)]
    fn send_byte_value(&mut self, value: u8) -> Result<usize, DaliAtxError> {
        let buffer = [DaliAtx::HEX_DIGITS[(value >> 4) as usize] as u8, DaliAtx::HEX_DIGITS[(value & 0xf) as usize]];

        Ok(self.uart.write(&buffer)?)
    }

    fn send_nl(&mut self) -> Result<usize, DaliAtxError> {
        let buffer = [b'\n'];
        Ok(self.uart.write(&buffer)?)
    }

    fn receive_nl(&mut self) -> Result<(), DaliAtxError> {
        let mut buffer = [0u8; 1];

        self.uart.set_read_mode(1, Duration::from_secs(1))?;
        self.uart.read(&mut buffer)?;

        if buffer[0] as char != '\n' { Err(DaliAtxError::UnexpectedReply(buffer[0] as char)) } else { Ok(()) }
    }

    fn receive_value8(&mut self) -> Result<u8, DaliAtxError> {
        let mut buffer = [0u8; 2];

        self.uart.set_read_mode(2, Duration::from_secs(1))?;
        self.uart.read(&mut buffer)?;

        DaliAtx::get_byte_value(&buffer)
    }

    fn receive_value16(&mut self) -> Result<u16, DaliAtxError> {
        let mut buffer = [0u8; 4];

        self.uart.set_read_mode(4, Duration::from_secs(1))?;
        self.uart.read(&mut buffer)?;

        Ok((DaliAtx::get_byte_value(&buffer[0..=1])? as u16) << 8 | DaliAtx::get_byte_value(&buffer[2..=3])? as u16) 
    }

    fn receive_value24(&mut self) -> Result<u32, DaliAtxError> {
        let mut buffer = [0u8; 6];

        self.uart.set_read_mode(6, Duration::from_secs(1))?;
        self.uart.read(&mut buffer)?;

        Ok(
            (DaliAtx::get_byte_value(&buffer[0..=1])? as u32) << 16 | 
            (DaliAtx::get_byte_value(&buffer[2..=3])? as u32) <<  8 | 
            DaliAtx::get_byte_value(&buffer[4..=5])? as u32
        ) 
    }

    fn receive_reply(&mut self, expected_bus: usize) -> Result<DaliBusResult, Box<dyn std::error::Error>> {
        self.uart.set_read_mode(1, Duration::from_secs(1))?;

        let (bus, reply_type) = {
            let mut buffer = [0u8; 1];

            self.uart.read(&mut buffer)?;
            let c = buffer[0] as char;

            if ('1'..='3').contains(&c) {
                self.uart.read(&mut buffer)?;
                let reply_type = buffer[0] as char;
                    (((c as u8) - b'0') as usize, reply_type)
            } else {
                (0, c)
            }
        };

        if bus == expected_bus {
            match reply_type {
                'H' => {
                        let v = self.receive_value16()?;
                        self.receive_nl()?;
                        Ok(DaliBusResult::Value16(v))
                },
                'J' | 'D' => {
                        let v = self.receive_value8()?;
                        self.receive_nl()?;
                        Ok(DaliBusResult::Value8(v))
                },
                'L' | 'V' => {
                        let v = self.receive_value24()?;
                        self.receive_nl()?;
                        Ok(DaliBusResult::Value24(v))
                }, 
                'X' => { self.receive_nl()?; Ok(DaliBusResult::ReceiveCollision) },
                'Z' => { self.receive_nl()?; Ok(DaliBusResult::TransmitCollision) },
                'N' => { self.receive_nl()?; Ok(DaliBusResult::None) },

                _ => Err(Box::new(DaliAtxError::UnexpectedReply(reply_type)))
            }

        } else {
            Err(Box::new(DaliAtxError::UnexpectedBus(expected_bus, bus)))
        }
    }
}
