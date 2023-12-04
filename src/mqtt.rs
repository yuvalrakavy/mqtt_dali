use crate::command_payload::{DaliCommand, QueryLightReply};
use crate::config_payload::{BusStatus, DaliConfig, Group};
use crate::dali_manager::{
    DaliBusIterator, DaliBusResult, DaliDeviceSelection, DaliManager, MatchGroupAction,
};
use crate::{get_version, Config};
use error_stack::{Report, ResultExt};
use log::{debug, error, info};
use rumqttc::{AsyncClient, Event, EventLoop, LastWill, MqttOptions, Packet, Publish, QoS};
use std::time::Duration;
use thiserror::Error;

pub struct MqttDali<'a> {
    dali_config: &'a mut DaliConfig,
    // mqtt_client: AsyncClient,
    // mqtt_events: EventLoop,
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

    // #[error("MQTT client error {0}")]
    // MqttClientError(#[from] ClientError),

    // #[error("MQTT connection error {0}")]
    // MqttConnectionError(#[from] ConnectionError),

    // #[error("Json Error {0}")]
    // JsonError(#[from] serde_json::Error),
    #[error("In context of '{0}'")]
    Context(String),
}

type Result<T> = std::result::Result<T, Report<CommandError>>;

impl<'a> MqttDali<'a> {
    fn get_command_topic(&self) -> String {
        format!("DALI/Controllers/{}/Command", self.dali_config.name)
    }

    fn get_status_topic(&self) -> String {
        format!("DALI/Status/{}", self.dali_config.name)
    }

    fn get_config_topic(&self) -> String {
        format!("DALI/Config/{}", self.dali_config.name)
    }

    fn get_is_active_topic(name: &str) -> String {
        format!("DALI/Active/{}", name)
    }

    fn get_version_topic(name: &str) -> String {
        format!("DALI/Version/{}", name)
    }

    fn get_light_reply_topic(&self, command: &str, bus: usize, short_address: u8) -> String {
        format!(
            "DALI/Reply/{}/{}/Bus_{}/Address_{}",
            command, self.dali_config.name, bus, short_address
        )
    }

    async fn publish_config(
        client: &AsyncClient,
        config_topic: &str,
        dali_config: &DaliConfig,
    ) -> Result<()> {
        let into_context =
            || CommandError::Context(format!("MQTT: Publish configuration to {config_topic}"));

        client
            .publish(
                config_topic,
                QoS::AtLeastOnce,
                true,
                serde_json::to_vec(dali_config).change_context_lazy(into_context)?,
            )
            .await
            .change_context_lazy(into_context)
    }

    fn update_bus_status(&mut self) -> Result<DaliBusResult> {
        let into_context = || CommandError::Context("MQTT: UpdateBusStatus command".to_owned());

        for (bus_number, bus) in self.dali_config.buses.iter_mut().enumerate() {
            bus.status = self
                .dali_manager
                .controller
                .get_bus_status(bus_number)
                .change_context_lazy(into_context)?;
        }

        Ok(DaliBusResult::None)
    }

    fn check_bus_status(bus_number: usize, status: &BusStatus) -> Result<DaliBusResult> {
        let into_context =
            || CommandError::Context(format!("MQTT: Checking bus {bus_number} status"));

        match status {
            BusStatus::Active => Ok(DaliBusResult::None),
            BusStatus::NoPower => {
                Err(CommandError::BusHasNoPower(bus_number)).change_context_lazy(into_context)
            }
            BusStatus::Overloaded => {
                Err(CommandError::BusOverloaded(bus_number)).change_context_lazy(into_context)
            }
            BusStatus::Unknown => {
                Err(CommandError::InvalidBusStatus(bus_number)).change_context_lazy(into_context)
            }
        }
    }

    fn check_bus(&mut self, bus_number: usize) -> Result<DaliBusResult> {
        let into_context =
            || CommandError::Context(format!("MQTT Checking bus {bus_number} status"));

        self.update_bus_status()?;

        if let Some(bus) = self.dali_config.buses.get(bus_number) {
            MqttDali::check_bus_status(bus_number, &bus.status)
        } else {
            Err(CommandError::BusNumber(bus_number)).change_context_lazy(into_context)
        }
    }

    fn rename_bus(&mut self, bus_number: usize, name: &str) -> Result<DaliBusResult> {
        if let Some(bus) = self.dali_config.buses.get_mut(bus_number) {
            bus.description = name.to_owned();
            Ok(DaliBusResult::None)
        } else {
            Err(CommandError::BusNumber(bus_number)).change_context(CommandError::Context(format!(
                "MQTT: Renaming bus {bus_number} to {name}"
            )))
        }
    }

