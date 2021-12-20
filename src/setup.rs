use std::{path::Path, fs::File, io, io::Write};
use crate::{config_payload::{Config, BusConfig, Channel, Group}, dali_manager::{DaliManager, DaliDeviceSelection}};
use serde_json;

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

impl Channel {
    pub fn to_string(&self) -> String {
        format!("{} - {}", self.short_address, self.description)
    }
}

impl Group {
    const CHANNELS_PER_LINE: usize = 4;

    pub fn display(&self, bus_config: &BusConfig) {
        println!("{} ({}):", self.group, self.description);
        for i in 0..self.channels.len() {
            if i % Group::CHANNELS_PER_LINE == 0 {
                print!("  ");
            }

            match bus_config.find_channel(self.channels[i]) {
                Some(channel) => print!("{:18}", channel.to_string()),
                _ => print!("Missing {:10}", self.channels[i]),
            }

            if (i+1) % Group::CHANNELS_PER_LINE == 0 {
                println!();
            }
        }

        if self.channels.len() % Group::CHANNELS_PER_LINE != 0 {
            println!();
        }
    }
}

impl BusConfig {
    const CHANNELS_PER_LINE: usize = 4;

    fn new(bus: usize) -> BusConfig {
        let description = format!("Bus-{}", bus+1);

        BusConfig {
            description,
            bus,
            channels: Vec::new(),
            groups: Vec::new(),
        }
    }

    pub fn find_channel(&self, channel: u8) -> Option<&Channel> {
        self.channels.iter().find(|c| c.short_address == channel)
    }

