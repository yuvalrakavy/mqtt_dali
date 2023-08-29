use log::{debug, trace, info};
use thiserror::Error;
use crate::dali_commands;
use crate::config_payload::{BusStatus, BusConfig, Group, Channel};
use crate::command_payload::LightStatus;
use std::{thread::sleep, time::Duration};

#[derive(Debug, Clone, Copy)]
pub enum DaliBusResult {
    None,
    ReceiveCollision,
    TransmitCollision,
    Value8 (u8),
    Value16 (u16),
    Value24 (u32),
}

#[derive(Debug, Error)]
pub enum DaliManagerError {
    #[error("Invalid short address: {0}")]
    ShortAddress(u8),

    #[error("Invalid group address: {0}")]
    GroupAddress(u8),

    #[error("Invalid command: {0}")]
    Command(u16),

    #[error("Unexpected light status {0:?}")]
    UnexpectedStatus(DaliBusResult),

    #[error("Pattern (regex) error: {0}")]
    RegExError(#[from] #[source] regex::Error),

    #[error("DALI interface error: {0:?}")]
    DaliInterfaceError(#[source] Box<dyn std::error::Error>),

    #[error("Add to group failed (light {0} group {1})")]
    GroupAddFailed(u8, u8),

    #[error("Remove from group failed (light {0} group {1})")]
    GroupRemoveFailed(u8, u8),

    #[error("No value was returned from the DALI bus")]
    NoResult,
}

pub type Result<T> = std::result::Result<T, DaliManagerError>;
pub type FindDeviceProgress = Box<dyn Fn(u8, u8)>;
pub type MatchGroupProgress = Box<dyn Fn(MatchGroupAction, &str)>;

pub trait DaliController {
    fn send_2_bytes(&mut self, bus: usize, b1: u8, b2: u8) -> Result<DaliBusResult>;
    fn send_2_bytes_repeat(&mut self, bus: usize, b1: u8, b2: u8) -> Result<DaliBusResult>;
    fn get_bus_status(&mut self, bus: usize) -> Result<BusStatus>;
}

pub struct DaliManager<'a> {
    pub controller: &'a mut dyn DaliController,
}

pub struct DaliBusIterator {
    progress: Option<FindDeviceProgress>,
    bus: usize,
    previous_low_byte: Option<u8>,
    previous_mid_byte: Option<u8>,
    previous_high_byte: Option<u8>,
    short_address: u8,
    terminate: bool,
}

pub enum DaliDeviceSelection {
    All,
    WithoutShortAddress,
    Address(u8),
}

pub enum MatchGroupAction<'a> {
    AddMember(&'a str, &'a str),
    RemoveMember(&'a str, &'a str),
}

impl<'manager> DaliManager<'manager> {
    pub fn new(controller: &'manager mut dyn DaliController) -> DaliManager {
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
        if group < 16 { 0x80 | (group << 1 ) } else { panic!("Invalid DALI group# {}", group) }
    }

    pub async fn set_light_brightness_async(&mut self, bus: usize, short_address: u8, value: u8) -> Result<DaliBusResult> {
        self.controller.send_2_bytes(bus, DaliManager::to_light_short_address(short_address), value)
    }

    pub fn set_light_brightness(&mut self, bus: usize, short_address: u8, level: u8) -> Result<DaliBusResult> {
        self.controller.send_2_bytes(bus, DaliManager::to_light_short_address(short_address), level)
    }

    pub async fn set_group_brightness_async(&mut self, bus: usize, group: u8, value: u8) -> Result<DaliBusResult> {
        self.controller.send_2_bytes(bus, DaliManager::to_light_group_address(group), value)
    }

    pub fn set_group_brightness(&mut self, bus: usize, group_address: u8, level: u8) -> Result<DaliBusResult> {
        self.controller.send_2_bytes(bus, DaliManager::to_light_group_address(group_address), level)
    }

