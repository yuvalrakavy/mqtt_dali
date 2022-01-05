use std::time::Duration;
use log::{error, debug, trace};
use rumqttc::{MqttOptions, AsyncClient, EventLoop, QoS, Event, Packet, Publish, LastWill};
use std::error::Error;
use crate::dali_manager::{DaliManager, DaliBusResult, DaliBusIterator, DaliDeviceSelection};
use crate::command_payload::{DaliCommand};
use crate::config_payload::{Config,  Group, BusStatus};


pub struct MqttDali<'a> {
    config: &'a mut Config,
    mqtt_client: AsyncClient,
    mqtt_events: EventLoop,
    dali_manager: &'a mut DaliManager<'a>,
}

#[derive(Debug)]
enum CommandError {
    BusNumber(usize),
    ShortAddress(u8),
    GroupAddress(u8),
    BusHasNoPower(usize),
    BusOverloaded(usize),
    InvalidBusStatus(usize),
    GroupAlreadyExist(usize, u8),
    NoSuchGroup(usize, u8),
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::BusNumber(bus_number) => write!(f, "Invalid bus number: {}", bus_number),
            CommandError::ShortAddress(short_address) => write!(f, "Invalid short address: {}", short_address),
            CommandError::GroupAddress(group_address) => write!(f, "Invalid group address: {}", group_address),
            CommandError::BusHasNoPower(bus_number) => write!(f, "Bus {} has no power", bus_number),
            CommandError::BusOverloaded(bus_number) => write!(f, "Bus {} is overloaded", bus_number),
            CommandError::InvalidBusStatus(bus_number) => write!(f, "Bus {} has invalid status", bus_number),
            CommandError::GroupAlreadyExist(bus_number, group_address) => write!(f, "Bus {} has already has group {}", bus_number, group_address),
            CommandError::NoSuchGroup(bus_number, group_address) => write!(f, "Bus {} has no group {} ", bus_number, group_address),
        }
    }
}

impl std::error::Error for CommandError {}