    fn rename_light(
        &mut self,
        bus_number: usize,
        short_address: u8,
        name: &str,
    ) -> Result<DaliBusResult> {
        let into_context = || {
            CommandError::Context(format!(
                "MQTT: Renaming light {short_address} on bus {bus_number} to {name}"
            ))
        };

        if let Some(bus) = self.dali_config.buses.get_mut(bus_number) {
            if let Some(channel) = bus
                .channels
                .iter_mut()
                .find(|c| c.short_address == short_address)
            {
                channel.description = name.to_owned();
                Ok(DaliBusResult::None)
            } else {
                Err(CommandError::ShortAddress(short_address)).change_context_lazy(into_context)
            }
        } else {
            Err(CommandError::BusNumber(bus_number)).change_context_lazy(into_context)
        }
    }

    fn rename_group(
        &mut self,
        bus_number: usize,
        group_address: u8,
        name: &str,
    ) -> Result<DaliBusResult> {
        let into_context = || {
            CommandError::Context(format!(
                "MQTT: Renaming group {group_address} on bus {bus_number} to {name}"
            ))
        };

        if let Some(bus) = self.dali_config.buses.get_mut(bus_number) {
            if let Some(group) = bus
                .groups
                .iter_mut()
                .find(|g| g.group_address == group_address)
            {
                group.description = name.to_owned();
                Ok(DaliBusResult::None)
            } else {
                Err(CommandError::GroupAddress(group_address)).change_context_lazy(into_context)
            }
        } else {
            Err(CommandError::BusNumber(bus_number)).change_context_lazy(into_context)
        }
    }

    fn new_group(&mut self, bus_number: usize) -> Result<DaliBusResult> {
        let into_context =
            || CommandError::Context(format!("MQTT: Create new group on bus {bus_number}"));

        if let Some(bus) = self.dali_config.buses.get_mut(bus_number) {
            let group_address = (0u8..16u8).find(|group_address| {
                !bus.groups
                    .iter()
                    .any(|group| group.group_address == *group_address)
            });

            if let Some(group_address) = group_address {
                bus.groups.push(Group {
                    description: format!("Group {}", group_address),
                    group_address,
                    members: Vec::new(),
                });
                Ok(DaliBusResult::None)
            } else {
                Err(CommandError::NoMoreGroups(bus_number)).change_context_lazy(into_context)
            }
        } else {
            Err(CommandError::BusNumber(bus_number)).change_context_lazy(into_context)
        }
    }

    fn remove_group(&mut self, bus_number: usize, group_address: u8) -> Result<DaliBusResult> {
        let into_context = || {
            CommandError::Context(format!(
                "MQTT: Remove group {group_address} from bus {bus_number}"
            ))
        };

        if let Some(bus) = self.dali_config.buses.get_mut(bus_number) {
            MqttDali::check_bus_status(bus_number, &bus.status)?;

            if let Some(index) = bus
                .groups
                .iter()
                .position(|g| g.group_address == group_address)
            {
                let group = bus.groups.get_mut(index).unwrap();

                // If group is not empty, remove membership of all members from this group
                if !group.members.is_empty()
                    && MqttDali::check_bus_status(bus_number, &bus.status).is_ok()
                {
                    for short_address in group.members.iter() {
                        self.dali_manager
                            .remove_from_group(bus_number, group_address, *short_address)
                            .change_context_lazy(into_context)?;
                    }
                }

                bus.groups.remove(index);
                Ok(DaliBusResult::None)
            } else {
                Err(CommandError::NoSuchGroup(bus_number, group_address))
                    .change_context_lazy(into_context)
            }
        } else {
            Err(CommandError::BusNumber(bus_number)).change_context_lazy(into_context)
        }
    }

