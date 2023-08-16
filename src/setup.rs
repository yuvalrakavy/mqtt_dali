use crate::dali_manager::{MatchGroupAction, DaliBusResult};
use crate::Config;
use crate::{
    config_payload::{BusConfig, BusStatus, Channel, DaliConfig, Group},
    dali_manager::{DaliBusIterator, DaliDeviceSelection, DaliManager},
};
use log::{log_enabled, Level::Trace};
use std::{fmt, fs::File, io, io::Write, path::Path};

#[derive(Debug)]
pub enum SetupError {
    JsonError(serde_json::Error),
    IoError(std::io::Error),
    UserQuit,
}

impl From<serde_json::Error> for SetupError {
    fn from(err: serde_json::Error) -> SetupError {
        SetupError::JsonError(err)
    }
}

impl From<std::io::Error> for SetupError {
    fn from(err: std::io::Error) -> SetupError {
        SetupError::IoError(err)
    }
}

impl std::fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetupError::JsonError(e) => write!(f, "Json error: {}", e),
            SetupError::IoError(e) => write!(f, "IO error: {}", e),
            SetupError::UserQuit => write!(f, "User quit"),
        }
    }
}

impl std::error::Error for SetupError {}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} - {}", self.short_address, self.description)
    }
}

impl fmt::Display for BusStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BusStatus::Active => write!(f, "Active"),
            BusStatus::NoPower => write!(f, "No power"),
            BusStatus::Overloaded => write!(f, "Overloaded"),
            BusStatus::Unknown => write!(f, "Unknown status"),
        }
    }
}

#[derive(Debug)]
pub enum SetupAction {
    Quit,
    Start(DaliConfig),
}

impl BusConfig {
    const CHANNELS_PER_LINE: usize = 4;

    pub fn new(bus_number: usize, status: BusStatus) -> BusConfig {
        let description = format!("Bus-{}", bus_number + 1);

        BusConfig {
            description,
            status,
            bus: bus_number,
            channels: Vec::new(),
            groups: Vec::new(),
        }
    }

    pub fn find_member(&self, channel: u8) -> Option<&Channel> {
        self.channels.iter().find(|c| c.short_address == channel)
    }

    fn get_channel_index(&self, short_address: u8) -> Option<usize> {
        self.channels
            .iter()
            .position(|channel| channel.short_address == short_address)
    }

    pub fn remove_channel(&mut self, short_address: u8) -> Option<Channel> {
        if let Some(index) = self.get_channel_index(short_address) {
            Some(self.channels.remove(index))
        } else {
            None
        }
    }

    fn get_group_index(&self, group_address: u8) -> Option<usize> {
        self.groups
            .iter()
            .position(|group| group.group_address == group_address)
    }

    fn get_unused_short_address(&self) -> Option<u8> {
        (1..64u8).find(|short_address| self.get_channel_index(*short_address).is_none())
    }

    fn get_unused_group_address(&self) -> Option<u8> {
        (1..16u8).find(|group_address| self.get_group_index(*group_address).is_none())
    }

