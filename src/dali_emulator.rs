use rand::Rng;
use std::cell::RefCell;
use crate::dali_commands::{self};
use crate::dali_manager::{DaliBusResult, DaliController};
use crate::config_payload::{BusConfig, BusStatus, Config};

#[derive(Debug)]
struct DaliLightEmulator {
    light_number: usize,
    debug: bool,
    initialize_mode: bool,
    brightness: u8,
    short_address: u8,
    random_address: u32,
    search_address: u32,
    enable_compare: bool,
    selected: bool,
    group_mask: u16,
    dtr: [u8; 3],
}

#[derive(Debug)]
pub struct DaliBusEmulator {
    bus_number: usize,
    lights: RefCell<Vec<DaliLightEmulator>>,
    debug: bool,
}

pub struct DaliControllerEmulator {
    buses: Vec<DaliBusEmulator>,
}

impl DaliLightEmulator {
    fn new(light_number: usize, debug: bool) -> DaliLightEmulator {
        DaliLightEmulator {
             light_number,
             debug,
             initialize_mode: false,
             brightness: 0,
             short_address: 0xff,
             search_address: 0,
             random_address: 0x0fff,
             enable_compare: false,
             selected: false,
             group_mask: 0,
             dtr: [0, 0, 0]
        }
    }

    fn new_with_config(light_number: usize, short_address: u8, group_mask: u16, debug: bool) -> DaliLightEmulator {
        DaliLightEmulator {
            light_number,
            debug,
            initialize_mode: false,
            brightness: 0,
            short_address,
            search_address: 0,
            random_address: 0x0fff,
            enable_compare: false,
            selected: false,
            group_mask,
            dtr: [0, 0, 0]
       }
    }

    fn command(&mut self, command: u16, parameter: u8) -> Option<u8> {
        match command {
            dali_commands::DALI_ADD_TO_GROUP0..=dali_commands::DALI_ADD_TO_GROUP15 => self.add_to_group(command-dali_commands::DALI_ADD_TO_GROUP0),
            dali_commands::DALI_REMOVE_FROM_GROUP0..=dali_commands::DALI_REMOVE_FROM_GROUP15 => self.remove_from_group(command-dali_commands::DALI_REMOVE_FROM_GROUP0),
            dali_commands::DALI_SET_SHORT_ADDRESS => self.set_short_address(),
            dali_commands::DALI_TERMINATE => self.terminate_initialize_mode(),
            dali_commands::DALI_DATA_TRANSFER_REGISTER0 => self.set_dtr(0, parameter),
            dali_commands::DALI_INITIALISE => self.start_initialize_mode(parameter),
            dali_commands::DALI_RANDOMISE => self.randomize(),
            dali_commands::DALI_COMPARE => return self.compare(),
            dali_commands::DALI_WITHDRAW => self.withdraw(),
            dali_commands::DALI_SEARCHADDRH => self.set_search_address_high(parameter),
            dali_commands::DALI_SEARCHADDRM => self.set_search_address_middle(parameter),
            dali_commands::DALI_SEARCHADDRL => self.set_search_address_low(parameter),
            dali_commands::DALI_PROGRAM_SHORT_ADDRESS => self.program_short_address(parameter),

            _ => println!("DALI Light {} - Unsupported command {} ({:#03x})", self.light_number, command, command),
        }
        None
    }

    fn is_special_command(b1: u8) -> bool {
        (0b10100000..=0b11001011).contains(&b1) || (0b11001100..=0b11111011).contains(&b1)
    }

    // Receive 2 bytes DALI command
    pub fn receive_2_bytes(&mut self, b1: u8, b2: u8) -> Option<u8> {
        if (b1  & 0x01) == 0 && !DaliLightEmulator::is_special_command(b1) { // b2 is light level
            let mut set_my_brightness = false;

            if b1 & 0b10000000 == 0 {
                if (b1 >> 1) == self.short_address { set_my_brightness = true }
            } else if (b1 & 0b11100000) == 0b10000000 {
                let group_mask: u16 = 1 << ((b1 & 0b00011110) >> 1);
                if (group_mask & self.group_mask) != 0 { set_my_brightness = true }
            } else {
                set_my_brightness = true;    // broadcast
            }
            
            if set_my_brightness {
                self.set_brightness(b2);
            }

            None            // No reply on the bus
        }
        else {
            let mut my_command = false;
            let mut command: u16 = b2 as u16;

            if DaliLightEmulator::is_special_command(b1) {
                my_command = true;
                command = 0x100 | (b1 as u16);
            } else if b1 & 0b10000000 == 0 {
                if (b1 >> 1) == self.short_address { my_command = true }
            } else if (b1 & 0b11100000) == 0b10000000 {
                let group_mask: u16 = 1 << ((b1 & 0b00011110) >> 1);
                if (group_mask & self.group_mask) != 0 { my_command = true }
            } else {
                my_command = true;    // broadcast
            }

            if my_command { self.command(command, b2) } else { None }
        }
    }