    fn add_to_group(
        &mut self,
        bus_number: usize,
        group_address: u8,
        short_address: u8,
    ) -> Result<DaliBusResult> {
        let into_context = || {
            CommandError::Context(format!(
                "MQTT: Add light {short_address} to group {group_address} on bus {bus_number}"
            ))
        };

        if let Some(bus) = self.dali_config.buses.get_mut(bus_number) {
            let group = bus
                .groups
                .iter_mut()
                .find(|g| g.group_address == group_address);

            // Create group if not found
            if group.is_none() {
                bus.groups.push(Group {
                    description: format!("Group {}", group_address),
                    group_address,
                    members: Vec::new(),
                });
            }

            MqttDali::check_bus_status(bus_number, &bus.status)
                .change_context_lazy(into_context)?;
            self.dali_manager
                .add_to_group_and_verify(bus_number, group_address, short_address)
                .change_context_lazy(into_context)?;

            let group = bus
                .groups
                .iter_mut()
                .find(|g| g.group_address == group_address)
                .unwrap();
            if !group.members.contains(&short_address) {
                group.members.push(short_address);
            }

            Ok(DaliBusResult::None)
        } else {
            Err(CommandError::BusNumber(bus_number)).change_context_lazy(into_context)
        }
    }

    fn remove_from_group(
        &mut self,
        bus_number: usize,
        group_address: u8,
        short_address: u8,
    ) -> Result<DaliBusResult> {
        let into_context = || {
            CommandError::Context(format!(
                "MQTT: Remove light {short_address} from group {group_address} on bus {bus_number}"
            ))
        };

        if let Some(bus) = self.dali_config.buses.get_mut(bus_number) {
            if let Some(group) = bus
                .groups
                .iter_mut()
                .find(|g| g.group_address == group_address)
            {
                if let Some(index) = group.members.iter().position(|m| *m == short_address) {
                    MqttDali::check_bus_status(bus_number, &bus.status)
                        .change_context_lazy(into_context)?;
                    self.dali_manager
                        .remove_from_group_and_verify(bus_number, group_address, short_address)
                        .change_context_lazy(into_context)?;
                    group.members.remove(index);
                }
                Ok(DaliBusResult::None)
            } else {
                Err(CommandError::NoSuchGroup(bus_number, group_address))
                    .change_context_lazy(into_context)
            }
        } else {
            Err(CommandError::BusNumber(bus_number)).change_context_lazy(into_context)
        }
    }

    fn match_group(
        &mut self,
        bus_number: usize,
        group_address: u8,
        light_name_pattern: &str,
    ) -> Result<DaliBusResult> {
        let into_context = || {
            CommandError::Context(format!("MQTT: Match group {group_address} on bus {bus_number} to pattern {light_name_pattern}"))
        };

        if let Some(bus) = self.dali_config.buses.get_mut(bus_number) {
            MqttDali::check_bus_status(bus_number, &bus.status)
                .change_context_lazy(into_context)?;

            self.dali_manager
                .match_group(
                    bus,
                    group_address,
                    light_name_pattern,
                    Option::<Box<dyn Fn(MatchGroupAction, &str)>>::None,
                )
                .change_context_lazy(into_context)?;
            Ok(DaliBusResult::None)
        } else {
            Err(CommandError::BusNumber(bus_number)).change_context_lazy(into_context)
        }
    }

    async fn query_light_status(
        &mut self,
        mqtt_client: &AsyncClient,
        bus: usize,
        short_address: u8,
    ) -> Result<DaliBusResult> {
        let into_context =
            || CommandError::Context(format!("MQTT: Query light {short_address} on bus {bus}"));

        let light_status = self.dali_manager.query_light_status(bus, short_address);
        let query_light_reply = match light_status {
            Ok(light_status) => {
                QueryLightReply::new(&self.dali_config.name, bus, short_address, light_status)
            }
            Err(e) => QueryLightReply::new_failure(
                &self.dali_config.name,
                bus,
                short_address,
                &e.to_string(),
            ),
        };
        let topic = self.get_light_reply_topic("QueryLightStatus", bus, short_address);

        mqtt_client
            .publish(
                topic,
                QoS::AtMostOnce,
                false,
                serde_json::to_vec(&query_light_reply).change_context_lazy(into_context)?,
            )
            .await
            .change_context_lazy(into_context)?;

        Ok(DaliBusResult::None)
    }

    async fn remove_short_address(
        &mut self,
        bus_number: usize,
        short_address: u8,
    ) -> Result<DaliBusResult> {
        let into_context = || {
            CommandError::Context(format!(
                "MQTT: Remove short address {short_address} from bus {bus_number}"
            ))
        };

        if let Some(bus) = self.dali_config.buses.get_mut(bus_number) {
            MqttDali::check_bus_status(bus_number, &bus.status)
                .change_context_lazy(into_context)?;

            self.dali_manager
                .remove_short_address(bus, short_address)
                .change_context_lazy(into_context)?;

            Ok(DaliBusResult::None)
        } else {
            Err(CommandError::BusNumber(bus_number)).change_context_lazy(into_context)
        }
    }