    pub fn send_command_to_address(&mut self, bus: usize, command: u16, short_address: u8, repeat: bool) -> Result<DaliBusResult> {
        if command > 0xff { return Err(DaliManagerError::Command(command)) }
        if short_address >= 64 { return Err(DaliManagerError::ShortAddress(short_address)); }

        let b1 = DaliManager::to_command_short_address(short_address);
        let b2 = (command & 0xff) as u8;

        if repeat {
            self.controller.send_2_bytes_repeat(bus, b1, b2)
        } else {
            self.controller.send_2_bytes(bus, b1, b2)
        }
    }

    pub fn send_command_to_address_and_get_byte(&mut self, bus: usize, command: u16, short_address: u8, repeat: bool) -> Result<u8> {
        let mut retry_count = 4;

        loop {
            let result = self.send_command_to_address(bus, command, short_address, repeat)?;

            if let DaliBusResult::Value8(b) = result {
                break Ok(b);
            }

            retry_count -= 1;
            if retry_count == 0 {
                break Err(DaliManagerError::NoResult);
            }

            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    #[allow(dead_code)]
    pub fn send_command_to_group(&mut self, bus: usize, command: u16, group_address: u8, repeat: bool) -> Result<DaliBusResult> {
        if command > 0xff { return Err(DaliManagerError::Command(command)) }
        if group_address >= 64 { return Err(DaliManagerError::GroupAddress(group_address)); }

        let b1 = DaliManager::to_command_group_address(group_address);
        let b2 = (command & 0xff) as u8;

        if repeat {
            self.controller.send_2_bytes_repeat(bus, b1, b2)
        } else {
            self.controller.send_2_bytes(bus, b1, b2)
        }
    }

    fn is_collision(result: &DaliBusResult) -> bool {
        matches!(result, &DaliBusResult::ReceiveCollision | &DaliBusResult::TransmitCollision)
    }

    fn broadcast_command(&mut self, bus: usize, command: u16, parameter: u8, repeat: bool, description: &str) -> Result<DaliBusResult> {
        let b1 = if (command & 0x100) != 0 { (command & 0xff) as u8 } else { 0xff };
        let b2 = if (command & 0x100) != 0 { parameter } else { command as u8 };
        let mut collision_count = 0;

        debug!("Send: {}", description);

        loop {
            let result = if repeat {
                self.controller.send_2_bytes_repeat(bus, b1, b2)?
            } else {
                self.controller.send_2_bytes(bus, b1, b2)?
            };

            if !DaliManager::is_collision(&result) {
                break Ok(result)
            }
            else {
                collision_count += 1;
                if collision_count > 300 {
                    break Err(DaliManagerError::UnexpectedStatus(DaliBusResult::TransmitCollision))
                }
            }
        }
    }

    fn broadcast_command_allow_collision(&mut self, bus: usize, command: u16, parameter: u8, repeat: bool, description: &str) -> Result<DaliBusResult> {
        let b1 = if (command & 0x100) != 0 { (command & 0xff) as u8 } else { 0xff };
        let b2 = if (command & 0x100) != 0 { parameter } else { command as u8 };

        debug!("Send (expect collision): {}", description);

        if repeat {
            self.controller.send_2_bytes_repeat(bus, b1, b2)
        } else {
            self.controller.send_2_bytes(bus, b1, b2)
        }
     }

    pub fn program_short_address(&mut self, bus: usize, short_address: u8) -> Result<()> {
        if short_address >= 64 { panic!("Invalid short address") }

        debug!("Program short address: {short_address}");

        self.broadcast_command(bus, dali_commands::DALI_PROGRAM_SHORT_ADDRESS, (short_address << 1) | 0x01, false, &format!("Program short address {}", short_address))?;

        // loop {
        //     self.broadcast_command(bus, dali_commands::DALI_PROGRAM_SHORT_ADDRESS, (short_address << 1) | 0x01, false, &format!("Program short address {}", short_address))?;

        //     let actual_short_address = self.query_short_address(bus)? >> 1;
        //     debug!("Actual short address {actual_short_address}");

        //     if actual_short_address == short_address {
        //         break;
        //     }
        // }

        loop {
            let status = self.broadcast_command(bus, dali_commands::DALI_WITHDRAW, 0, false, "Withdraw")?;

            if let DaliBusResult::None = status {
                break;
            }

            debug!("Withdraw status: {:?} - retry", status);
        }

        Ok(())
    }

    pub fn set_dtr(&mut self, bus: usize, value: u8) -> Result<DaliBusResult> {
        self.broadcast_command(bus, dali_commands::DALI_DATA_TRANSFER_REGISTER0, value, false, &format!("Set DTR to {}", value))
    }

    pub fn query_group_membership(&mut self, bus: usize, short_address: u8) -> Result<u16> {
        let groups_0to7 = self.send_command_to_address_and_get_byte(bus, dali_commands::DALI_QUERY_GROUPS_0_7, short_address, false)?;
        let groups_8to15 = self.send_command_to_address_and_get_byte(bus, dali_commands::DALI_QUERY_GROUPS_8_15, short_address, false)?;

        let membership = ((groups_8to15 as u16) << 8) | (groups_0to7 as u16);
        info!("QueryGroupMembership bus {}/light {} mask {:04x}", bus, short_address, membership);

        Ok(membership)
    }

    pub fn is_group_member(&mut self, bus: usize, short_address: u8, group_address: u8) -> Result<bool> {
        let membership_mask = self.query_group_membership(bus, short_address)?;

        let is_member = (1 << group_address) & membership_mask != 0;
        info!("IsGroupMember light {} group {} mask {:04x} => {}", short_address, group_address, membership_mask, is_member);
        Ok(is_member)
    }

    pub fn remove_from_group(&mut self, bus: usize, group_address:u8, short_address:u8) -> Result<DaliBusResult> {
        info!("Remove light {bus}/{short_address} from group {group_address}", short_address=short_address, group_address=group_address);
        self.send_command_to_address(bus, dali_commands::DALI_REMOVE_FROM_GROUP0+(group_address as u16), short_address, true)
    }

    pub fn remove_from_group_and_verify(&mut self, bus: usize, group_address:u8, short_address:u8) -> Result<DaliBusResult> {
        let mut retry_count = 3;

        loop {
            self.remove_from_group(bus, group_address, short_address)?;

            if !self.is_group_member(bus, short_address, group_address)? {
                break Ok(DaliBusResult::None)
            } else {
                trace!("Remove light {short_address} from group {group_address} failed, retry again");

                retry_count -= 1;

                if retry_count == 0 {
                    break Err(DaliManagerError::GroupRemoveFailed(short_address, group_address))
                }

                sleep(Duration::from_millis(200));
            }
        }
    }

    pub fn add_to_group(&mut self, bus: usize, group_address:u8, short_address:u8) -> Result<DaliBusResult> {
        self.send_command_to_address(bus, dali_commands::DALI_ADD_TO_GROUP0+(group_address as u16), short_address, true)
    }

    pub fn add_to_group_and_verify(&mut self, bus: usize, group_address:u8, short_address:u8) -> Result<DaliBusResult> {
        let mut retry_count = 8;

        loop {
            self.add_to_group(bus, group_address, short_address)?;

            if self.is_group_member(bus, short_address, group_address)? {
                break Ok(DaliBusResult::None)
            } else {
                println!("Add light {short_address} to group {group_address} failed, retry again");

                retry_count -= 1;

                if retry_count == 0 {
                    break Err(DaliManagerError::GroupAddFailed(short_address, group_address))
                }

                sleep(Duration::from_millis(200));
            }
        }
    }

        // Change one short address to another.
    // If new address is 0xff, then short address is removed and the device should be found again when doing bus commissioning
    //
    pub fn change_short_address(&mut self, bus_config: &mut BusConfig, existing_address: u8, new_address: u8) -> Result<DaliBusResult> {
        if existing_address >= 64 { panic!("Invalid existing short address") }
        if new_address >= 64 && new_address != 0xff { panic!("Invalid new short address") }
        let bus = bus_config.bus;

        self.set_dtr(bus, new_address)?;
        self.send_command_to_address(bus, dali_commands::DALI_SET_SHORT_ADDRESS, existing_address, true)?;

        let description = if let Some(existing_channel) = bus_config.remove_channel(existing_address) {
            existing_channel.description
        }
        else {
            format!("Light {}", new_address)
        };

        if new_address != 0xff {
            bus_config.channels.push(Channel { description, short_address: new_address });
        }

        Ok(DaliBusResult::None)
    }

    pub fn remove_short_address(&mut self, bus_config: &mut BusConfig, existing_address: u8) -> Result<DaliBusResult> {
        let bus = bus_config.bus;
        let groups = self.query_group_membership(bus, existing_address)?;

        for group_address in 0..16 {
            if (groups & (1 << group_address)) != 0 {
                self.remove_from_group(bus, group_address, existing_address)?;
                bus_config.remove_from_group(group_address, existing_address);
            }
        }

        self.change_short_address(bus_config, existing_address, 0xff)
    }

    pub fn match_group(&mut self, bus_config: &mut BusConfig, group_address: u8, light_name_pattern: &str, progress: Option<MatchGroupProgress>) -> Result<DaliBusResult> {
        let re = regex::Regex::new(light_name_pattern)?;
        let group = bus_config.groups.iter_mut().find(|g| g.group_address == group_address);

        // Create group if not found
        if group.is_none() {
            bus_config.groups.push( Group { description: format!("New-Group {}", group_address), group_address, members: Vec::new()});
        }

        let group = bus_config.groups.iter_mut().find(|g| g.group_address == group_address).unwrap();

        for light in bus_config.channels.iter() {
            if re.is_match(&light.description) {
                // If this light is not member of the group, add it
                if !group.members.contains(&light.short_address) {
                    if let Some(progress) = &progress {
                        progress(MatchGroupAction::AddMember(
                            &format!("{} ({})", light.description, light.short_address),
                            &format!("{} ({})", group.description, group.group_address)
                        ), light_name_pattern)
                    }

                    trace!("Light {}: {} matches {} - added to group {}", light.short_address, light.description, light_name_pattern, group_address);
                    self.add_to_group_and_verify(bus_config.bus, group_address, light.short_address)?;
                    group.members.push(light.short_address);
                }
            } else {
                // If this light is member of the group, remove it since its name does not match the pattern
                if let Some(index) = group.members.iter().position(|short_address|  *short_address == light.short_address) {
                    if let Some(progress) = &progress {
                        progress(MatchGroupAction::RemoveMember(
                            &format!("{} ({})", light.description, light.short_address),
                            &format!("{} ({})", group.description, group.group_address)
                        ), light_name_pattern)
                    }
                    trace!("Light {}: {} does not match {} - removed from group {}", light.short_address, light.description, light_name_pattern, group_address);
                    self.remove_from_group_and_verify(bus_config.bus, group_address, light.short_address)?;
                    group.members.remove(index);
                }
            }
        }

        Ok(DaliBusResult::None)
    }

    pub fn query_light_status(&mut self, bus: usize, short_address: u8) -> Result<LightStatus> {
        match self.send_command_to_address(bus, dali_commands::DALI_QUERY_STATUS, short_address, false) {
            Ok(DaliBusResult::Value8(v)) => Ok(LightStatus::from(v)),
            Ok(bus_result) => Err(DaliManagerError::UnexpectedStatus(bus_result)),
            Err(e) => Err(e),
        }
    }
}

impl DaliBusIterator {
    pub fn new(dali_manager: &mut DaliManager, bus: usize, selection: DaliDeviceSelection, progress: Option<FindDeviceProgress>) -> Result<DaliBusIterator>  {
        let parameter = match selection {
            DaliDeviceSelection::All => 0,
            DaliDeviceSelection::WithoutShortAddress => 0xff,
            DaliDeviceSelection::Address(a) => a << 1 | 1
        };

        dali_manager.broadcast_command(bus, dali_commands::DALI_TERMINATE, 0, true, "Terminate")?;
        std::thread::sleep(std::time::Duration::from_millis(300));

        dali_manager.broadcast_command(bus, dali_commands::DALI_INITIALISE, parameter, true, "Initialize")?;
        std::thread::sleep(std::time::Duration::from_millis(400));
        dali_manager.broadcast_command(bus, dali_commands::DALI_RANDOMISE, 0, true, "Randomize")?;
        std::thread::sleep(std::time::Duration::from_millis(250));

        Ok(DaliBusIterator {
            bus,
            progress,

            previous_low_byte: None,
            previous_mid_byte: None,
            previous_high_byte: None,
            short_address: 0,
            terminate: false
        })
    }

    fn diff_value(previous: Option<u8>, new: u8) -> Option<u8> {
        match previous {
            None => Some(new),
            Some(previous) => if previous != new { Some(new) } else { None }
        }
    }

    fn send_search_address(&mut self, dali_manager: &mut DaliManager, search_address: u32) -> Result<DaliBusResult> {
        let low = DaliBusIterator::diff_value(self.previous_low_byte, search_address as u8);
        let mid = DaliBusIterator::diff_value(self.previous_mid_byte, (search_address >> 8) as u8);
        let high = DaliBusIterator::diff_value(self.previous_high_byte, (search_address >> 16) as u8);

        self.previous_low_byte = Some(search_address as u8);
        self.previous_mid_byte = Some((search_address >> 8) as u8);
        self.previous_high_byte = Some((search_address >> 16) as u8);

        if let Some(low) = low {
             dali_manager.broadcast_command(self.bus, dali_commands::DALI_SEARCHADDRL, low, false, &format!("Set search address low: {}", low))?;
        }
        if let Some(mid) = mid {
             dali_manager.broadcast_command(self.bus, dali_commands::DALI_SEARCHADDRM, mid, false, &format!("Set search address mid: {}", mid))?;
        }
        if let Some(high) = high {
             dali_manager.broadcast_command(self.bus, dali_commands::DALI_SEARCHADDRH, high, false, &format!("Set search address high: {}", high))?;
        }

        Ok(DaliBusResult::None)
    }

    fn is_random_address_le(&mut self, dali_manager: &mut DaliManager, retry: u8) -> Result<bool> {
        match dali_manager.broadcast_command_allow_collision(self.bus, dali_commands::DALI_COMPARE, 0, false, "Is random address le") {
            Ok(DaliBusResult::None) => if retry == 0 { Ok(false) } else { self.is_random_address_le(dali_manager, retry-1) },               // No answer
            Ok(_) => Ok(true),    // More than one yes reply
            Err(e) => Err(e),
        }
    }

    pub fn find_next_device(&mut self, dali_manager: &mut DaliManager) -> Result<Option<u8>> {
        // Find next device by trying to match its random address
        let mut search_address = 0x00800000;        // Start in half the range (24 bits)
        let mut delta = 0x00400000;
        let mut step = 0;

        if self.terminate {
            dali_manager.broadcast_command(self.bus, dali_commands::DALI_TERMINATE, 0, false, "terminate")?;
            return Ok(None);
        }

        while delta > 0 {
            trace!("find_next_device: Send search address {}", search_address);

            self.send_search_address(dali_manager, search_address)?;

            let random_address_le = self.is_random_address_le(dali_manager, 2)?;   // On real hardware consider changing this to 1 retry

            if random_address_le {
                search_address -= delta;
            } else {
                search_address += delta;
            }

            delta >>= 1;

            if let Some(progress) = self.progress.as_ref() {
                progress(self.short_address, step);
            }

            step += 1;
        }

        self.send_search_address(dali_manager, search_address)?;
        if !self.is_random_address_le(dali_manager, 2)? {
            search_address += 1;
            self.send_search_address(dali_manager, search_address)?;
            self.is_random_address_le(dali_manager, 2)?;
        }

        if search_address > 0xffffff {
            debug!("No more devices found!");
            dali_manager.broadcast_command(self.bus, dali_commands::DALI_TERMINATE, 0, false, "terminate")?;
            Ok(None)
        } else {
            debug!("Found light at long address {}", search_address);
            let short_address = self.short_address;
            self.short_address += 1;
            Ok(Some(short_address)) 
        }
    }

    pub fn terminate(&mut self) {
        self.terminate = true;
    }

}

