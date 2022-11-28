use std::time::Duration;
use log::{error, debug};
use thiserror::Error;
use rumqttc::{MqttOptions, AsyncClient, EventLoop, QoS, Event, Packet, Publish, LastWill, ClientError, ConnectionError};
use crate::dali_manager::{DaliManager, DaliBusResult, DaliBusIterator, DaliDeviceSelection, DaliManagerError, MatchGroupAction};
use crate::command_payload::{DaliCommand, QueryLightReply};
use crate::config_payload::{Config,  Group, BusStatus};


pub struct MqttDali<'a> {
    config: &'a mut Config,
    mqtt_client: AsyncClient,
    mqtt_events: EventLoop,
    dali_manager: &'a mut DaliManager<'a>,
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("Invalid bus number: {0}")]
    BusNumber(usize),

    #[error("Invalid short address: {0}")]
    ShortAddress(u8),

    #[error("Invalid group address: {0}")]
    GroupAddress(u8),

    #[error("Bus {0} has no power")]
    BusHasNoPower(usize),

    #[error("Bus {0} is overloaded")]
    BusOverloaded(usize),

    #[error("Bus {0} has invalid status")]
    InvalidBusStatus(usize),

    #[error("No more groups can be added to bus {0}")]
    NoMoreGroups(usize),

    #[error("Bus {0} has no group {1}")]
    NoSuchGroup(usize, u8),

    #[error("DALI error: {0:?}")]
    DaliManagerError(#[from] DaliManagerError),

    #[error("MQTT client error {0}")]
    MqttClientError(#[from] ClientError),

    #[error("MQTT connection error {0}")]
    MqttConnectionError(#[from] ConnectionError),

    #[error("Json Error {0}")]
    JsonError(#[from] serde_json::Error),
}

type Result<T> = std::result::Result<T, CommandError>;

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
        format!("DALI/Controllers/{}/Command", self.config.name)
    }

    fn get_status_topic(&self) -> String {
        format!("DALI/Status/{}", self.config.name)
    }

    fn get_config_topic(&self) -> String {
        format!("DALI/Config/{}", self.config.name)
    }

    fn get_is_active_topic(name: &str) -> String {
        format!("DALI/Active/{}", name)
    }

    fn get_light_reply_topic(&self, command: &str, bus: usize, short_address: u8) -> String {
        format!("DALI/Reply/{}/{}/Bus_{}/Address_{}", command, self.config.name, bus, short_address)
    }

    async fn publish_config(client: &AsyncClient, config_topic: &str, config: &Config) -> Result<()> {
        Ok(client.publish(config_topic, QoS::AtLeastOnce, true, serde_json::to_vec(config)?).await?)
    }

    fn update_bus_status(&mut self) -> Result<DaliBusResult> {
        for (bus_number, bus) in self.config.buses.iter_mut().enumerate() {
            bus.status = self.dali_manager.controller.get_bus_status(bus_number)?;
        }

        Ok(DaliBusResult::None)
    }

    fn check_bus_status(bus_number: usize, status: &BusStatus) -> Result<DaliBusResult> {
        match status {
            BusStatus::Active => Ok(DaliBusResult::None),
            BusStatus::NoPower => Err(CommandError::BusHasNoPower(bus_number)),
            BusStatus::Overloaded => Err(CommandError::BusOverloaded(bus_number)),
            BusStatus::Unknown => Err(CommandError::InvalidBusStatus(bus_number)),
        }

    }
    fn check_bus(&mut self, bus_number: usize) -> Result<DaliBusResult> {
        self.update_bus_status()?;

        if let Some(bus) = self.config.buses.get(bus_number) {
            MqttDali::check_bus_status(bus_number, &bus.status)
        } else {
            Err(CommandError::BusNumber(bus_number))
        }
    }
 
    fn rename_bus(&mut self, bus_number: usize, name: &str) -> Result<DaliBusResult> {
        if let Some(bus) = self.config.buses.get_mut(bus_number) {
            bus.description = name.to_owned();
            Ok(DaliBusResult::None)
        } else {
            Err(CommandError::BusNumber(bus_number))
        }
    }

    fn rename_light(&mut self, bus_number: usize, short_address: u8, name: &str) ->  Result<DaliBusResult> {
        if let Some(bus) = self.config.buses.get_mut(bus_number) {
            if let Some(channel) = bus.channels.iter_mut().find(|c| c.short_address == short_address) {
                channel.description = name.to_owned();
                Ok(DaliBusResult::None)
            } else {
                Err(CommandError::ShortAddress(short_address))
            }
        } else {
            Err(CommandError::BusNumber(bus_number))
        }
    }

    fn rename_group(&mut self, bus_number: usize, group_address: u8, name: &str) ->  Result<DaliBusResult> {
        if let Some(bus) = self.config.buses.get_mut(bus_number) {
            if let Some(group) = bus.groups.iter_mut().find(|g| g.group_address == group_address) {
                group.description = name.to_owned();
                Ok(DaliBusResult::None)
            } else {
                Err(CommandError::GroupAddress(group_address))
            }
        } else {
            Err(CommandError::BusNumber(bus_number))
        }
    }

    fn new_group(&mut self, bus_number: usize) -> Result<DaliBusResult> {
        if let Some(bus) = self.config.buses.get_mut(bus_number) {
            let group_address = (0u8..16u8).find(|group_address| !bus.groups.iter().any(|group| group.group_address == *group_address));

            if let Some(group_address) = group_address {
                bus.groups.push( Group { description: format!("Group {}", group_address), group_address, members: Vec::new() });
                Ok(DaliBusResult::None)
                
            } else {
                Err(CommandError::NoMoreGroups(bus_number))
            }
        }  else {
            Err(CommandError::BusNumber(bus_number))
        }
    }

    fn remove_group(&mut self, bus_number: usize, group_address: u8) -> Result<DaliBusResult> {
        if let Some(bus) = self.config.buses.get_mut(bus_number) {
            MqttDali::check_bus_status(bus_number, &bus.status)?;

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
                Err(CommandError::NoSuchGroup(bus_number, group_address))
            }
        }  else {
            Err(CommandError::BusNumber(bus_number))
        }

    }

    fn add_to_group(&mut self, bus_number: usize, group_address: u8, short_address: u8) -> Result<DaliBusResult> {
        if let Some(bus) = self.config.buses.get_mut(bus_number) {
            let group = bus.groups.iter_mut().find(|g| g.group_address == group_address);

            // Create group if not found
            if group.is_none() {
                bus.groups.push( Group { description: format!("Group {}", group_address), group_address, members: Vec::new()});
            }

            MqttDali::check_bus_status(bus_number, &bus.status)?;
            self.dali_manager.add_to_group_and_verify(bus_number, group_address, short_address)?;

            let group = bus.groups.iter_mut().find(|g| g.group_address == group_address).unwrap();
            if !group.members.contains(&short_address) {
                group.members.push(short_address);
            }

            Ok(DaliBusResult::None)
        }  else {
            Err(CommandError::BusNumber(bus_number))
        }
    }

    fn remove_from_group(&mut self, bus_number: usize, group_address: u8, short_address: u8) -> Result<DaliBusResult> {
        if let Some(bus) = self.config.buses.get_mut(bus_number) {
            if let Some(group) = bus.groups.iter_mut().find(|g| g.group_address == group_address) {
                if let Some(index) = group.members.iter().position(|m| *m== short_address) {
                    MqttDali::check_bus_status(bus_number, &bus.status)?;
                    self.dali_manager.remove_from_group_and_verify(bus_number, group_address, short_address)?;
                    group.members.remove(index);
                }
                Ok(DaliBusResult::None)
            } else {
                Err(CommandError::NoSuchGroup(bus_number, group_address))
            }

        }  else {
            Err(CommandError::BusNumber(bus_number))
        }
    }

    fn match_group(&mut self, bus_number: usize, group_address: u8, light_name_pattern: &str) -> Result<DaliBusResult> {
        if let Some(bus) = self.config.buses.get_mut(bus_number) {
            MqttDali::check_bus_status(bus_number, &bus.status)?;

            self.dali_manager.match_group(bus, group_address, light_name_pattern, Option::<Box<dyn Fn(MatchGroupAction, &str)>>::None)?;
            Ok(DaliBusResult::None)
        }  else {
            Err(CommandError::BusNumber(bus_number))
        }
    }

    async fn query_light_status(&mut self, bus: usize, short_address: u8) -> Result<DaliBusResult> {
        let light_status = self.dali_manager.query_light_status(bus, short_address);
        let query_light_reply = match light_status {
            Ok(light_status) => QueryLightReply::new(&self.config.name, bus, short_address, light_status),
            Err(e) => QueryLightReply::new_failure(&self.config.name, bus, short_address, e),
        };
        let topic = self.get_light_reply_topic("QueryLightStatus", bus, short_address);

        self.mqtt_client.publish(topic, QoS::AtMostOnce, false, serde_json::to_vec(&query_light_reply)?).await?;

        Ok(DaliBusResult::None)
    }

    async fn find_lights(&mut self, config_topic: &str, bus_number: usize, selection: DaliDeviceSelection) -> Result<DaliBusResult> {
        self.check_bus(bus_number)?;

        if matches!(selection, DaliDeviceSelection::All) {
            let bus = self.config.buses.get_mut(bus_number).unwrap();

            bus.channels.clear();
        }

        let mut device_iterator = DaliBusIterator::new(self.dali_manager, bus_number, selection, Option::<Box<dyn Fn(u8, u8)>>::None)?;

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

    pub async fn run(&mut self, config_filename: &str) -> Result<()> {
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
                            let command_result: Result<DaliBusResult> = match command {
                                DaliCommand::SetLightBrightness { bus, address, value} => { 
                                    republish_config = false;
                                    self.dali_manager.set_light_brightness_async(bus, address, value).await.map_err(CommandError::DaliManagerError)
                                },
                                DaliCommand::SetGroupBrightness { bus, group, value } => {
                                    republish_config = false;
                                    self.dali_manager.set_group_brightness_async(bus, group, value).await.map_err(CommandError::DaliManagerError)
                                },
                                DaliCommand::UpdateBusStatus => self.update_bus_status(),
                                DaliCommand::RenameBus { bus: bus_number, ref name } => self.rename_bus(bus_number, name),
                                DaliCommand::RenameLight { bus, address, ref name } => self.rename_light(bus, address, name),
                                DaliCommand::RenameGroup { bus, group, ref name } => self.rename_group(bus, group, name),
                                DaliCommand::NewGroup { bus } => self.new_group(bus),
                                DaliCommand::MatchGroup { bus, group, ref pattern} => self.match_group(bus, group, pattern),
                                DaliCommand::RemoveGroup { bus, group } => self.remove_group(bus, group),
                                DaliCommand::AddToGroup {bus, group, address} => self.add_to_group(bus, group, address),
                                DaliCommand::RemoveFromGroup {bus, group, address} => self.remove_from_group(bus, group, address),
                                DaliCommand::FindAllLights { bus } => self.find_lights(config_topic, bus, DaliDeviceSelection::All).await,
                                DaliCommand::FindNewLights { bus } => self.find_lights(config_topic, bus, DaliDeviceSelection::WithoutShortAddress).await,
                                DaliCommand::QueryLightStatus { bus, address } => { republish_config = false; self.query_light_status(bus, address).await },
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