    async fn find_lights(
        &mut self,
        mqtt_client: &AsyncClient,
        config_topic: &str,
        bus_number: usize,
        selection: DaliDeviceSelection,
    ) -> Result<DaliBusResult> {
        let into_context =
            || CommandError::Context(format!("MQTT: Find lights on bus {bus_number}"));

        self.check_bus(bus_number)
            .change_context_lazy(into_context)?;

        if matches!(selection, DaliDeviceSelection::All) {
            let bus = self.dali_config.buses.get_mut(bus_number).unwrap();

            bus.channels.clear();
        }

        let mut device_iterator = DaliBusIterator::new(
            self.dali_manager,
            bus_number,
            selection,
            Option::<Box<dyn Fn(u8, u8)>>::None,
        )
        .change_context_lazy(into_context)?;

        while device_iterator
            .find_next_device(self.dali_manager)
            .change_context_lazy(into_context)?
            .is_some()
        {
            let short_address = (0..64u8)
                .find(|short_address| {
                    !self.dali_config.buses[bus_number]
                        .channels
                        .iter()
                        .any(|channel| channel.short_address == *short_address)
                })
                .expect("Unable to find unused short address!!");

            self.dali_manager
                .program_short_address(bus_number, short_address)
                .change_context_lazy(into_context)?;
            {
                let bus = self.dali_config.buses.get_mut(bus_number).unwrap();
                bus.channels.push(crate::config_payload::Channel {
                    description: format!("Light {}", short_address),
                    short_address,
                });
            }

            MqttDali::publish_config(mqtt_client, config_topic, self.dali_config)
                .await
                .change_context_lazy(into_context)?;
        }

        Ok(DaliBusResult::None)
    }