    ///////////////////////////////////////////////////////////////////////////////
    /// Command implementation
    /// 
    fn set_short_address(&mut self) {
        if self.dtr[0] < 63 {
            if self.debug { println!("DALI light {} set to short address {}", self.light_number, self.dtr[0]) }
            self.short_address = self.dtr[0];
        } else if self.debug { println!("DALI light {} Attempt to set short address {} which is invalid", self.light_number, self.dtr[0]) }
    }

    fn set_brightness(&mut self, level: u8) {
        println!("DALI light {}:{} brightness set to {}", self.light_number, self.short_address, level);
        self.brightness = level;
    }

    fn add_to_group(&mut self, group_number: u16) {
        if self.debug { println!("DALI light {}:{} added to group {}", self.light_number, self.short_address, group_number); }
        self.group_mask |= 1 << group_number;
    }

    fn remove_from_group(&mut self, group_number: u16) {
        if self.debug { println!("DALI light {}:{} removed from group {}", self.light_number, self.short_address, group_number); }
        self.group_mask &= !(1 << group_number);
    }

    fn start_initialize_mode(&mut self, parameter: u8) {
        
        if (parameter == 0xff && self.short_address == 0xff) || parameter == 0 || ((parameter & 0x01) != 0 && (parameter >> 1) == self.short_address) {
            if self.debug { println!("DALI light {} start initialization mode", self.light_number) };
            self.initialize_mode = true;
            self.enable_compare = true;
            self.selected = false;
        }
    }

    fn terminate_initialize_mode(&mut self) {
        if self.debug { println!("DALI light {} terminate initialization mode", self.light_number); }
        self.initialize_mode =false;
        self.enable_compare = false;
    }

    fn set_dtr(&mut self, dtr_number: u8, value: u8) {
        if self.debug { println!("DALI light {} set DTR{} to {}", self.light_number, dtr_number, value); }

        if dtr_number < 3 {
            self.dtr[dtr_number as usize] = value;
        } else {
            println!("  Invalid DTR number (0, 1, 2)");
        }
    }

    fn randomize(&mut self) {
        let mut rng = rand::thread_rng();

        self.random_address = rng.gen_range(0..=0x0fff);
        if self.debug { println!("DALI light {} randomized address set to {}", self.light_number, self.random_address) }
    }

    fn compare(&mut self) -> Option<u8> {
        if self.enable_compare {
            if self.debug { println!("DALI light {} check if random {} <= search {} ", self.light_number, self.random_address, self.search_address) };
            self.selected = self.random_address == self.search_address;
            if self.random_address <= self.search_address { Some(0xff) } else { None }
        } else {
            if self.debug { println!("DALI light {} not participating in compare", self.light_number) };
            None
        }
    }

    fn withdraw(&mut self) {
        if self.selected {
            if self.debug { println!("DALI light {} withdrawing from compare process", self.light_number); }
            self.enable_compare = false;
        } else if self.debug { println!("DALI light {} not withdrawing from compare process", self.light_number) };
    }

    fn set_search_address_low(&mut self, value: u8) {
        if self.debug { println!("DALI light {} set search address low byte to {}", self.light_number, value); }
        self.search_address &= 0xffff00;
        self.search_address |= value as u32;
    }

    fn set_search_address_middle(&mut self, value: u8) {
        if self.debug { println!("DALI light {} set search address middle byte to {}", self.light_number, value); }
        self.search_address &= 0xff00ff;
        self.search_address |= (value as u32) << 8;
    }

