
use crate::dali_commands;

pub enum DaliBusResult {
    None,
    Collision,
    Value (u8),

    InvalidAddress,
    InvalidGroup,
    InvalidCommand,
}

pub trait DaliController {
    fn send_2_bytes(&self, bus: usize, b1: u8, b2: u8) -> DaliBusResult;
    fn send_2_bytes_repeat(&self, bus: usize, b1: u8, b2: u8) -> DaliBusResult;
}

pub struct DaliManager<'a> {
    controller: &'a dyn DaliController,
}

pub struct DaliBusIterator<'a> {
    manager: &'a DaliManager<'a>,
    bus: usize,
    previous_low_byte: Option<u8>,
    previous_mid_byte: Option<u8>,
    previous_high_byte: Option<u8>,
    short_address: u8,
}

pub enum DaliDeviceSelection {
    All,
    WithoutShortAddress,
    Address(u8),
}

impl<'a> DaliManager<'a> {
    pub fn new(controller: &'a dyn DaliController) -> DaliManager {
        DaliManager { controller }
    }

    #[allow(dead_code)]
    fn to_command_short_address(channel: u8) -> u8 {
        DaliManager::to_light_short_address(channel) | 0x01
    }

    #[allow(dead_code)]
    fn to_command_group_address(group: u8) -> u8 {
        DaliManager::to_light_group_address(group) | 0x01
    }

    fn to_light_short_address(channel: u8) -> u8 {
        if channel < 64 { channel << 1 } else { panic!("Invalid DALI short address {}", channel) }
    }

    fn to_light_group_address(group: u8) -> u8 {
        if group < 16 { 0x80 | (group << 1 ) } else { panic!("Invlid DALI group# {}", group) }
    }

    pub async fn set_light_brightness_async(&mut self, bus: usize, channel: u8, value: u8) -> DaliBusResult {
        self.controller.send_2_bytes(bus, DaliManager::to_light_short_address(channel), value)
    }

    pub fn set_light_brightness(&self, bus: usize, short_address: u8, level: u8) -> DaliBusResult {
        self.controller.send_2_bytes(bus, DaliManager::to_light_short_address(short_address), level)
    }

    pub async fn set_group_brightness_async(&mut self, bus: usize, group: u8, value: u8) -> DaliBusResult {
        self.controller.send_2_bytes(bus, DaliManager::to_light_group_address(group), value)
    }

    pub fn set_group_brightness(&self, bus: usize, group_address: u8, level: u8) -> DaliBusResult {
        self.controller.send_2_bytes(bus, DaliManager::to_light_group_address(group_address), level)
    }

    pub fn send_command_to_address(&self, bus: usize, command: u16, short_address: u8, repeat: bool) -> DaliBusResult {
        if command > 0xff { return DaliBusResult::InvalidCommand }
        if short_address >= 64 { return DaliBusResult::InvalidAddress }

        let b1 = DaliManager::to_command_short_address(short_address);
        let b2 = (command & 0xff) as u8;

        if repeat {
            self.controller.send_2_bytes_repeat(bus, b1, b2)
        } else {
            self.controller.send_2_bytes(bus, b1, b2)
        }
    }

    #[allow(dead_code)]
    pub fn send_command_to_group(&self, bus: usize, command: u16, group: u8, repeat: bool) -> DaliBusResult {
        if command > 0xff { return DaliBusResult::InvalidCommand }
        if group >= 64 { return DaliBusResult::InvalidGroup }

        let b1 = DaliManager::to_command_group_address(group);
        let b2 = (command & 0xff) as u8;

        if repeat {
            self.controller.send_2_bytes_repeat(bus, b1, b2)
        } else {
            self.controller.send_2_bytes(bus, b1, b2)
        }
    }

    fn broadcast_command(&self, bus: usize, command: u16, parameter: u8, repeat: bool) -> DaliBusResult {
        let b1 = if (command & 0x100) != 0 { (command & 0xff) as u8 } else { 0xff };
        let b2 = if (command & 0x100) != 0 { parameter } else { command as u8 };

        if repeat {
            self.controller.send_2_bytes_repeat(bus, b1, b2)
        } else {
            self.controller.send_2_bytes(bus, b1, b2)
        }
    }