impl <'a> MqttDali<'a> {
    pub fn new(dali_manager: &'a mut DaliManager<'a>, config: &'a mut Config, mqtt_broker: &str) -> MqttDali<'a> {
        let mut mqtt_options = MqttOptions::new(&config.name, mqtt_broker, 1883);
        let last_will = LastWill::new(MqttDali::get_is_active_topic(&config.name), "false".as_bytes(), QoS::AtLeastOnce, true);
        mqtt_options.set_keep_alive(Duration::from_secs(5)).set_last_will(last_will);

        let (mqtt_client, mqtt_events) = AsyncClient::new(mqtt_options, 10);

        MqttDali {
             config,
             mqtt_client,
             mqtt_events, 
             dali_manager,
         }
    }

    fn get_command_topic(&self) -> String {
        let mut topic = "DALI/Controllers/".to_owned();
    
        topic.push_str(&self.config.name);
        topic.push_str("/Command");
    
        topic
    }

    fn get_status_topic(&self) -> String {
        let mut topic = "DALI/Status/".to_owned();
    
        topic.push_str(&self.config.name);
        topic
    }

    fn get_config_topic(&self) -> String {
        let mut topic = "DALI/Config/".to_owned();

        topic.push_str(&self.config.name);

        topic
    }

    fn get_is_active_topic(name: &str) -> String {
        let mut topic = "DALI/Active/".to_owned();

        topic.push_str(name);
        topic
    }

    async fn publish_config(client: &AsyncClient, config_topic: &str, config: &Config) -> Result<(), Box<dyn Error>> {
        Ok(client.publish(config_topic, QoS::AtLeastOnce, true, serde_json::to_vec(config)?).await?)
    }

    fn update_bus_status(&mut self) -> Result<DaliBusResult, Box<dyn Error>> {
        for (bus_number, bus) in self.config.buses.iter_mut().enumerate() {
            bus.status = self.dali_manager.controller.get_bus_status(bus_number)?;
        }

        Ok(DaliBusResult::None)
    }

    fn check_bus_status(bus_number: usize, status: &BusStatus) -> Result<DaliBusResult, Box<dyn Error>> {
        match status {
            BusStatus::Active => Ok(DaliBusResult::None),
            BusStatus::NoPower => Err(Box::new(CommandError::BusHasNoPower(bus_number))),
            BusStatus::Overloaded => Err(Box::new(CommandError::BusOverloaded(bus_number))),
            BusStatus::Unknown => Err(Box::new(CommandError::InvalidBusStatus(bus_number))),
        }

    }
    fn check_bus(&mut self, bus_number: usize) -> Result<DaliBusResult, Box<dyn std::error::Error>> {
        self.update_bus_status()?;

        if let Some(bus) = self.config.buses.get(bus_number) {
            MqttDali::check_bus_status(bus_number, &bus.status)
        } else {
            Err(Box::new(CommandError::BusNumber(bus_number)))
        }
    }
 
    fn rename_bus(&mut self, bus_number: usize, name: &str) -> Result<DaliBusResult, Box<dyn std::error::Error>> {
        if let Some(bus) = self.config.buses.get_mut(bus_number) {
            bus.description = name.to_owned();
            Ok(DaliBusResult::None)
        } else {
            Err(Box::new(CommandError::BusNumber(bus_number)))
        }
    }

    fn rename_light(&mut self, bus_number: usize, short_address: u8, name: &str) ->  Result<DaliBusResult, Box<dyn std::error::Error>> {
        if let Some(bus) = self.config.buses.get_mut(bus_number) {
            if let Some(channel) = bus.channels.iter_mut().find(|c| c.short_address == short_address) {
                channel.description = name.to_owned();
                Ok(DaliBusResult::None)
            } else {
                Err(Box::new(CommandError::ShortAddress(short_address)))
            }
        } else {
            Err(Box::new(CommandError::BusNumber(bus_number)))
        }
    }

    fn rename_group(&mut self, bus_number: usize, group_address: u8, name: &str) ->  Result<DaliBusResult, Box<dyn std::error::Error>> {
        if let Some(bus) = self.config.buses.get_mut(bus_number) {
            if let Some(group) = bus.groups.iter_mut().find(|g| g.group_address == group_address) {
                group.description = name.to_owned();
                Ok(DaliBusResult::None)
            } else {
                Err(Box::new(CommandError::GroupAddress(group_address)))
            }
        } else {
            Err(Box::new(CommandError::BusNumber(bus_number)))
        }
    }

    fn new_group(&mut self, bus_number: usize, group_address: u8) -> Result<DaliBusResult, Box<dyn std::error::Error>> {
        if let Some(bus) = self.config.buses.get_mut(bus_number) {
            if bus.groups.iter().any(|g| g.group_address == group_address) {
                Err(Box::new(CommandError::GroupAlreadyExist(bus_number, group_address)))
            } else {
                bus.groups.push( Group { description: format!("Group {}", group_address), group_address, members: Vec::new() });
                Ok(DaliBusResult::None)
            }
        }  else {
            Err(Box::new(CommandError::BusNumber(bus_number)))
        }
    }

    fn remove_group(&mut self, bus_number: usize, group_address: u8) -> Result<DaliBusResult, Box<dyn std::error::Error>> {
        self.update_bus_status()?;

        if let Some(bus) = self.config.buses.get_mut(bus_number) {
            if let Some(index) = bus.groups.iter().position(|g| g.group_address == group_address) {
                let group = bus.groups.get_mut(index).unwrap();

                // If group is not empty, remove membership of all members from this group
                if !group.members.is_empty() && MqttDali::check_bus_status(bus_number, &bus.status).is_ok() {
                    for short_address in group.members.iter() {
                        self.dali_manager.remove_from_group(bus_number, group_address, *short_address)?;
                    }
                }

                bus.groups.remove(index);
                Ok(DaliBusResult::None)
            } else {
                Err(Box::new(CommandError::NoSuchGroup(bus_number, group_address)))
            }
        }  else {
            Err(Box::new(CommandError::BusNumber(bus_number)))
        }

    }

    fn add_to_group(&mut self, bus_number: usize, group_address: u8, short_address: u8) -> Result<DaliBusResult, Box<dyn std::error::Error>> {
        if let Some(bus) = self.config.buses.get_mut(bus_number) {
            let group = bus.groups.iter_mut().find(|g| g.group_address == group_address);

            // Create group if not found
            if group.is_none() {
                bus.groups.push( Group { description: format!("Group {}", group_address), group_address, members: Vec::new()});
            }

            MqttDali::check_bus_status(bus_number, &bus.status)?;
            self.dali_manager.add_to_group(bus_number, group_address, short_address)?;

            let group = bus.groups.iter_mut().find(|g| g.group_address == group_address).unwrap();
            if !group.members.contains(&short_address) {
                group.members.push(short_address);
            }

            Ok(DaliBusResult::None)
        }  else {
            Err(Box::new(CommandError::BusNumber(bus_number)))
        }
    }

    fn remove_from_group(&mut self, bus_number: usize, group_address: u8, short_address: u8) -> Result<DaliBusResult, Box<dyn std::error::Error>> {
        if let Some(bus) = self.config.buses.get_mut(bus_number) {
            if let Some(group) = bus.groups.iter_mut().find(|g| g.group_address == group_address) {
                if let Some(index) = group.members.iter().position(|m| *m== short_address) {
                    MqttDali::check_bus_status(bus_number, &bus.status)?;
                    self.dali_manager.remove_from_group(bus_number, group_address, short_address)?;
                    group.members.remove(index);
                }
                Ok(DaliBusResult::None)
            } else {
                Err(Box::new(CommandError::NoSuchGroup(bus_number, group_address)))
            }

        }  else {
            Err(Box::new(CommandError::BusNumber(bus_number)))
        }
    }

    fn match_group(&mut self, bus_number: usize, group_address: u8, light_name_pattern: &str) -> Result<DaliBusResult, Box<dyn std::error::Error>> {
        let re = regex::Regex::new(light_name_pattern)?;

        if let Some(bus) = self.config.buses.get_mut(bus_number) {
            MqttDali::check_bus_status(bus_number, &bus.status)?;

            let group = bus.groups.iter_mut().find(|g| g.group_address == group_address);

            // Create group if not found
            if group.is_none() {
                bus.groups.push( Group { description: format!("Group {}", group_address), group_address, members: Vec::new()});
            }

            let group = bus.groups.iter_mut().find(|g| g.group_address == group_address).unwrap();

            for light in bus.channels.iter() {
                if re.is_match(&light.description) {
                    // If this light is not member of the group, add it
                    if !group.members.contains(&light.short_address) {
                        trace!("Light {}: {} matches {} - added to group {}", light.short_address, light.description, light_name_pattern, group_address);
                        self.dali_manager.add_to_group(bus_number, group_address, light.short_address)?;
                        group.members.push(light.short_address);
                    }

                } else {
                    // If this light is member of the group, remove it since its name does not match the pattern
                    if let Some(index) = group.members.iter().position(|short_address|  *short_address == light.short_address) {
                        trace!("Light {}: {} does not match {} - removed from group {}", light.short_address, light.description, light_name_pattern, group_address);
                        self.dali_manager.remove_from_group(bus_number, group_address, light.short_address)?;
                        group.members.remove(index);
                    }
                }
            }

            Ok(DaliBusResult::None)
        }  else {
            Err(Box::new(CommandError::BusNumber(bus_number)))
        }

    }

    async fn find_lights(&mut self, config_topic: &str, bus_number: usize, selection: DaliDeviceSelection) -> Result<DaliBusResult, Box<dyn std::error::Error>> {
        self.check_bus(bus_number)?;

        if matches!(selection, DaliDeviceSelection::All) {
            let bus = self.config.buses.get_mut(bus_number).unwrap();

            bus.channels.clear();
        }

        let mut device_iterator = DaliBusIterator::new(self.dali_manager, bus_number, selection, None)?;

        while device_iterator.find_next_device(self.dali_manager)?.is_some() {
            let short_address = (0..64u8).find(|short_address|
                !self.config.buses[bus_number].channels.iter().any(|channel| channel.short_address == *short_address)
            ).expect("Unable to find unused short address!!");

            self.dali_manager.program_short_address(bus_number, short_address)?;
            {
                let bus = self.config.buses.get_mut(bus_number).unwrap();
                bus.channels.push(crate::config_payload::Channel{ description: format!("Light {}", short_address), short_address });
            }

            MqttDali::publish_config(&self.mqtt_client, config_topic, self.config).await?;
        }

        Ok(DaliBusResult::None)
    }

    pub async fn run(&mut self, config_filename: &str) -> Result<(), Box<dyn Error>> {
        let config_topic = &self.get_config_topic();
        let status_topic = &self.get_status_topic();

        self.mqtt_client.publish(&MqttDali::get_is_active_topic(&self.config.name), QoS::AtLeastOnce, true, "true".as_bytes()).await?;
        MqttDali::publish_config(&self.mqtt_client, config_topic, self.config).await?;

        let command_topic = &self.get_command_topic();
        self.mqtt_client.subscribe(command_topic, QoS::AtLeastOnce).await?;

        loop {
            let event = self.mqtt_events.poll().await?;

            if let Event::Incoming(Packet::Publish(Publish { ref topic, payload, ..})) = event {
                if topic == command_topic {
                    let mut republish_config = true;  // Should the configuration republished after command execution

                    match serde_json::from_slice(payload.as_ref()) as serde_json::Result<DaliCommand> {
                        Ok(command) => {
                            debug!("Got command {:?}", command);
                            let command_result = match command {
                                DaliCommand::SetLightBrightness { bus, address, value} => { republish_config = false;  self.dali_manager.set_light_brightness_async(bus, address, value).await },
                                DaliCommand::SetGroupBrightness { bus, group, value } => { republish_config = false; self.dali_manager.set_group_brightness_async(bus, group, value).await },
                                DaliCommand::UpdateBusStatus => self.update_bus_status(),
                                DaliCommand::RenameBus { bus: bus_number, ref name } => self.rename_bus(bus_number, name),
                                DaliCommand::RenameLight { bus, address, ref name } => self.rename_light(bus, address, name),
                                DaliCommand::RenameGroup { bus, group, ref name } => self.rename_group(bus, group, name),
                                DaliCommand::NewGroup { bus, group } => self.new_group(bus, group),
                                DaliCommand::MatchGroup { bus, group, ref light_name_pattern} => self.match_group(bus, group, light_name_pattern),
                                DaliCommand::RemoveGroup { bus, group } => self.remove_group(bus, group),
                                DaliCommand::AddToGroup {bus, group, address} => { self.add_to_group(bus, group, address) },
                                DaliCommand::RemoveFromGroup {bus, group, address} => { self.remove_from_group(bus, group, address) },
                                DaliCommand::FindAllLights { bus } => self.find_lights(config_topic, bus, DaliDeviceSelection::All).await,
                                DaliCommand::FindNewLights { bus } => self.find_lights(config_topic, bus, DaliDeviceSelection::WithoutShortAddress).await,
                            };

                            if let Err(e) = command_result {
                                let error_message = serde_json::to_string(&format!("Command {:?} completed with error {}", command, e))?;

                                error!("{}", error_message);
                                self.mqtt_client.publish(status_topic, QoS::AtMostOnce, false, error_message.as_bytes()).await?;
                            } else {
                                self.mqtt_client.publish(status_topic, QoS::AtMostOnce, false, "\"OK\"".as_bytes()).await?;
                                if republish_config {
                                    MqttDali::publish_config(&self.mqtt_client, config_topic, self.config).await?;
                                    self.config.save(config_filename).expect("Saving config file");
                                }
                            }
                        },
                        Err(e) => error!("Invalid payload received on {}: {}", command_topic, e),
                    }
                        
                }
                else {
                    error!("Got publish on unexpected topic {}", topic);
                }
            }
        }
    }
}