    pub fn remove_from_group(&mut self, group_address: u8, short_address: u8) -> bool {
        if let Some(group) = self
            .groups
            .iter_mut()
            .find(|g| g.group_address == group_address)
        {
            if let Some(index) = group.members.iter().position(|m| *m == short_address) {
                group.members.remove(index);
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    fn display_channels(&self) {
        println!("  Channels:");
        for i in 0..self.channels.len() {
            if i % BusConfig::CHANNELS_PER_LINE == 0 {
                print!("    ");
            }

            print!("{:18}", self.channels[i].to_string());

            if (i + 1) % BusConfig::CHANNELS_PER_LINE == 0 {
                println!();
            }
        }

        if self.channels.len() % BusConfig::CHANNELS_PER_LINE != 0 {
            println!();
        }
    }

    fn display_group(&self, group: &Group) {
        println!("    {} ({}):", group.group_address, group.description);
        for i in 0..group.members.len() {
            if i % BusConfig::CHANNELS_PER_LINE == 0 {
                print!("      ");
            }

            if let Some(channel) = self.find_member(group.members[i]) {
                print!("{:18}", channel.to_string())
            } else {
                print!("Missing {:10}", self.channels[i])
            }

            if (i + 1) % BusConfig::CHANNELS_PER_LINE == 0 {
                println!();
            }
        }

        if group.members.len() % BusConfig::CHANNELS_PER_LINE != 0 {
            println!();
        }
    }

    pub fn display(&self) {
        println!(
            "{}: DALI bus: {} ({})",
            self.bus + 1,
            self.description,
            self.status
        );

        if self.channels.is_empty() {
            println!("  No channels");
        } else {
            self.display_channels();
        }

        if self.groups.is_empty() {
            println!("  No groups");
        } else {
            println!("  groups:");
            for group in self.groups.iter() {
                self.display_group(group);
            }
        }
    }

    fn do_query_light(&self, dali_manager: &mut DaliManager, short_address: u8) {
        let status = dali_manager.query_light_status(self.bus, short_address);

        print!("{:2}: ", short_address);

        let status = match status {
            Ok(status) => Some(status),
            Err(_) => None,
        };

        if let Some(status) = status {
            print!("{} ", status);

            let group_mask = dali_manager.query_group_membership(self.bus, short_address);

            if let Ok(group_mask) = group_mask {
                print!("groups: {:#06x}", group_mask);

                let mut mask = 1u16;
                for group_number in 0..16 {
                    let group = self.groups.iter().find(|g| g.group_address == group_number);

                    if (group_mask & mask) != 0 {
                        if let Some(group) = group {
                            print!(" {}", group.description);
                        } else {
                            // Light reports membership, but it is not reflected in the configuration
                            print!(" _Group_{}", group_number);
                        }
                    }

                    mask <<= 1;
                }
            } else {
                print!("Error getting groups");
            }
        } else {
            print!(" not found");
        }

        println!();
    }

    fn query_bus(&self, dali_manager: &mut DaliManager) {
        for light in self.channels.iter() {
            self.do_query_light(dali_manager, light.short_address);
        }
    }
}

impl DaliConfig {
    pub fn new(name: &str) -> DaliConfig {
        DaliConfig {
            name: name.to_owned(),
            buses: Vec::new(),
        }
    }

    pub fn display(&self) {
        println!("Controller {}", self.name);

        for bus_number in 0..self.buses.len() {
            self.buses[bus_number].display()
        }
    }

    pub fn interactive_new() -> Result<DaliConfig, Box<dyn std::error::Error>> {
        let controller_name = Setup::prompt_for_string("Controller name", None)?;

        Ok(DaliConfig::new(&controller_name))
    }

    fn update_bus_status(
        &mut self,
        dali_manager: &mut DaliManager,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for (bus_number, bus) in self.buses.iter_mut().enumerate() {
            bus.status = dali_manager.controller.get_bus_status(bus_number)?;
        }

        Ok(())
    }
}

pub struct Setup {}

impl Setup {
    pub fn assign_addresses(
        config: &Config,
        mut dali_config: DaliConfig,
        dali_manager: &mut DaliManager,
        bus_number: usize,
    ) -> Result<DaliConfig, Box<dyn std::error::Error>> {
        //let bus_config = &mut dali_config.buses[bus_number];

        loop {
            let default_assign = if dali_config.buses[bus_number].channels.is_empty() {
                Some("a")
            } else {
                Some("b")
            };
            let command = Setup::prompt_for_string("Assign short addresses - a:All, m:missing, =:set address, #:change light's address, -:remove address, d:change light's description, b:back", default_assign)?;

            if let Some(command) = command.chars().next() {
                match command {
                    'b' => return Ok(dali_config),
                    '=' => {
                        let short_address = loop {
                            let default_short_address =
                                dali_config.buses[bus_number].get_unused_short_address();
                            let short_address = Setup::prompt_for_short_address(
                                "Short address",
                                default_short_address,
                            )?;

                            if dali_config.buses[bus_number]
                                .get_channel_index(short_address)
                                .is_none()
                            {
                                break short_address;
                            }

                            println!("Short address is already used");
                        };

                        let default_description = format!("Light {}", short_address);
                        let description =
                            Setup::prompt_for_string("Description", Some(&default_description))?;

                        dali_config.buses[bus_number].channels.push(Channel {
                            description,
                            short_address,
                        });
                        config.save(&dali_config)?;
                    }
                    '-' => {
                        if let Ok(short_address) =
                            Setup::prompt_for_number::<u8>("Remove address", None)
                        {
                            dali_manager
                                .remove_short_address(
                                    &mut dali_config.buses[bus_number],
                                    short_address,
                                )
                                .unwrap_or_else(|e| {
                                    println!("Error when removing address: {}", e);
                                    DaliBusResult::None
                                });
                        }
                    }
                    'd' => {
                        if let Ok(short_address) =
                            Setup::prompt_for_number::<u8>("Change description of address", None)
                        {
                            if let Some(index) =
                                dali_config.buses[bus_number].get_channel_index(short_address)
                            {
                                let new_description =
                                    Setup::prompt_for_string("Description", None)?;
                                dali_config.buses[bus_number].channels[index].description =
                                    new_description;
                                config.save(&dali_config)?;
                            } else {
                                println!("No channel with this address found");
                            }
                        }
                    }
                    'a' => {
                        if !dali_config.buses[bus_number].channels.is_empty()
                            && !Setup::prompt_for_yes_no(
                                "This will erase all existing addresses. Are you sure?",
                                false,
                            )?
                        {
                            continue;
                        }

                        let mut count = 0;
                        let prompt_for_each = Setup::prompt_for_string(
                            "Assign all -  a:auto, p:prompt for short-address/description",
                            Some("a"),
                        )?;
                        let prompt_for_each = !prompt_for_each.starts_with('a');

                        let mut dali_bus_iterator = DaliBusIterator::new(
                            dali_manager,
                            bus_number,
                            DaliDeviceSelection::All,
                            if log_enabled!(Trace) {
                                None
                            } else {
                                Some(Box::new(|n, s| {
                                    print!("\r{:2} [{:23}]", n, "*".repeat(s as usize + 1));
                                    io::stdout().flush().unwrap();
                                }))
                            },
                        )
                        .expect("Error while initializing DALI bus iteration");
                        dali_config.buses[bus_number].channels = Vec::new();
                        dali_config.buses[bus_number].groups = Vec::new();

                        while dali_bus_iterator.find_next_device(dali_manager)?.is_some() {
                            if !log_enabled!(Trace) {
                                println!();
                            }

                            let default_short_address =
                                dali_config.buses[bus_number].get_unused_short_address();

                            let short_address = match default_short_address {
                                Some(default_short_address) if !prompt_for_each => {
                                    default_short_address
                                }
                                _ => loop {
                                    let short_address = Setup::prompt_for_short_address(
                                        "Short address",
                                        default_short_address,
                                    )?;
                                    if dali_config.buses[bus_number]
                                        .get_channel_index(short_address)
                                        .is_none()
                                    {
                                        break short_address;
                                    }
                                    println!("Short address is already used");
                                },
                            };
                            let default_description = format!("Light {}", short_address);

                            let description = if prompt_for_each {
                                Setup::prompt_for_string("Description", Some(&default_description))?
                            } else {
                                default_description
                            };

                            if !prompt_for_each {
                                println!(
                                    "     assigning address {} to {}",
                                    short_address, description
                                );
                            }

                            dali_manager
                                .program_short_address(bus_number, short_address)
                                .unwrap_or_else(|e| {
                                    println!("Error when programming address: {}", e)
                                });
                            dali_config.buses[bus_number].channels.push(Channel {
                                description,
                                short_address,
                            });

                            count += 1;
                            config.save(&dali_config)?;
                        }

                        println!();
                        println!("Found {} devices on bus", count);
                    }
                    'm' => {
                        let mut dali_bus_iterator = DaliBusIterator::new(
                            dali_manager,
                            bus_number,
                            DaliDeviceSelection::WithoutShortAddress,
                            if log_enabled!(Trace) {
                                None
                            } else {
                                Some(Box::new(|n, s| {
                                    print!("\r{:2} [{:23}]", n, "*".repeat(s as usize + 1));
                                    io::stdout().flush().unwrap();
                                }))
                            },
                        )
                        .expect("Error while initializing DALI bus iteration");

                        let mut prompt_for_terminate = true;

                        while dali_bus_iterator.find_next_device(dali_manager)?.is_some() {
                            let default_short_address =
                                dali_config.buses[bus_number].get_unused_short_address();

                            println!();
                            let short_address = loop {
                                let short_address = Setup::prompt_for_short_address(
                                    "Short address",
                                    default_short_address,
                                )?;
                                if dali_config.buses[bus_number]
                                    .get_channel_index(short_address)
                                    .is_none()
                                {
                                    break short_address;
                                }
                                println!("Short address is already used");
                            };
                            let description = Setup::prompt_for_string(
                                "Description",
                                Some(&format!("Light {}", short_address)),
                            )?;

                            dali_manager
                                .program_short_address(bus_number, short_address)
                                .unwrap_or_else(|e| {
                                    println!("Error when programming address: {}", e)
                                });
                            dali_config.buses[bus_number].channels.push(Channel {
                                description,
                                short_address,
                            });
                            config.save(&dali_config)?;

                            if prompt_for_terminate {
                                let look_for_more = Setup::prompt_for_string(
                                    "Look for more lights y=yes, n=no, a=all",
                                    Some("y"),
                                )?;

                                match look_for_more.chars().next() {
                                    Some('n') => dali_bus_iterator.terminate(),
                                    Some('a') => prompt_for_terminate = false,
                                    _ => {}
                                }
                            }
                        }
                        println!();
                    }
                    '#' => {
                        if let Ok(short_address) =
                            Setup::prompt_for_short_address("Change address", None)
                        {
                            if let Some(index) =
                                dali_config.buses[bus_number].get_channel_index(short_address)
                            {
                                if let Ok(new_short_address) =
                                    Setup::prompt_for_short_address("To address", None)
                                {
                                    if new_short_address >= 64 {
                                        println!("Invalid new address");
                                    }
                                    if new_short_address != short_address {
                                        if dali_config.buses[bus_number]
                                            .find_member(new_short_address)
                                            .is_some()
                                        {
                                            println!("Short address is already used");
                                        } else {
                                            let mut dali_bus_iterator = DaliBusIterator::new(
                                                dali_manager,
                                                bus_number,
                                                DaliDeviceSelection::Address(short_address),
                                                None,
                                            )
                                            .expect("Error while initializing DALI bus iteration");
                                            let mut done = false;

                                            while dali_bus_iterator
                                                .find_next_device(dali_manager)?
                                                .is_some()
                                            {
                                                if !done {
                                                    dali_manager.program_short_address(bus_number, new_short_address).unwrap_or_else(|e| println!("Error when programming address: {}", e));
                                                    dali_config.buses[bus_number].channels[index]
                                                        .short_address = new_short_address; // Update configuration
                                                    done = true;
                                                    config.save(&dali_config)?;
                                                } else {
                                                    println!("Unexpected - more than one device found with short address {}", short_address);
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                println!("A channel with this address is not defined");
                            }
                        }
                    }
                    _ => println!("Invalid command"),
                }
            }
        }

        //let dali_bus_iterator = dali_manager.get_dali_bus_iter(self.bus, dali_manager::DaliDeviceSelection::)
    }

    fn delete_group(
        config: &Config,
        mut dali_config: DaliConfig,
        dali_manager: &mut DaliManager,
        bus_number: usize,
        group_address: u8,
    ) -> Result<DaliConfig, Box<dyn std::error::Error>> {
        //let bus_config = &mut dali_config.buses[bus_number];

        if let Some(group_index) = dali_config.buses[bus_number].get_group_index(group_address) {
            let group = &dali_config.buses[bus_number].groups[group_index];

            for short_address in group.members.iter() {
                dali_manager.remove_from_group(bus_number, group_address, *short_address)?;
            }

            dali_config.buses[bus_number].groups.remove(group_index);
            config.save(&dali_config)?
        }
        Ok(dali_config)
    }

    fn new_group(
        config: &Config,
        mut dali_config: DaliConfig,
        dali_manager: &mut DaliManager,
        bus_number: usize,
        group_address: u8,
    ) -> Result<DaliConfig, Box<dyn std::error::Error>> {
        //let bus_config = &mut dali_config.buses[bus_number];

        let description =
            Setup::prompt_for_string("Description", Some(&format!("Group {}", group_address)))?;
        dali_config.buses[bus_number].groups.push(Group {
            description,
            group_address,
            members: Vec::new(),
        });
        dali_config =
            Setup::edit_group(config, dali_config, dali_manager, bus_number, group_address)?;
        config.save(&dali_config)?;
        Ok(dali_config)
    }

    fn edit_group(
        config: &Config,
        mut dali_config: DaliConfig,
        dali_manager: &mut DaliManager,
        bus_number: usize,
        group_address: u8,
    ) -> Result<DaliConfig, Box<dyn std::error::Error>> {
        //let bus_config = &mut dali_config.buses[bus_number];

        if let Some(group_index) = dali_config.buses[bus_number].get_group_index(group_address) {
            loop {
                dali_config.buses[bus_number]
                    .display_group(&dali_config.buses[bus_number].groups[group_index]);

                let command = Setup::prompt_for_string(
                    "Group members - a:add, d:delete, b:back, p:by Pattern",
                    Some("b"),
                )?;

                if let Some(command) = command.chars().next() {
                    match command {
                        'a' => {
                            let short_address =
                                Setup::prompt_for_short_address("Add to group", None)?;
                            let group = &dali_config.buses[bus_number].groups[group_index];

                            if dali_config.buses[bus_number]
                                .get_channel_index(short_address)
                                .is_none()
                            {
                                println!("No light with this address");
                            } else if group.members.contains(&short_address) {
                                println!("Already in group");
                            } else {
                                let group = &mut dali_config.buses[bus_number].groups[group_index];
                                group.members.push(short_address);

                                if let Err(err) = dali_manager.add_to_group_and_verify(
                                    bus_number,
                                    group_address,
                                    short_address,
                                ) {
                                    println!("Error when adding to group {}", err);
                                } else {
                                    config.save(&dali_config)?;
                                }
                            }
                        }
                        'd' => {
                            let group = &mut dali_config.buses[bus_number].groups[group_index];
                            let short_address =
                                Setup::prompt_for_short_address("Delete from group", None)?;
                            let index = group.members.iter().position(|a| *a == short_address);

                            if let Some(index) = index {
                                group.members.remove(index);
                                dali_manager.remove_from_group_and_verify(
                                    bus_number,
                                    group_address,
                                    short_address,
                                )?;
                                config.save(&dali_config)?;
                            } else {
                                println!("Not in group");
                            }
                        }
                        'p' => {
                            let light_name_pattern = Setup::prompt_for_string(
                                "Group members are lights whose names match",
                                None,
                            )?;

                            dali_manager.match_group(
                                &mut dali_config.buses[bus_number],
                                group_address,
                                &light_name_pattern,
                                Some(Box::new(|action, _| {
                                    std::thread::sleep(std::time::Duration::from_millis(100));
                                    match action {
                                        MatchGroupAction::AddMember(light, _) => {
                                            println!("  Adding {}", light)
                                        }
                                        MatchGroupAction::RemoveMember(light, _) => {
                                            println!("  Remove {}", light)
                                        }
                                    }
                                })),
                            )?;

                            config.save(&dali_config)?;
                        }
                        'b' => break,
                        _ => println!("Invalid command"),
                    }
                }
            }
        }
        Ok(dali_config)
    }

    fn prompt_for_existing_group_address(
        bus_config: &BusConfig,
        prompt: &str,
        default_value: Option<u8>,
    ) -> Result<Option<u8>, Box<dyn std::error::Error>> {
        Ok(loop {
            match Setup::prompt_for_group_address(prompt, default_value) {
                Ok(group_address) => {
                    if bus_config.get_group_index(group_address).is_none() {
                        println!("This group is not defined");
                    } else {
                        break Some(group_address);
                    }
                }
                Err(e) => return Err(e),
            }
        })
    }

    fn prompt_for_new_group_address(
        bus_config: &BusConfig,
        prompt: &str,
    ) -> Result<Option<u8>, Box<dyn std::error::Error>> {
        Ok(loop {
            match Setup::prompt_for_group_address(prompt, bus_config.get_unused_group_address()) {
                Ok(group_address) => {
                    if bus_config.get_group_index(group_address).is_some() {
                        println!("This group is already defined");
                    } else {
                        break Some(group_address);
                    }
                }
                Err(e) => return Err(e),
            }
        })
    }

    fn fix_group_membership(bus_config: &BusConfig, dali_manager: &mut DaliManager) {
        for light in bus_config.channels.iter() {
            match dali_manager.query_group_membership(bus_config.bus, light.short_address) {
                Ok(group_mask) => {
                    // First, look if light is member in groups which are not defined in the configuration, if so, remove them
                    let mut mask = 1u16;
                    for group_number in 0..16 {
                        if (group_mask & mask) != 0
                            && !bus_config
                                .groups
                                .iter()
                                .any(|g| g.group_address == group_number)
                        {
                            println!(
                                "Light {} is member of group {} which is not in configuration:",
                                light.short_address, group_number
                            );
                            match dali_manager.remove_from_group_and_verify(
                                bus_config.bus,
                                group_number,
                                light.short_address,
                            ) {
                                Ok(_) => println!("  removed!"),
                                Err(e) => println!(" error: {}", e),
                            }
                        }

                        mask <<= 1;
                    }

                    // Now ensure that light is indeed member in groups it is supposed to be member of
                    for group in bus_config.groups.iter() {
                        let mask = 1 << group.group_address;

                        if group.members.iter().any(|m| light.short_address == *m)
                            && (group_mask & mask) == 0
                        {
                            println!(
                                "Light {} should be member of group {}, however it is not:",
                                light.short_address, group.description
                            );
                            match dali_manager.add_to_group_and_verify(
                                bus_config.bus,
                                group.group_address,
                                light.short_address,
                            ) {
                                Ok(_) => println!("  added!"),
                                Err(e) => println!(" error: {}", e),
                            }
                        }
                    }
                }
                Err(e) => println!(
                    "Error obtaining group membership of light {}: {}",
                    light.short_address, e
                ),
            }
        }
    }

    pub fn interactive_setup_groups(
        config: &Config,
        mut dali_config: DaliConfig,
        dali_manager: &mut DaliManager,
        bus_number: usize,
    ) -> Result<DaliConfig, Box<dyn std::error::Error>> {
        //let mut bus_config = &mut dali_config.buses[bus_number];
        let mut last_group_address: Option<u8> = None;
        let mut default_level = 255u8;

        loop {
            dali_config.buses[bus_number].display();

            let command = Setup::prompt_for_string(
                "Groups: n=new, d=delete, e=edit, s=set-level, f=fix, b=back",
                Some("b"),
            )?;

            if let Some(command) = command.chars().next() {
                match command {
                    'b' => return Ok(dali_config),
                    'n' => {
                        if let Some(group_address) = Setup::prompt_for_new_group_address(
                            &dali_config.buses[bus_number],
                            "Add group",
                        )? {
                            dali_config = Setup::new_group(
                                config,
                                dali_config,
                                dali_manager,
                                bus_number,
                                group_address,
                            )?;
                        }
                    }
                    's' => {
                        if let Some(group_address) = Setup::prompt_for_existing_group_address(
                            &dali_config.buses[bus_number],
                            "Group address",
                            last_group_address,
                        )? {
                            let level = Setup::prompt_for_number("Level", Some(default_level))?;

                            dali_manager.set_group_brightness(bus_number, group_address, level)?;
                            default_level = 255 - level;
                            last_group_address = Some(group_address);
                        }
                    }
                    'd' => {
                        if let Some(group_address) = Setup::prompt_for_existing_group_address(
                            &dali_config.buses[bus_number],
                            "Delete group",
                            None,
                        )? {
                            dali_config = Setup::delete_group(
                                config,
                                dali_config,
                                dali_manager,
                                bus_number,
                                group_address,
                            )?;
                        }
                    }
                    'e' => {
                        if let Some(group_address) = Setup::prompt_for_existing_group_address(
                            &dali_config.buses[bus_number],
                            "Edit group",
                            None,
                        )? {
                            dali_config = Setup::edit_group(
                                config,
                                dali_config,
                                dali_manager,
                                bus_number,
                                group_address,
                            )?;
                        }
                    }
                    'f' => {
                        Setup::fix_group_membership(&dali_config.buses[bus_number], dali_manager)
                    }
                    _ => println!("Invalid command"),
                }
            }
        }
    }

    fn prompt_for_existing_short_address(
        bus_config: &BusConfig,
        prompt: &str,
        default_value: Option<u8>,
    ) -> Result<Option<u8>, Box<dyn std::error::Error>> {
        Ok(loop {
            match Setup::prompt_for_short_address(prompt, default_value) {
                Ok(short_address) => {
                    if bus_config.get_channel_index(short_address).is_none() {
                        println!("No light with this address");
                    } else {
                        break Some(short_address);
                    }
                }
                Err(e) => return Err(e),
            }
        })
    }

    fn fix_config(
        config: &Config,
        mut dali_config: DaliConfig,
        dali_manager: &mut DaliManager,
        bus_number: usize,
    ) -> Result<DaliConfig, Box<dyn std::error::Error>> {
        //let bus_config = &dali_config.buses[bus_number];
        let mut all_lights_ok = true;
        let mut remove_list = Vec::<u8>::new();

        for light in dali_config.buses[bus_number].channels.iter() {
            match dali_manager.query_light_status(bus_number, light.short_address) {
                Ok(_) => {}
                Err(_) => {
                    all_lights_ok = false;
                    let remove_light = loop {
                        if let Some(reply) = Setup::prompt_for_string(&format!("Light at address {} does not response, remove it from the configuration", light.short_address), Some("n"))?.chars().next() {
                            match  reply {
                                'y' => break true,
                                _ => break false,
                            };
                        }
                    };

                    if remove_light {
                        remove_list.push(light.short_address);
                    }
                }
            }
        }

        if all_lights_ok {
            println!(
                "All lights were found ({})",
                dali_config.buses[bus_number].channels.len()
            );
        } else {
            for short_address_to_remove in remove_list.iter() {
                if let Some(index) = dali_config.buses[bus_number]
                    .channels
                    .iter()
                    .position(|l| l.short_address == *short_address_to_remove)
                {
                    dali_config.buses[bus_number].channels.remove(index);
                }
            }
            config.save(&dali_config)?;
        }

        Ok(dali_config)
    }

    fn interactive_setup_lights(
        config: &Config,
        mut dali_config: DaliConfig,
        dali_manager: &mut DaliManager,
        bus_number: usize,
    ) -> Result<DaliConfig, Box<dyn std::error::Error>> {
        //let mut bus_config = &mut dali_config.buses[bus_number];
        let mut last_short_address: Option<u8> = None;
        let mut default_level = 255u8;

        dali_config.buses[bus_number].display();

        loop {
            let command = Setup::prompt_for_string(
                "Lights - r:rename, s:set-level, q:query, g:group-membership, b:back",
                Some("b"),
            )?;

            if let Some(command) = command.chars().next() {
                match command {
                    'b' => return Ok(dali_config),
                    'r' => {
                        let bus_config = &mut dali_config.buses[bus_number];

                        if let Some(short_address) = Setup::prompt_for_existing_short_address(
                            bus_config,
                            "Rename",
                            last_short_address,
                        )? {
                            let index = bus_config.get_channel_index(short_address).unwrap();
                            let description = Setup::prompt_for_string(
                                "Description: ",
                                Some(&bus_config.channels[index].description),
                            )?;

                            bus_config.channels[index].description = description;
                            last_short_address = Some(short_address);
                            config.save(&dali_config)?;
                        }
                    }
                    's' => {
                        if let Some(short_address) = Setup::prompt_for_existing_short_address(
                            &dali_config.buses[bus_number],
                            "Address",
                            last_short_address,
                        )? {
                            let level = Setup::prompt_for_number("Level", Some(default_level))?;

                            dali_manager.set_light_brightness(bus_number, short_address, level)?;
                            default_level = 255 - level;
                            last_short_address = Some(short_address);
                        }
                    }
                    'q' => {
                        if let Some(short_address) = Setup::prompt_for_existing_short_address(
                            &dali_config.buses[bus_number],
                            "Address",
                            last_short_address,
                        )? {
                            dali_config.buses[bus_number]
                                .do_query_light(dali_manager, short_address);
                        }
                    }
                    'g' => {
                        if let Some(short_address) = Setup::prompt_for_existing_short_address(
                            &dali_config.buses[bus_number],
                            "Address",
                            last_short_address,
                        )? {
                            let mask =
                                dali_manager.query_group_membership(bus_number, short_address)?;
                            println!(
                                "Light {bus_number}/{short_address} Group membership mask: {mask}"
                            );
                        }
                    }
                    '?' => dali_config.buses[bus_number].display(),
                    _ => println!("Invalid command"),
                }
            }
        }
    }

    pub fn interactive_bus_setup(
        config: &Config,
        mut dali_config: DaliConfig,
        dali_manager: &mut DaliManager,
        bus_number: usize,
    ) -> Result<DaliConfig, Box<dyn std::error::Error>> {
        //let mut bus_config = &mut dali_config.buses[bus_number];

        if !matches!(dali_config.buses[bus_number].status, BusStatus::Active) {
            loop {
                dali_config.buses[bus_number].display();
                println!(
                    "Bus {}!",
                    if matches!(dali_config.buses[bus_number].status, BusStatus::NoPower) {
                        "has no power"
                    } else {
                        "is overloaded"
                    }
                );
                let command = Setup::prompt_for_string("Bus: r=rename, b=back", Some("b"))?;

                if let Some(command) = command.chars().next() {
                    match command {
                        'b' => return Ok(dali_config),
                        'r' => {
                            dali_config.buses[bus_number].description = Setup::prompt_for_string(
                                "Description",
                                Some(&dali_config.buses[bus_number].description),
                            )?;
                            config.save(&dali_config)?;
                        }
                        _ => println!("Invalid command"),
                    }
                }
            }
        } else {
            loop {
                dali_config.buses[bus_number].display();
                let command = Setup::prompt_for_string("Bus - r:rename, a:assign addresses, l:lights, g:groups, q:query, f:fix, b:back", Some("b"))?;

                if let Some(command) = command.chars().next() {
                    match command {
                        'b' => return Ok(dali_config),
                        'r' => {
                            dali_config.buses[bus_number].description = Setup::prompt_for_string(
                                "Description",
                                Some(&dali_config.buses[bus_number].description),
                            )?;
                            config.save(&dali_config)?;
                        }
                        'a' => {
                            dali_config = Setup::assign_addresses(
                                config,
                                dali_config,
                                dali_manager,
                                bus_number,
                            )?
                        }
                        'g' => {
                            dali_config = Setup::interactive_setup_groups(
                                config,
                                dali_config,
                                dali_manager,
                                bus_number,
                            )?
                        }
                        'l' => {
                            dali_config = Setup::interactive_setup_lights(
                                config,
                                dali_config,
                                dali_manager,
                                bus_number,
                            )?
                        }
                        'q' => dali_config.buses[bus_number].query_bus(dali_manager),
                        'f' => {
                            dali_config =
                                Setup::fix_config(config, dali_config, dali_manager, bus_number)?
                        }
                        _ => println!("Invalid command"),
                    }
                }
            }
        }
    }

    pub fn interactive_setup(
        config: &Config,
        mut dali_config: DaliConfig,
        dali_manager: &mut DaliManager,
    ) -> Result<SetupAction, Box<dyn std::error::Error>> {
        loop {
            if let Err(e) = dali_config.update_bus_status(dali_manager) {
                println!("Warning: error when trying to obtain bus status: {}", e);
            }

            dali_config.display();

            let command = Setup::prompt_for_string(
                "Controller - r:rename, b:bus setup, q:quit, s:start",
                Some("s"),
            )?;

            if let Some(command) = command.chars().next() {
                match command {
                    's' => return Ok(SetupAction::Start(dali_config)),
                    'q' => return Ok(SetupAction::Quit),
                    'r' => {
                        dali_config.name =
                            Setup::prompt_for_string("Name", Some(&dali_config.name))?;
                    }
                    'b' => {
                        let bus_number = if dali_config.buses.len() == 1 {
                            0
                        } else {
                            Setup::prompt_for_number("Setup bus#", Some(1))? - 1
                        };

                        if bus_number >= dali_config.buses.len() {
                            println!("Invalid bus number");
                        } else {
                            dali_config = Setup::interactive_bus_setup(
                                config,
                                dali_config,
                                dali_manager,
                                bus_number,
                            )?;
                        }
                    }
                    _ => println!("Invalid command"),
                }
            }
        }
    }

    pub fn display_prompt<T: std::fmt::Display>(prompt: &str, default_value: Option<T>) {
        if let Some(default_value) = default_value {
            print!("{} [{}]: ", prompt, default_value);
        } else {
            print!("{}: ", prompt);
        }

        io::stdout().flush().unwrap();
    }

    fn get_input() -> Result<String, Box<dyn std::error::Error>> {
        let mut value = String::new();
        io::stdin().read_line(&mut value)?;

        Ok(value.trim_end().to_owned())
    }

    pub fn prompt_for_string(
        prompt: &str,
        default_value: Option<&str>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        loop {
            Setup::display_prompt(prompt, default_value);
            let value = Setup::get_input()?;

            if value.is_empty() {
                if let Some(default_value) = default_value {
                    return Ok(default_value.to_owned());
                }

                println!("Value cannot be empty");
            } else {
                return Ok(value.trim_end().to_owned());
            }
        }
    }

    pub fn prompt_for_yes_no(
        prompt: &str,
        default_value: bool,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        loop {
            let default_prompt = if default_value { "y" } else { "n" };

            let value = Setup::prompt_for_string(prompt, Some(default_prompt))?;

            match value.chars().next().unwrap() {
                'y' | 'Y' => return Ok(true),
                'n' | 'N' => return Ok(false),
                _ => println!("Invalid value"),
            }
        }
    }

    pub fn prompt_for_number<T: std::str::FromStr + std::fmt::Display + Copy>(
        prompt: &str,
        default_value: Option<T>,
    ) -> Result<T, Box<dyn std::error::Error>> {
        loop {
            Setup::display_prompt(prompt, default_value);

            let value_as_string = Setup::get_input()?;

            if value_as_string.is_empty() {
                if let Some(default_value) = default_value {
                    return Ok(default_value.to_owned());
                } else {
                    return Err(Box::new(SetupError::UserQuit));
                }
            }

            match value_as_string.parse() {
                Ok(v) => return Ok(v),
                Err(_) => {
                    println!("Invalid value");
                }
            }
        }
    }

    pub fn prompt_for_short_address(
        prompt: &str,
        default_value: Option<u8>,
    ) -> Result<u8, Box<dyn std::error::Error>> {
        loop {
            let short_address = Setup::prompt_for_number(prompt, default_value)?;

            if short_address >= 64 {
                println!("Invalid short address (valid is 0-63)");
            } else {
                break Ok(short_address);
            }
        }
    }

    pub fn prompt_for_group_address(
        prompt: &str,
        default_value: Option<u8>,
    ) -> Result<u8, Box<dyn std::error::Error>> {
        loop {
            let group = Setup::prompt_for_number(prompt, default_value)?;

            if group >= 16 {
                println!("Invalid group number (valid is 0-15)");
            } else {
                break Ok(group);
            }
        }
    }
}

impl Config {
    pub fn load(&self) -> Result<DaliConfig, SetupError> {
        let path = Path::new(&self.config_filename);

        let file = File::open(path)?;
        let dali_config: DaliConfig = serde_json::from_reader(file)?;

        Ok(dali_config)
    }

    pub fn save(&self, dali_config: &DaliConfig) -> Result<(), SetupError> {
        let path = Path::new(&self.config_filename);
        let file = File::create(path)?;

        serde_json::to_writer_pretty(file, &dali_config)?;
        Ok(())
    }
}