    /// Get iterator over devices on the DALI bus. 
    /// This can be used to assign short address.
    /// 
    /// # example
    /// ```
    ///     let dali_bus_iterator = manager.get_dali_bus_iter(0, DaliDeviceSelection::All);
    /// 
    ///     for short_address in dali_bus_iterator {
    ///         dali_bus_iterator.program_short_address(short_address);
    ///     }
    /// ```
    /// 
    pub fn get_dali_bus_iter(&self, bus: usize, selection: DaliDeviceSelection) -> DaliBusIterator {
        let parameter = match selection {
            DaliDeviceSelection::All => 0,
            DaliDeviceSelection::WithoutShortAddress => 0xff,
            DaliDeviceSelection::Address(a) => a << 1 | 1
        };

        self.broadcast_command(bus, dali_commands::DALI_INITIALISE, parameter, true);
        self.broadcast_command(bus, dali_commands::DALI_RANDOMISE, 0, true);

        DaliBusIterator {
            bus,
            manager: self,
            previous_low_byte: None,
            previous_mid_byte: None,
            previous_high_byte: None,
            short_address: 0,
        }
    }

    pub fn program_short_address(&self, bus: usize, short_address: u8) {
        if short_address >= 64 { panic!("Invalid short address") }

        self.broadcast_command(bus, dali_commands::DALI_PROGRAM_SHORT_ADDRESS, (short_address << 1) | 0x01, false);
        self.broadcast_command(bus, dali_commands::DALI_WITHDRAW, 0, false);
    }

    pub fn remove_from_group(&self, bus: usize, group_address:u8, short_address:u8) -> DaliBusResult {
        self.send_command_to_address(bus, dali_commands::DALI_REMOVE_FROM_GROUP0+(group_address as u16), short_address, true)
    }

    pub fn add_to_group(&self, bus: usize, group_address:u8, short_address:u8) -> DaliBusResult {
        self.send_command_to_address(bus, dali_commands::DALI_ADD_TO_GROUP0+(group_address as u16), short_address, true)
    }
}

impl<'a> DaliBusIterator<'a> {
    fn diff_value(previous: Option<u8>, new: u8) -> Option<u8> {
        match previous {
            None => Some(new),
            Some(previous) => if previous != new { Some(new) } else { None }
        }
    }

    fn send_search_address(&mut self, search_address: u32) -> DaliBusResult {
        let low = DaliBusIterator::diff_value(self.previous_low_byte, search_address as u8);
        let mid = DaliBusIterator::diff_value(self.previous_mid_byte, (search_address >> 8) as u8);
        let high = DaliBusIterator::diff_value(self.previous_high_byte, (search_address >> 16) as u8);

        self.previous_low_byte = Some(search_address as u8);
        self.previous_mid_byte = Some((search_address >> 8) as u8);
        self.previous_high_byte = Some((search_address >> 16) as u8);

        if let Some(low) = low { self.manager.broadcast_command(self.bus, dali_commands::DALI_SEARCHADDRL, low, false); }
        if let Some(mid) = mid { self.manager.broadcast_command(self.bus, dali_commands::DALI_SEARCHADDRM, mid, false); }
        if let Some(high) = high { self.manager.broadcast_command(self.bus, dali_commands::DALI_SEARCHADDRH, high, false); }

        DaliBusResult::None
    }

    fn is_random_address_le(& mut self, retry: u8) -> bool {
        match self.manager.broadcast_command(self.bus, dali_commands::DALI_COMPARE, 0, false) {
            DaliBusResult::None => if retry == 0 { false } else { self.is_random_address_le(retry-1) },               // No answer
            DaliBusResult::Collision => true,           // More than one yes reply
            DaliBusResult::Value(0xff) => true,         // Yes answer
            _ => panic!("Unexpected replyf for DALI compare command"),
        }
    }
}

impl<'a> Iterator for DaliBusIterator<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        // Find next device by trying to match its random address
        let mut search_address = 0x00800000;        // Start in half the range (24 bits)
        let mut delta = 0x00400000;

        while delta > 0 {
            self.send_search_address(search_address);

            let random_address_le = self.is_random_address_le(0);   // On real hardware consider changing this to 1 retry

            if random_address_le {
                search_address -= delta;
            } else {
                search_address += delta;
            }

            delta >>= 1; 
        }

        self.send_search_address(search_address);
        if !self.is_random_address_le(0) {
            search_address += 1;
            self.send_search_address(search_address);
        }

        if search_address > 0xffffff {
            self.manager.broadcast_command(self.bus, dali_commands::DALI_TERMINATE, 0, false);
            None
        } else {
            let short_address = self.short_address;
            self.short_address += 1;
            Some(short_address) 
        }
    }
}
