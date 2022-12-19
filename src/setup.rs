use log::{log_enabled, Level::Trace};
use std::{path::Path, fs::File, io, io::Write, fmt};
use crate::{config_payload::{Config, BusConfig, BusStatus, Channel, Group}, dali_manager::{DaliManager, DaliBusIterator, DaliDeviceSelection}};
use crate::dali_manager::{MatchGroupAction};

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

impl std::error::Error for SetupError { }

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
    Start,
}

impl BusConfig {
    const CHANNELS_PER_LINE: usize = 4;

    pub fn new(bus_number: usize, status: BusStatus) -> BusConfig {
        let description = format!("Bus-{}", bus_number+1);

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

    fn display_channels(&self) {
        println!("  Channels:");
        for i in 0..self.channels.len() {
            if i % BusConfig::CHANNELS_PER_LINE == 0 {
                print!("    ");
            }

            print!("{:18}", self.channels[i].to_string());

            if (i+1) % BusConfig::CHANNELS_PER_LINE == 0 {
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

            match self.find_member(group.members[i]) {
                Some(channel) => print!("{:18}", channel.to_string()),
                _ => print!("Missing {:10}", self.channels[i]),
            }

            if (i+1) % BusConfig::CHANNELS_PER_LINE == 0 {
                println!();
            }
        }

        if group.members.len() % BusConfig::CHANNELS_PER_LINE != 0 {
            println!();
        }
    }

    pub fn display(&self, bus_number: usize) {
        println!("{}: DALI bus: {} ({})", bus_number+1, self.description, self.status);

        if self.channels.is_empty() {
            println!("  No channels");
        }
        else {
            self.display_channels();
        }

        if self.groups.is_empty() {
                println!("  No groups");
        }
        else {
            println!("  groups:");
            for group in self.groups.iter() {
                self.display_group(group);
            }
        }
    }

    fn get_channel_index(&self, short_address: u8) -> Option<usize> {
        self.channels.iter().position(|channel| channel.short_address == short_address)
    }

    fn get_group_index(&self, group_address: u8) -> Option<usize> {
        self.groups.iter().position(|group| group.group_address == group_address)
    }
    fn get_unused_short_address(&self) -> Option<u8> {
        (0..64u8).find(|short_address| self.get_channel_index(*short_address).is_none())
    }

    fn get_unused_group_address(&self) -> Option<u8> {
        (0..16u8).find(|group_address| self.get_group_index(*group_address).is_none())
    }

    pub fn assign_addresses(&mut self, dali_manager: &mut DaliManager) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            let default_assign = if self.channels.is_empty() { Some("a") } else { Some("b") };
            let command = Config::prompt_for_string("Assign short addresses: a=All, m=missing, #=change light's address, d=change light's description, b=back", default_assign)?;

            if let Some(command) = command.chars().next() {
                match command {
                    'b' => return Ok(()),
                    'd' => {
                        if let Ok(short_address) = Config::prompt_for_number::<u8>("Change description of address", &None) {
                            if let Some(index) = self.get_channel_index(short_address) {
                                let new_description = Config::prompt_for_string("Description", None)?;
                                self.channels[index].description = new_description;
                            } else {
                                println!("No channel with this address found");
                            }
                        }
                    },
                    'a' => {
                        let mut count = 0;
                        let prompt_for_each = Config::prompt_for_string("Assign all: a=auto, p=prompt for short-address/description", Some("a"))?;
                        let prompt_for_each = !prompt_for_each.starts_with('a');

                        let mut dali_bus_iterator  = DaliBusIterator::new(dali_manager, self.bus, DaliDeviceSelection::All,
                             if log_enabled!(Trace) { None } else { 
                                    Some(Box::new(|n, s| {
                                        print!("\r{:2} [{:23}]", n, "*".repeat(s as usize + 1));
                                        io::stdout().flush().unwrap();
                                    })) 
                                }).expect( "Error while initializing DALI bus iteration");
                        self.channels = Vec::new();
                        self.groups = Vec::new();

                        while dali_bus_iterator.find_next_device(dali_manager)?.is_some() {
                            if !log_enabled!(Trace) {
                                println!();
                            }

                            let default_short_address = self.get_unused_short_address();

                            let short_address = match default_short_address {
                                Some(default_short_address) if !prompt_for_each => default_short_address,
                                _ => { 
                                    loop {
                                        let short_address = Config::prompt_for_short_address("Short address", &default_short_address)?;
                                        if self.get_channel_index(short_address).is_none() {
                                            break short_address;
                                        }
                                        println!("Short address is already used");
                                    }
                                }
                            };
                            let default_description = format!("Light {}", short_address);

                            let description = if prompt_for_each {
                                Config::prompt_for_string("Description",Some(&default_description))?
                            } else {
                                default_description
                            };

                            if !prompt_for_each {
                                println!("     assigning address {} to {}", short_address, description);
                            }

                            dali_manager.program_short_address(self.bus, short_address).unwrap_or_else(|e| println!("Error when programming address: {}", e));
                            self.channels.push(Channel{ description, short_address });

                            count += 1;
                        }

                        println!();
                        println!("Found {} devices on bus", count);
                    }
                    'm' => {
                        let mut dali_bus_iterator = 
                            DaliBusIterator::new(dali_manager, self.bus, DaliDeviceSelection::WithoutShortAddress, None).
                            expect("Error while initializing DALI bus iteration");

                        while dali_bus_iterator.find_next_device(dali_manager)?.is_some() {
                            let default_short_address = self.get_unused_short_address();
                            let short_address = loop {
                                let short_address = Config::prompt_for_short_address("Short address", &default_short_address)?;
                                if self.get_channel_index(short_address).is_none() {
                                    break short_address;
                                }
                                println!("Short address is already used");
                            };
                            let description = Config::prompt_for_string("Description",Some(&format!("Light {}", short_address)))?;

                            dali_manager.program_short_address(self.bus, short_address).unwrap_or_else(|e| println!("Error when programming address: {}", e));
                            self.channels.push(Channel{ description, short_address });
                        }
                    }
                    '#' => {
                        if let Ok(short_address) = Config::prompt_for_short_address("Change address", &None) {
                            if let Some(index) = self.get_channel_index(short_address) {
                                if let Ok(new_short_address) = Config::prompt_for_short_address("To address", &None) {
                                    if new_short_address >= 64 {
                                        println!("Invalid new address");
                                    }
                                    if new_short_address != short_address {
                                        if self.find_member(new_short_address).is_some() {
                                            println!("Short address is already used");
                                        }
                                        else {
                                            let mut dali_bus_iterator = 
                                                DaliBusIterator::new(dali_manager, self.bus , DaliDeviceSelection::Address(short_address), None).expect("Error while initializing DALI bus iteration");
                                            let mut done = false;

                                            while dali_bus_iterator.find_next_device(dali_manager)?.is_some() {
                                                if !done {
                                                    dali_manager.program_short_address(self.bus, new_short_address).unwrap_or_else(|e| println!("Error when programming address: {}", e));
                                                    self.channels[index].short_address = new_short_address;     // Update configuration
                                                    done = true;
                                                } else {
                                                    println!("Unexpected - more than one device found with short address {}", short_address);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            else {
                                println!("A channel with this address is not defined");
                            }
                        }
                    },
                    _ => println!("Invalid command"),
                }
            }
        }

        //let dali_bus_iterator = dali_manager.get_dali_bus_iter(self.bus, dali_manager::DaliDeviceSelection::)
    }

    fn delete_group(&mut self, dali_manager: &mut DaliManager, group_address: u8) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(group_index) = self.get_group_index(group_address) {
            let group = &self.groups[group_index];

            for short_address in group.members.iter() {
                dali_manager.remove_from_group(self.bus, group_address, *short_address)?;
            }

            self.groups.remove(group_index);
        }
        Ok(())
    }

    fn new_group(&mut self, dali_manager: &mut DaliManager, group_address: u8) -> Result<(), Box<dyn std::error::Error>> {
        let description = Config::prompt_for_string("Description", Some(&format!("Group {}", group_address)))?;
        self.groups.push(Group { description, group_address, members: Vec::new() });
        self.edit_group(dali_manager, group_address)?;
        Ok(())
    }

    fn edit_group(&mut self, dali_manager: &mut DaliManager, group_address: u8) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(group_index) = self.get_group_index(group_address) {

            loop {

                self.display_group(&self.groups[group_index]);
                let command = Config::prompt_for_string("Group members: a=add, d=delete, b=back, p=by Pattern", Some("b"))?;

                if let Some(command) = command.chars().next() {
                    match command {
                        'a' => {
                            let short_address = Config::prompt_for_short_address("Add to group", &None)?;
                            let group = & self.groups[group_index];

                            if self.get_channel_index(short_address).is_none() {
                                println!("No light with this address");
                            } else if group.members.contains(&short_address) {
                                println!("Already in group");
                            } else {
                                let group = & mut self.groups[group_index];
                                group.members.push(short_address);

                                if let Err(err) = dali_manager.add_to_group_and_verify(self.bus, group_address, short_address) {
                                    println!("Error when adding to group {}", err);
                                }
                            }
                        },
                        'd' => {
                            let group = & mut self.groups[group_index];
                            let short_address = Config::prompt_for_short_address("Delete from group", &None)?;
                            let index = group.members.iter().position(|a| *a == short_address);

                            if let Some(index) = index {
                                group.members.remove(index);
                                dali_manager.remove_from_group_and_verify(self.bus, group_address, short_address)?;
                            }
                            else {
                                println!("Not in group");
                            }
                        },
                        'p' => {
                            let light_name_pattern = Config::prompt_for_string("Group members are lights whose names match", None)?;

                            dali_manager.match_group(self, group_address, &light_name_pattern, Some(Box::new(|action, _| {
                                    match action {
                                        MatchGroupAction::AddMember(light, _) => println!("  Adding {}", light),
                                        MatchGroupAction::RemoveMember(light, _) => println!("  Remove {}", light),
                                    }
                                }))
                            )?;

                        },
                        'b' => break,
                        _ => println!("Invalid command"),
                    }
                }
            }
        }
        Ok(())
    }

    fn prompt_for_existing_group_address(&self, prompt: &str, default_value: Option<u8>) -> Result<Option<u8>, Box<dyn std::error::Error>> {
        Ok(loop {
            match Config::prompt_for_group_address(prompt, &default_value) {
                Ok(group_address) => {
                    if self.get_group_index(group_address).is_none() {
                        println!("This group is not defined");
                    }
                    else { break Some(group_address) }
                },
                Err(e) => return Err(e),
            }
        })
    }

    fn prompt_for_new_group_address(&self, prompt: &str) -> Result<Option<u8>, Box<dyn std::error::Error>> {
        Ok(loop {
            match Config::prompt_for_group_address(prompt, &self.get_unused_group_address()) {
                Ok(group_address) => {
                    if self.get_group_index(group_address).is_some() {
                        println!("This group is already defined");
                    }
                    else { break Some(group_address) }
                },
                Err(e) => return Err(e),
            }
        })
    }

    fn fix_group_membership(&self, dali_manager: &mut DaliManager) {
        for light in self.channels.iter() {
            match dali_manager.query_group_membership(self.bus, light.short_address) {
                Ok(group_mask) => {
                    // First, look if light is member in groups which are not defined in the configuration, if so, remove them
                    let mut mask = 1u16;
                    for group_number in 0..16 {
                        if (group_mask & mask) != 0 && !self.groups.iter().any(|g| g.group_address == group_number) {
                            println!("Light {} is member of group {} which is not in configuration:", light.short_address, group_number);
                            match dali_manager.remove_from_group(self.bus, group_number, light.short_address) {
                                Ok(_) => println!("  removed!"),
                                Err(e) => println!(" error: {}", e),
                            }
                        }

                        mask <<= 1;
                    }

                    // Now ensure that light is indeed member in groups it is supposed to be member of
                    for group in self.groups.iter() {
                        let mask = 1 << group.group_address;

                        if group.members.iter().any(|m| light.short_address == *m) && (group_mask & mask) == 0 {
                            println!("Light {} should be member of group {}, however it is not:", light.short_address, group.description);
                            match dali_manager.add_to_group(self.bus, group.group_address, light.short_address) {
                                Ok(_) => println!("  added!"),
                                Err(e) => println!(" error: {}", e),
                            }
                        }
                    }

                },
                Err(e) =>  println!("Error obtaining group membership of light {}: {}", light.short_address, e),
            }
        }
    }
    pub fn interactive_setup_groups(&mut self, dali_manager: &mut DaliManager, bus_number: usize) -> Result<(), Box<dyn std::error::Error>> {
        let mut last_group_address: Option<u8> = None;
        let mut default_level = 255u8;

        loop {
            self.display(bus_number);
            let command = Config::prompt_for_string("Groups: n=new, d=delete, e=edit, s=set-level, f=fix, b=back", Some("b"))?;

            if let Some(command) = command.chars().next() {
                match command {
                    'b' => return Ok(()),
                    'n' => {
                        if let Some(group_address) = self.prompt_for_new_group_address("Add group")? {
                            self.new_group(dali_manager, group_address)?;
                        }
                    },
                    's' => {
                        if let Some(group_address) = self.prompt_for_existing_group_address("Group address", last_group_address)? {
                            let level = Config::prompt_for_number("Level", &Some(default_level))?;

                            dali_manager.set_group_brightness(self.bus, group_address, level)?;
                            default_level = 255-level;
                            last_group_address = Some(group_address);
                        }
                    },
                    'd' => {
                        if let Some(group_address) = self.prompt_for_existing_group_address("Delete group", None)? {
                            self.delete_group(dali_manager, group_address)?;
                        }
                    },
                    'e' => {
                        if let Some(group_address) = self.prompt_for_existing_group_address("Edit group", None)? {
                            self.edit_group(dali_manager, group_address)?;
                        }

                    },
                    'f' => self.fix_group_membership(dali_manager),
                    _ => println!("Invalid command"),
                }
            }
        }
    }

    fn prompt_for_existing_short_address(&self, prompt: &str, default_value: Option<u8>) -> Result<Option<u8>, Box<dyn std::error::Error>> {
        Ok(loop {
            match Config::prompt_for_short_address(prompt, &default_value) {
                Ok(short_address) => {
                    if self.get_channel_index(short_address).is_none() {
                        println!("No light with this address");
                    }
                    else { break Some(short_address) }
                },
                Err(e) => return Err(e),
            }
        })
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
                        } else {        // Light reports membership, but it is not reflected in the configuration 
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

    fn fix_config(&mut self, dali_manager: &mut DaliManager) -> Result<(), Box<dyn std::error::Error>> {
        let mut all_lights_ok = true;
        let mut remove_list = Vec::<u8>::new();

        for light in self.channels.iter() {
            match dali_manager.query_light_status(self.bus, light.short_address) {
                Ok(_) => {},
                Err(_) => {
                    all_lights_ok = false;
                    let remove_light = loop {
                        if let Some(reply) = Config::prompt_for_string(&format!("Light at address {} does not response, remove it from the configuration", light.short_address), Some("n"))?.chars().next() {
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
            println!("All lights were found ({})", self.channels.len());
        } else {
            for short_address_to_remove in remove_list.iter() {
                if let Some(index) = self.channels.iter().position(|l| l.short_address == *short_address_to_remove) {
                    self.channels.remove(index);
                }
            }
        }

        Ok(())
    }

    fn interactive_setup_lights(&mut self, dali_manager: &mut DaliManager, bus_number: usize) -> Result<(), Box<dyn std::error::Error>> {
        let mut last_short_address: Option<u8> = None;
        let mut default_level = 255u8;

        self.display(bus_number);

        loop {
            let command = Config::prompt_for_string("Lights: r=rename, s=set-level, q=query, b=back", Some("b"))?;

            if let Some(command) = command.chars().next() {
                match command {
                    'b' => return Ok(()),
                    'r' => {
                        if let Some(short_address) = self.prompt_for_existing_short_address("Rename", last_short_address)? {
                            let index = self.get_channel_index(short_address).unwrap();
                            let description = Config::prompt_for_string("Description: ", Some(&self.channels[index].description))?;

                            self.channels[index].description = description;
                            last_short_address = Some(short_address);
                        }                        
                    },
                    's' => {
                        if let Some(short_address) = self.prompt_for_existing_short_address("Address", last_short_address)? {
                            let level = Config::prompt_for_number("Level", &Some(default_level))?;

                            dali_manager.set_light_brightness(self.bus, short_address, level)?;
                            default_level = 255-level;
                            last_short_address = Some(short_address);
                        }
                    },
                    'q' => {
                        if let Some(short_address) = self.prompt_for_existing_short_address("Address", last_short_address)? {
                            self.do_query_light(dali_manager,short_address);
                        }
                    },
                    '?' => self.display(bus_number),
                    _ => println!("Invalid command"),
                }
            }
        }

    }

    pub fn interactive_setup(&mut self, dali_manager: &mut DaliManager, bus_number: usize) -> Result<(), Box<dyn std::error::Error>> {
        if !matches!(self.status, BusStatus::Active) {
            loop {
                self.display(bus_number);
                println!("Bus {}!", if matches!(self.status,BusStatus::NoPower) { "has no power"} else { "is overloaded"});
                let command = Config::prompt_for_string("Bus: r=rename, b=back", Some("b"))?;

                if let Some(command) = command.chars().next() {
                    match command {
                        'b' => return Ok(()),
                        'r' => self.description = Config::prompt_for_string("Description", Some(&self.description))?,
                        _ => println!("Invalid command"),
                    }
                }
            }
        }
        else {
            loop {
                self.display(bus_number);
                let command = Config::prompt_for_string("Bus: r=rename, a=assign addresses, l=lights, g=groups, q=query, f=fix, b=back", Some("b"))?;

                if let Some(command) = command.chars().next() {
                    match command {
                        'b' => return Ok(()),
                        'r' => self.description = Config::prompt_for_string("Description", Some(&self.description))?,
                        'a' => self.assign_addresses(dali_manager)?,
                        'g' => self.interactive_setup_groups(dali_manager, bus_number)?,
                        'l' => self.interactive_setup_lights(dali_manager, bus_number)?,
                        'q' => self.query_bus(dali_manager),
                        'f' => self.fix_config(dali_manager)?,
                        _ => println!("Invalid command"),
                    }
                }
            }
        }
    }
}

impl Config {
    pub fn new(name: &str) -> Config {
        Config { 
            name: name.to_owned(),
            buses: Vec::new(),
        }
    }

    pub fn load(config_file: &str) -> Result<Config, SetupError> {
        let path = Path::new(config_file);

        let file = File::open(path)?;
        let config: Config = serde_json::from_reader(file)?;

        Ok(config)
    }

    pub fn save(&self, config_file: &str) -> Result<(), SetupError> {
        let path = Path::new(config_file);
        let file = File::create(path)?;

        serde_json::to_writer_pretty(file,self)?;
        Ok(())
    }

    pub fn display(&self) {
        println!("Controller {}", self.name);

        for bus_number in 0..self.buses.len() {
            self.buses[bus_number].display(bus_number)
        }
    }

    pub fn display_prompt<T : std::fmt::Display>(prompt: &str, default_value: &Option<T>) {
        if let Some(default_value) = default_value {
            print!("{} [{}]: ", prompt, default_value);
        }
        else {
            print!("{}: ", prompt);
        }

        io::stdout().flush().unwrap();
    }

    fn get_input() -> Result<String, Box<dyn std::error::Error>> {
        let mut value = String::new();
        io::stdin().read_line(&mut value)?;

        Ok(value.trim_end().to_owned())
    }

    pub fn prompt_for_string(prompt: &str, default_value: Option<&str>) -> Result<String, Box<dyn std::error::Error>> {
        loop {
            Config::display_prompt(prompt, &default_value);
            let value = Config::get_input()?;

            if value.is_empty() {
                if let Some(default_value) = default_value {
                    return Ok(default_value.to_owned());
                }

                println!("Value cannot be empty");
            }
            else {
                return Ok(value.trim_end().to_owned());
            }
        }
    }

    pub fn prompt_for_number<T: std::str::FromStr + std::fmt::Display + Copy>(prompt: &str, default_value: &Option<T>) -> Result<T, Box<dyn std::error::Error>> {
        loop {
            Config::display_prompt(prompt, default_value);

            let value_as_string = Config::get_input()?;

            if value_as_string.is_empty() {
                if !default_value.is_none() {
                    return Ok(default_value.unwrap().to_owned());
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

    pub fn prompt_for_short_address(prompt: &str, default_value: &Option<u8>) -> Result<u8, Box<dyn std::error::Error>> {
        loop {
            let short_address = Config::prompt_for_number(prompt, default_value)?;

            if short_address >= 64 {
                println!("Invalid short address (valid is 0-63)");
            } else {
                break Ok(short_address);
            }
        }
    }

    pub fn prompt_for_group_address(prompt: &str, default_value: &Option<u8>) -> Result<u8, Box<dyn std::error::Error>> {
        loop {
            let group = Config::prompt_for_number(prompt, default_value)?;

            if group >= 16 {
                println!("Invalid group number (valid is 0-15)");
            } else {
                break Ok(group);
            }
        }
    }

    pub fn interactive_new() -> Result<Config, Box<dyn std::error::Error>> {
        let controller_name = Config::prompt_for_string("Controller name", None)?;

        Ok(Config::new(&controller_name))
    }

    fn update_bus_status(&mut self, dali_manager: &mut DaliManager) -> Result<(), Box<dyn std::error::Error>> {

        for (bus_number, bus) in self.buses.iter_mut().enumerate() {
            bus.status = dali_manager.controller.get_bus_status(bus_number)?;
        }

        Ok(())
    }

    pub fn interactive_setup(&mut self, dali_manager: &mut DaliManager) -> Result<SetupAction, Box<dyn std::error::Error>> {

        loop {
            if let Err(e) = self.update_bus_status(dali_manager) {
                println!("Warning: error when trying to obtain bus status: {}", e);
            }
            
            self.display();

            let command = Config::prompt_for_string("Controller: r=rename, b=bus setup, q=quit, s=start", Some("s"))?;

            if let Some(command) = command.chars().next() {
                match command {
                    's' => return Ok(SetupAction::Start),
                    'q' => return Ok(SetupAction::Quit),
                    'r' => {
                        self.name = Config::prompt_for_string("Name", Some(&self.name))?;
                    },
                    'b' => {
                        let bus_number = if self.buses.len() == 1 { 0 } else {
                            Config::prompt_for_number("Setup bus#", &Some(1))? - 1
                        };

                        if bus_number >= self.buses.len() {
                            println!("Invalid bus number");
                        }
                        else {
                            self.buses[bus_number].interactive_setup(dali_manager, bus_number)?;
                        }
                    },
                    _ => println!("Invalid command"),
                }
            }
        }
    }
}