    pub fn display(&self, bus_number: usize) {
        println!("{}: DALI bus: {}", bus_number+1, self.description);

        if self.channels.is_empty() {
            println!("  No channels");
        }
        else {
            println!("  Channels:");
            for i in 0..self.channels.len() {
                if i % BusConfig::CHANNELS_PER_LINE == 0 {
                    print!("  ");
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

        if self.groups.is_empty() {
                println!("  No groups");
        }
        else {
            println!("{} groups:", self.groups.len());
            for group in self.groups.iter() {
                group.display(self);
            }
        }
    }

    fn get_channel_index(&self, short_address: u8) -> Option<usize> {
        self.channels.iter().position(|channel| channel.short_address == short_address)
    }

    fn get_unused_short_address(&self) -> Option<u8> {
        for short_address in 0..64u8 {
            if let None = self.get_channel_index(short_address) {
                return Some(short_address);
            }
        }

        None
    }

    pub fn assign_addresses(&mut self, dali_manager: &mut DaliManager) -> Result<(), SetupError> {
        loop {
            let command = Config::prompt_for_string("a=All m=missing, #=change light's address, d=change light's description, b=back", Some("m"))?;
            if let Some(command) = command.chars().nth(0) {
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
                        let dali_bus_iterator = dali_manager.get_dali_bus_iter(self.bus, DaliDeviceSelection::All);
                        self.channels = Vec::new();

                        for _ in dali_bus_iterator {
                            let default_short_address = self.get_unused_short_address();
                            let short_address = loop {
                                let short_address = Config::prompt_for_short_address("Short address", &default_short_address)?;
                                if let None = self.get_channel_index(short_address) {
                                    break short_address;
                                }
                                println!("Short address is already used");
                            };
                            let description = Config::prompt_for_string("Description",Some(&format!("Channel {}", short_address)))?;

                            dali_manager.program_short_address(self.bus, short_address);
                            self.channels.push(Channel{ description, short_address });
                        }
                    }
                    'm' => {
                        let dali_bus_iterator = dali_manager.get_dali_bus_iter(self.bus, DaliDeviceSelection::WithoutShortAddress);

                        for _ in dali_bus_iterator {
                            let default_short_address = self.get_unused_short_address();
                            let short_address = loop {
                                let short_address = Config::prompt_for_short_address("Short address", &default_short_address)?;
                                if let None = self.get_channel_index(short_address) {
                                    break short_address;
                                }
                                println!("Short address is already used");
                            };
                            let description = Config::prompt_for_string("Description",Some(&format!("Channel {}", short_address)))?;

                            dali_manager.program_short_address(self.bus, short_address);
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
                                        if let Some(_) = self.find_channel(new_short_address) {
                                            println!("Short address is already used");
                                        }
                                        else {
                                            let dali_bus_iterator = dali_manager.get_dali_bus_iter(self.bus , DaliDeviceSelection::Address(short_address));
                                            let mut done = false;

                                            for _ in dali_bus_iterator {
                                                if !done {
                                                    dali_manager.program_short_address(self.bus, new_short_address);
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

    pub fn interactive_setup_groups(&mut self, _dali_manager: &DaliManager, bus_number: usize) -> Result<(), SetupError> {
        loop {
            self.display(bus_number);
            let command = Config::prompt_for_string("a=add, d=delete, e=edit, b=back", Some("b"))?;

            if let Some(command) = command.chars().nth(0) {
                match command {
                    'b' => return Ok(()),
                    _ => println!("Invalid command"),
                }
            }
        }
    }

    pub fn interactive_setup(&mut self, dali_manager: &mut DaliManager, bus_number: usize) -> Result<(), SetupError> {
        loop {
            self.display(bus_number);
            let command = Config::prompt_for_string("r=rename, a=assign addresses, g=groups, b=back", Some("b"))?;

            if let Some(command) = command.chars().nth(0) {
                match command {
                    'b' => return Ok(()),
                    'r' => self.description = Config::prompt_for_string("Description", Some(&self.description))?,
                    'a' => self.assign_addresses(dali_manager)?,
                    'g' => self.interactive_setup_groups(dali_manager, bus_number)?,
                    _ => println!("Invalid command"),
                }
            }
        }
    }
}

impl Config {
    pub fn new(name: &str, bus_count: usize) -> Config {
        Config { 
            name: name.to_owned(),
            buses: Vec::from_iter((0..bus_count).map(|bus| BusConfig::new(bus))),
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

    fn get_input() -> Result<String, SetupError> {
        let mut value = String::new();
        io::stdin().read_line(&mut value)?;

        Ok(value.trim_end().to_owned())
    }

    pub fn prompt_for_string(prompt: &str, default_value: Option<&str>) -> Result<String, SetupError> {
        loop {
            Config::display_prompt(prompt, &default_value);
            let value = Config::get_input()?;

            if value.is_empty() {
                if default_value.is_some() {
                    return Ok(default_value.unwrap().to_owned());
                }

                println!("Value cannot be empty");
            }
            else {
                return Ok(value.trim_end().to_owned());
            }
        }
    }

    pub fn prompt_for_number<T: std::str::FromStr + std::fmt::Display + Copy>(prompt: &str, default_value: &Option<T>) -> Result<T, SetupError> {
        loop {
            Config::display_prompt(prompt, default_value);

            let value_as_string = Config::get_input()?;

            if value_as_string.is_empty() {
                if !default_value.is_none() {
                    return Ok(default_value.unwrap().to_owned());
                } else {
                    return Err(SetupError::UserQuit);
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

    pub fn prompt_for_short_address(prompt: &str, default_value: &Option<u8>) -> Result<u8, SetupError> {
        loop {
            let short_address = Config::prompt_for_number(prompt, default_value)?;

            if short_address >= 64 {
                println!("Invalid short address (0-63)");
            } else {
                break Ok(short_address);
            }
        }
    }

    pub fn interactive_new() -> Result<Config, SetupError> {
        let controller_name = Config::prompt_for_string("Controller name", None)?;

        let bus_count = loop {
            let bus_count: usize = Config::prompt_for_number("Number of DALI buses supported (1, 2 or 4)", &Some(1))?;

            match bus_count {
                1 | 2 | 4 => break bus_count,
                _ => println!("Valid values are 1, 2, or 4"),
            }
        };

        Ok(Config::new(&controller_name, bus_count))
    }

    pub fn iteractive_setup(&mut self, dali_manager: &mut DaliManager) -> Result<(), SetupError> {

        loop {
            self.display();

            let command = Config::prompt_for_string("Command (r=rename, b=bus setup, q=quit, s=start): ", Some("s"))?;

            if let Some(command) = command.chars().nth(0) {
                match command {
                    's' => return Ok(()),
                    'q' => return Err(SetupError::UserQuit),
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
                    }
                    _ => println!("Invalid command"),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::setup::Config;

    #[test]
    fn test_create_new_config() {
        let config = Config::new("test", 4);

        config.save("dali.json").expect("Save failed");
    }

    #[test]
    fn test_load_config() {
        test_create_new_config();

        let config = Config::load("dali.json").expect("Loading failed");

        assert_eq!(config.name, "test");
        assert_eq!(config.buses[0].description, "Bus-1");
    }
}