    fn set_search_address_high(&mut self, value: u8) {
        if self.debug { println!("DALI light {} set search address high byte to {}", self.light_number, value); }
        self.search_address &= 0x00ffff;
        self.search_address |= (value as u32) << 16;
    }

    fn program_short_address(&mut self, short_address: u8) {
        if self.selected {
            if self.debug { println!("DALI light {} is selected, set short address to {}", self.light_number, short_address); }
            self.short_address = short_address;
        }
    }

}

impl DaliBusEmulator {
    pub fn new(bus_number: usize, light_count: usize, debug: bool) -> DaliBusEmulator {
        let mut lights: Vec<DaliLightEmulator> = Vec::new();

        for light_number in 0..light_count {
            lights.push(DaliLightEmulator::new(light_number, debug));
        }

        DaliBusEmulator { bus_number, lights: RefCell::new(lights), debug }
    }

    pub fn new_with_config(bus_config: &BusConfig, debug: bool) -> DaliBusEmulator {
        let mut lights: Vec<DaliLightEmulator> = Vec::new();

        for (light_number, channel) in bus_config.channels.iter().enumerate() {
            let mut group_mask = 0u16;

            for group in bus_config.groups.iter() {
                if group.members.iter().any(|short_address| *short_address == channel.short_address) {
                    group_mask |= 1 << group.group_address;
                }
            }

            lights.push(DaliLightEmulator::new_with_config(light_number, channel.short_address, group_mask, debug));
        }

        DaliBusEmulator { bus_number: bus_config.bus, lights: RefCell::new(lights), debug }
    }

    pub fn send_2_bytes(&self, b1: u8, b2: u8) -> DaliBusResult {
        if self.debug { println!("DALI Bus#{} send {:#02x},{:#02x}", self.bus_number, b1, b2); }

        let mut result = DaliBusResult::None;

        for dali_light in self.lights.borrow_mut().iter_mut() {
            result = match dali_light.receive_2_bytes(b1, b2) {
                Some(x) => match result {
                    DaliBusResult::None => DaliBusResult::Value8(x),
                    DaliBusResult::Value8(_) => DaliBusResult::ReceiveCollision,
                    DaliBusResult::ReceiveCollision => DaliBusResult::ReceiveCollision,
                    _ => result,
                },
                _ => result,
            }
        }

        if !self.debug { 
            // Emulate real time - bus speed is 1200bps, transaction is (2 bytes message + 1 byte reply = 30 bits (inc stop bits)) total of 1200/30 = 40 messages per second, so
            // each message is 1000/40 = 25 milliseconds 
            std::thread::sleep(std::time::Duration::from_millis(25));
        }

        result
    }
}

impl DaliControllerEmulator {
    pub fn try_new(config: &mut Config, debug: bool) -> Result<Box<dyn DaliController>, Box<dyn std::error::Error>> {
        let mut buses: Vec<DaliBusEmulator> = Vec::new();

        if config.buses.is_empty() {
            let bus_count: usize = Config::prompt_for_number("Number of DALI buses supported (1, 2 or 4)", &Some(1)).unwrap();
            let light_count = Config::prompt_for_number("Number of lights to emulate", &Some(3)).unwrap();

            for bus_number in 0..bus_count {
                config.buses.push(BusConfig::new(bus_number, BusStatus::Active));
                buses.push(DaliBusEmulator::new(bus_number, light_count, debug));
            }
        }
        else {
            for bus_config in config.buses.iter() {
                buses.push(DaliBusEmulator::new_with_config(bus_config, debug))
            }
        }

        Ok(Box::new(DaliControllerEmulator{ buses }))
    }
}

impl DaliController for DaliControllerEmulator {
    fn send_2_bytes(&mut self, bus: usize, b1: u8, b2: u8) -> Result<DaliBusResult, Box<dyn std::error::Error>> {
        if bus >= self.buses.len() {
            panic!("Send to invalid bus {}", bus);
        }

        Ok(self.buses[bus].send_2_bytes(b1, b2))
    }

    fn send_2_bytes_repeat(&mut self, bus: usize, b1: u8, b2: u8) -> Result<DaliBusResult, Box<dyn std::error::Error>> {
        self.send_2_bytes(bus, b1, b2)
    }

    fn get_bus_status(&mut self, _bus: usize) -> Result<BusStatus, Box<dyn std::error::Error>> {
        Ok(BusStatus::Active)
    }
}