    pub async fn run_session(
        &mut self,
        config: &Config,
        mqtt_client: AsyncClient,
        mut mqtt_events: EventLoop,
    ) -> Result<()> {
        let into_context = || CommandError::Context("MQTT session: Event loop".to_owned());
        let config_topic = &self.get_config_topic();
        let status_topic = &self.get_status_topic();

        info!("MQTT session started: Connecting to MQTT broker");
        let active_topic = MqttDali::get_is_active_topic(&self.dali_config.name);

        info!("Trying to set {active_topic} to true");

        mqtt_client
            .publish(&active_topic, QoS::AtLeastOnce, true, "true".as_bytes())
            .await
            .change_context_lazy(into_context)?;

        info!("MQTT {active_topic} was set to true");

        let version = get_version();
        mqtt_client
            .publish(
                &MqttDali::get_version_topic(&self.dali_config.name),
                QoS::AtLeastOnce,
                true,
                version.as_bytes(),
            )
            .await
            .change_context_lazy(into_context)?;

        MqttDali::publish_config(&mqtt_client, config_topic, self.dali_config)
            .await
            .change_context_lazy(into_context)?;

        let command_topic = &self.get_command_topic();
        mqtt_client
            .subscribe(command_topic, QoS::AtLeastOnce)
            .await
            .change_context_lazy(into_context)?;

        loop {
            let event = mqtt_events.poll().await.change_context_lazy(into_context)?;

            if let Event::Incoming(Packet::Publish(Publish {
                ref topic, payload, ..
            })) = event
            {
                if topic == command_topic {
                    let mut republish_config = true; // Should the configuration republished after command execution

                    match serde_json::from_slice(payload.as_ref())
                        as serde_json::Result<DaliCommand>
                    {
                        Ok(command) => {
                            debug!("Got command {:?}", command);
                            let command_result: Result<DaliBusResult> = match command {
                                DaliCommand::SetLightBrightness {
                                    bus,
                                    address,
                                    value,
                                } => {
                                    republish_config = false;
                                    self.dali_manager
                                        .set_light_brightness_async(bus, address, value)
                                        .await
                                        .change_context_lazy(|| CommandError::Context(format!("MQTT: SetLightBrightness command on bus {bus} address {address} value {value}")))
                                }
                                DaliCommand::SetGroupBrightness { bus, group, value } => {
                                    republish_config = false;
                                    self.dali_manager
                                        .set_group_brightness_async(bus, group, value)
                                        .await
                                        .change_context_lazy(|| CommandError::Context(format!("MQTT: SetGroupBrightness command on bus {bus} group {group} value {value}")))
                                }
                                DaliCommand::UpdateBusStatus => self.update_bus_status(),
                                DaliCommand::RenameBus {
                                    bus: bus_number,
                                    ref name,
                                } => self.rename_bus(bus_number, name),
                                DaliCommand::RenameLight {
                                    bus,
                                    address,
                                    ref name,
                                } => self.rename_light(bus, address, name),
                                DaliCommand::RenameGroup {
                                    bus,
                                    group,
                                    ref name,
                                } => self.rename_group(bus, group, name),
                                DaliCommand::NewGroup { bus } => self.new_group(bus),
                                DaliCommand::MatchGroup {
                                    bus,
                                    group,
                                    ref pattern,
                                } => self.match_group(bus, group, pattern),
                                DaliCommand::RemoveGroup { bus, group } => {
                                    self.remove_group(bus, group)
                                }
                                DaliCommand::AddToGroup {
                                    bus,
                                    group,
                                    address,
                                } => self.add_to_group(bus, group, address),
                                DaliCommand::RemoveFromGroup {
                                    bus,
                                    group,
                                    address,
                                } => self.remove_from_group(bus, group, address),
                                DaliCommand::FindAllLights { bus } => {
                                    self.find_lights(
                                        &mqtt_client,
                                        config_topic,
                                        bus,
                                        DaliDeviceSelection::All,
                                    )
                                    .await
                                }
                                DaliCommand::FindNewLights { bus } => {
                                    self.find_lights(
                                        &mqtt_client,
                                        config_topic,
                                        bus,
                                        DaliDeviceSelection::WithoutShortAddress,
                                    )
                                    .await
                                }
                                DaliCommand::QueryLightStatus { bus, address } => {
                                    republish_config = false;
                                    self.query_light_status(&mqtt_client, bus, address).await
                                }
                                DaliCommand::RemoveShortAddress { bus, address } => {
                                    republish_config = false;
                                    self.remove_short_address(bus, address).await
                                }
                            };

                            if let Err(e) = command_result {
                                let error_message = serde_json::to_string(&format!(
                                    "Command {:?} completed with error {}",
                                    command, e
                                ))
                                .change_context_lazy(into_context)?;

                                error!("{}", error_message);
                                mqtt_client
                                    .publish(
                                        status_topic,
                                        QoS::AtMostOnce,
                                        false,
                                        error_message.as_bytes(),
                                    )
                                    .await
                                    .change_context_lazy(into_context)?;
                            } else {
                                mqtt_client
                                    .publish(
                                        status_topic,
                                        QoS::AtMostOnce,
                                        false,
                                        "\"OK\"".as_bytes(),
                                    )
                                    .await
                                    .change_context_lazy(into_context)?;
                                if republish_config {
                                    MqttDali::publish_config(
                                        &mqtt_client,
                                        config_topic,
                                        self.dali_config,
                                    )
                                    .await
                                    .change_context_lazy(into_context)?;
                                    config.save(self.dali_config).expect("Saving config file");
                                }
                            }
                        }
                        Err(e) => error!("Invalid payload received on {}: {}", command_topic, e),
                    }
                } else {
                    error!("Got publish on unexpected topic {}", topic);
                }
            }
        }
    }

    pub fn new(
        dali_manager: &'a mut DaliManager<'a>,
        dali_config: &'a mut DaliConfig,
    ) -> MqttDali<'a> {
        MqttDali {
            dali_config,
            dali_manager,
        }
    }

    pub async fn run(
        config: &Config,
        dali_manager: &'a mut DaliManager<'a>,
        dali_config: &'a mut DaliConfig,
        mqtt_broker: &str,
    ) -> Result<()> {
        let name = dali_config.name.clone();
        let mut mqtt = MqttDali::new(dali_manager, dali_config);

        loop {
            info!("Connecting to MQTT broker");

            let client_id = format!("DALI-{}", name);
            let mut mqtt_options = MqttOptions::new(client_id, mqtt_broker, 1883);
            let last_will = LastWill::new(
                MqttDali::get_is_active_topic(&name),
                "false".as_bytes(),
                QoS::AtLeastOnce,
                true,
            );
            mqtt_options
                .set_keep_alive(Duration::from_secs(5))
                .set_last_will(last_will);

            let (mqtt_client, mqtt_events) = AsyncClient::new(mqtt_options, 10);

            match mqtt.run_session(config, mqtt_client, mqtt_events).await {
                Ok(_) => break Ok(()),
                Err(e) => {
                    info!("MQTT session terminated due to error: {e}, wait 10 seconds and try to reconnect");
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    info!("Reconnecting to MQTT broker");
                }
            }
        }
    }
}
