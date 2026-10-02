use crate::command_payload::{DaliCommand, QueryLightReply};
use crate::config_payload::{BusStatus, DaliConfig, Group};
use crate::dali_manager::{
    DaliBusIterator, DaliBusResult, DaliController, DaliDeviceSelection, DaliManager,
    MatchGroupAction,
};
use crate::{get_version, Config};
use error_stack::{Report, ResultExt};
use rumqttc::v5::{AsyncClient, Event, EventLoop, MqttOptions};
use rumqttc::v5::mqttbytes::QoS;
use rumqttc::v5::mqttbytes::v5::{LastWill, Packet, Publish, PublishProperties};
use rumqttc::Outgoing;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::{oneshot, watch};
use tracing::{debug, error, info, warn, Instrument};

pub struct MqttDali<'a> {
    dali_config: &'a mut DaliConfig,
    dali_manager: &'a mut DaliManager<'a>,
    /// The broker's reachability, across sessions.
    outage: Outage,
    saver: ConfigSaver,
}

/// How long the session waits for the configuration file to be written.
const SAVE_BOUND: Duration = Duration::from_secs(2);

/// Writes the configuration file off the session (fleet class F1): the write runs on the blocking
/// pool, and the session waits for it at most SAVE_BOUND, since everything the bridge does next
/// waits on the session. A write that has not finished by then is left running; while it is, a
/// later save is refused at once, so a filesystem that stopped answering holds one thread, not one
/// per command. The runtime's bounded shutdown leaves it behind (`dali-runtime-shutdown`).
#[derive(Default)]
struct ConfigSaver {
    /// A write that outlived SAVE_BOUND.
    stuck: Option<tokio::task::JoinHandle<std::io::Result<()>>>,
}

impl ConfigSaver {
    async fn save_config(&mut self, path: &str, dali_config: &DaliConfig) -> std::result::Result<(), String> {
        if let Some(stuck) = &self.stuck {
            if !stuck.is_finished() {
                return Err(format!("an earlier write of {path} has not finished"));
            }
            self.stuck = None;
        }
        let json = serde_json::to_vec_pretty(dali_config).map_err(|e| e.to_string())?;
        let file = path.to_owned();
        let mut write = tokio::task::spawn_blocking(move || std::fs::write(file, json));
        // WAIT: dali-config-save
        match tokio::time::timeout(SAVE_BOUND, &mut write).await {
            Ok(Ok(written)) => written.map_err(|e| e.to_string()),
            Ok(Err(e)) => Err(format!("the write ended abnormally: {e}")),
            Err(_) => {
                self.stuck = Some(write);
                Err(format!("the write did not finish within {} s", SAVE_BOUND.as_secs()))
            }
        }
    }
}

/// How long `run` waits between a failed session and the next.
const RECONNECT_DELAY: Duration = Duration::from_secs(10);

/// How long a stopping session waits for its goodbye (Active=false, DISCONNECT) to be written.
pub const GOODBYE_BOUND: Duration = Duration::from_secs(2);

/// A broker outage this long is a WARN, once per outage (fleet class F2).
const OUTAGE_WARN_AFTER: Duration = Duration::from_secs(30);

/// How long a connection accepted during an outage must hold before the outage counts as over:
/// longer than the reconnect delay (10 s), so two bridges taking one client id's session from each
/// other never look recovered, and than two keep-alive periods (6 s each), by which a broker that
/// accepts and then never answers has been found out.
const STABLE_AFTER: Duration = Duration::from_secs(15);

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

    #[error("Bus {0} has no free short address")]
    NoFreeShortAddress(usize),

    #[error("Mqtt Error {0}")]
    MqttError(String),

    #[error("In context of '{0}'")]
    Context(String),
}

pub(crate) type Result<T> = std::result::Result<T, Report<CommandError>>;

/// Build a `PublishProperties` stamped with the current traceparent, if any
/// trace is active. Returns `None` when no trace is active so callers can
/// use plain `publish` in that case.
fn traceparent_properties() -> Option<PublishProperties> {
    let tp = tracing_init::traceparent::current()?;
    let mut props = PublishProperties::default();
    props.user_properties.push(("traceparent".into(), tp));
    Some(props)
}

/// Publish `payload` on `topic` with `qos`/`retain`, stamping a
/// `traceparent` user property when a trace is active.
async fn publish_with_trace(
    client: &AsyncClient,
    topic: impl Into<String>,
    qos: QoS,
    retain: bool,
    payload: Vec<u8>,
) -> std::result::Result<(), rumqttc::v5::ClientError>
{
    let topic = topic.into();
    if let Some(props) = traceparent_properties() {
        // WAIT: mqtt-request
        client.publish_with_properties(topic, qos, retain, payload, props).await
    } else {
        // WAIT: mqtt-request
        client.publish(topic, qos, retain, payload).await
    }
}

/// What the pump hands the session.
enum PumpEvent {
    /// The broker accepted the connection.
    Connected,
    Publish(Publish),
    /// The session's DISCONNECT has been written: its goodbye is out.
    Disconnected,
    /// The connection failed; the session ends and `run` reconnects.
    Ended(String),
}

/// A forward queue past this many unread publishes is a WARN, once; back under `LOW_WATER`, an
/// INFO with how long it lasted. Nothing is dropped (Store no-hang §14.6, ruling 1).
const HIGH_WATER: usize = 1000;
const LOW_WATER: usize = 100;

/// The forward queue's depth, and whether its high-water WARN is standing.
#[derive(Default)]
struct Backlog {
    depth: AtomicUsize,
    high_since: Mutex<Option<Instant>>,
}

impl Backlog {
    // Each records under the lock and logs after releasing it: a log write can block (tracing-init's
    // console and file writers are synchronous), and the pump pushes here (finding C-10).
    fn pushed(&self) {
        let depth = self.depth.fetch_add(1, Ordering::SeqCst) + 1;
        if depth >= HIGH_WATER {
            let first = {
                // WAIT: mqtt-backlog-lock
                let mut high = self.high_since.lock().unwrap_or_else(|p| p.into_inner());
                if high.is_none() {
                    *high = Some(Instant::now());
                    true
                } else {
                    false
                }
            };
            if first {
                warn!(kind = "mqtt_backlog_high", depth, "MQTT commands are arriving faster than the bridge handles them");
            }
        }
    }

    fn popped(&self) {
        let depth = self.depth.fetch_sub(1, Ordering::SeqCst) - 1;
        if depth <= LOW_WATER {
            let since = {
                // WAIT: mqtt-backlog-lock
                let mut high = self.high_since.lock().unwrap_or_else(|p| p.into_inner());
                high.take()
            };
            if let Some(since) = since {
                info!(kind = "mqtt_backlog_drained", depth, lasted_ms = since.elapsed().as_millis() as u64, "MQTT command backlog drained");
            }
        }
    }

    /// The session ended: the commands it left unread are dropped with it, and a standing
    /// high-water episode is closed here, since no read will ever close it (finding C-6).
    fn session_ended(&self) {
        let discarded = self.depth.swap(0, Ordering::SeqCst);
        let since = {
            // WAIT: mqtt-backlog-lock
            let mut high = self.high_since.lock().unwrap_or_else(|p| p.into_inner());
            high.take()
        };
        if discarded > 0 {
            warn!(kind = "mqtt_commands_discarded", discarded, "MQTT session ended with commands not yet handled; they are dropped");
        }
        if let Some(since) = since {
            info!(kind = "mqtt_backlog_drained", depth = 0, discarded, lasted_ms = since.elapsed().as_millis() as u64, "MQTT command backlog drained");
        }
    }
}

/// Polls rumqttc's event loop in a task of its own (Store no-hang §14.3). rumqttc's request
/// channel drains only while the event loop is polled, so the task that polls waits on nothing
/// else — no publish, no subscribe, no bounded send — and forwards what arrives on an unbounded
/// queue. Dropping it stops the task.
struct Pump {
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Pump {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// The session's end of the pump's queue.
struct Incoming {
    rx: tokio::sync::mpsc::UnboundedReceiver<PumpEvent>,
    backlog: Arc<Backlog>,
}

impl Incoming {
    async fn recv(&mut self) -> Option<PumpEvent> {
        // WAIT: mqtt-pump-queue
        let event = self.rx.recv().await;
        if matches!(event, Some(PumpEvent::Publish(_))) {
            self.backlog.popped();
        }
        event
    }
}

impl Drop for Incoming {
    fn drop(&mut self) {
        self.backlog.session_ended();
    }
}

impl Pump {
    fn start(mut events: EventLoop) -> (Pump, Incoming) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let backlog = Arc::new(Backlog::default());
        let pushed = backlog.clone();
        let task = tokio::spawn(async move {
            loop {
                // WAIT: mqtt-poll
                let forward = match events.poll().await {
                    Ok(Event::Incoming(Packet::ConnAck(_))) => PumpEvent::Connected,
                    Ok(Event::Incoming(Packet::Publish(publish))) => {
                        pushed.pushed();
                        PumpEvent::Publish(publish)
                    }
                    Ok(Event::Outgoing(Outgoing::Disconnect)) => PumpEvent::Disconnected,
                    Ok(_) => continue,
                    Err(e) => {
                        let _ = tx.send(PumpEvent::Ended(e.to_string()));
                        return;
                    }
                };
                if tx.send(forward).is_err() {
                    return; // the session is gone
                }
            }
        });
        (Pump { task }, Incoming { rx, backlog })
    }
}

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

        publish_with_trace(
            client,
            config_topic,
            QoS::AtLeastOnce,
            true,
            serde_json::to_vec(dali_config).change_context_lazy(into_context)?,
        )
        .await
        .map_err(|e| CommandError::MqttError(e.to_string()))
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

    fn check_group_address(group_address: u8) -> Result<()> {
        if group_address < 16 {
            Ok(())
        } else {
            Err(Report::new(CommandError::GroupAddress(group_address)))
        }
    }

    fn check_short_address(short_address: u8) -> Result<()> {
        if short_address < 64 {
            Ok(())
        } else {
            Err(Report::new(CommandError::ShortAddress(short_address)))
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
            name.clone_into(&mut bus.description);
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
                name.clone_into(&mut channel.description);
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
                name.clone_into(&mut group.description);
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

        // Checked before the config is touched: a refused command leaves no group behind.
        MqttDali::check_group_address(group_address).change_context_lazy(into_context)?;
        MqttDali::check_short_address(short_address).change_context_lazy(into_context)?;

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

        // Checked before the config is touched: a refused command leaves no group behind.
        MqttDali::check_group_address(group_address).change_context_lazy(into_context)?;

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

        publish_with_trace(
            mqtt_client,
            topic,
            QoS::AtMostOnce,
            false,
            serde_json::to_vec(&query_light_reply).change_context_lazy(into_context)?,
        )
        .await
        .map_err(|e| CommandError::MqttError(e.to_string()))
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
            // A full bus is an error to report, not a panic on the session (finding C-35).
            let short_address = (0..64u8)
                .find(|short_address| {
                    !self.dali_config.buses[bus_number]
                        .channels
                        .iter()
                        .any(|channel| channel.short_address == *short_address)
                })
                .ok_or(CommandError::NoFreeShortAddress(bus_number))
                .change_context_lazy(into_context)?;

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
        mqtt_events: EventLoop,
        stop: &mut watch::Receiver<bool>,
    ) -> Result<()> {
        let config_topic = &self.get_config_topic();
        let status_topic = &self.get_status_topic();
        let mut status_ok = false;
        // Polling first, so every publish below has an event loop draining it (no-hang §14.3).
        let (_pump, mut incoming) = Pump::start(mqtt_events);

        debug!("MQTT session started: connecting to broker");
        let active_topic = MqttDali::get_is_active_topic(&self.dali_config.name);

        // Each session is one connection (a fresh client, a clean start), so this is every
        // ConnAck's work: subscribe, republish the retained model from the bridge's own state (a
        // broker that restarted lost it), and only then say Active=true — the bridge is listening
        // by the time anyone sees it (fleet class F4). One connection delivers these in order.
        let command_topic = &self.get_command_topic();
        // WAIT: mqtt-request
        mqtt_client
            .subscribe(command_topic, QoS::AtLeastOnce)
            .await
            .map_err(|e| CommandError::MqttError(e.to_string()))?;

        let version = get_version();
        publish_with_trace(
            &mqtt_client,
            &MqttDali::get_version_topic(&self.dali_config.name),
            QoS::AtLeastOnce,
            true,
            version.into_bytes(),
        )
        .await
        .map_err(|e| CommandError::MqttError(e.to_string()))?;

        MqttDali::publish_config(&mqtt_client, config_topic, self.dali_config)
            .await
            .map_err(|e| CommandError::MqttError(e.to_string()))?;

        publish_with_trace(&mqtt_client, &active_topic, QoS::AtLeastOnce, true, b"true".to_vec())
            .await
            .map_err(|e| CommandError::MqttError(e.to_string()))?;

        // When this connection, accepted during an outage, will have held long enough to end it.
        let mut holds_at: Option<tokio::time::Instant> = None;
        loop {
            // The session never polls: it waits on the pump's queue, and its publishes wait on
            // rumqttc's request channel, which the pump keeps draining (no-hang §14.3). A stop is
            // seen here, between commands: a command runs to its end.
            // WAIT: mqtt-pump-queue
            let event = tokio::select! {
                biased;
                () = stopped(stop) => {
                    self.goodbye(&mqtt_client, &mut incoming).await;
                    return Ok(());
                }
                () = tokio::time::sleep_until(holds_at.unwrap_or_else(tokio::time::Instant::now)), if holds_at.is_some() => {
                    holds_at = None;
                    self.outage.held();
                    continue;
                }
                event = incoming.recv() => event,
            };
            let publish = match event {
                Some(PumpEvent::Publish(publish)) => publish,
                Some(PumpEvent::Connected) => {
                    if self.outage.connected(Instant::now()) == OutageLog::Reconnected {
                        holds_at = Some(tokio::time::Instant::now() + STABLE_AFTER);
                    }
                    continue;
                }
                Some(PumpEvent::Disconnected) => continue,
                Some(PumpEvent::Ended(e)) => return Err(CommandError::MqttError(e).into()),
                None => return Err(CommandError::MqttError("the MQTT pump stopped".to_owned()).into()),
            };
            self.handle_publish(config, &mqtt_client, command_topic, config_topic, status_topic, &mut status_ok, publish)
                .await?;
        }
    }

    /// One publish from the broker: a command on the command topic is run, and its result
    /// reported on the status topic.
    #[allow(clippy::too_many_arguments)]
    async fn handle_publish(
        &mut self,
        config: &Config,
        mqtt_client: &AsyncClient,
        command_topic: &str,
        config_topic: &str,
        status_topic: &str,
        status_ok: &mut bool,
        publish: Publish,
    ) -> Result<()> {
        let into_context = || CommandError::Context("MQTT session: Event loop".to_owned());
        let topic = String::from_utf8_lossy(&publish.topic).into_owned();
        let Publish { ref payload, ref properties, .. } = publish;

        if topic == command_topic {
            // Extract and propagate traceparent from inbound MQTT v5 user properties.
            let traceparent = properties.as_ref().and_then(|p| {
                p.user_properties
                    .iter()
                    .find(|(k, _)| k == "traceparent")
                    .map(|(_, v)| v.clone())
            });

            let span = tracing::info_span!("mqtt_command", topic = %topic);
            if let Some(ref tp) = traceparent {
                tracing_init::traceparent::set_remote_parent(&span, tp);
            }

            // Span entry via .instrument(): never hold span.enter()
            // across .await — on the multi-thread runtime it corrupts
            // the current-span thread-local (fleet logging policy).
            async {
                    let mut republish_config = true; // Should the configuration republished after command execution

                    match serde_json::from_slice(payload.as_ref())
                        as serde_json::Result<DaliCommand>
                    {
                        Ok(command) => {
                            info!(command = ?command, "received command");

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
                                        mqtt_client,
                                        config_topic,
                                        bus,
                                        DaliDeviceSelection::All,
                                    )
                                    .await
                                }
                                DaliCommand::FindNewLights { bus } => {
                                    self.find_lights(
                                        mqtt_client,
                                        config_topic,
                                        bus,
                                        DaliDeviceSelection::WithoutShortAddress,
                                    )
                                    .await
                                }
                                DaliCommand::QueryLightStatus { bus, address } => {
                                    republish_config = false;
                                    self.query_light_status(mqtt_client, bus, address).await
                                }
                                DaliCommand::RemoveShortAddress { bus, address } => {
                                    self.remove_short_address(bus, address).await
                                }
                                DaliCommand::SetLightFadeTime {
                                    bus,
                                    address,
                                    fade_time,
                                } => {
                                    republish_config = false;
                                    self.dali_manager
                                        .set_light_fade_time(bus, address, fade_time)
                                        .change_context_lazy(|| CommandError::Context(format!("MQTT: SetLightFadeTime command on bus {bus} address {address} fade_time {fade_time}")))
                                }
                                DaliCommand::SetGroupFadeTime {
                                    bus,
                                    group,
                                    fade_time,
                                } => {
                                    republish_config = false;
                                    self.dali_manager
                                        .set_group_fade_time(bus, group, fade_time)
                                        .change_context_lazy(|| CommandError::Context(format!("MQTT: SetGroupFadeTime command on bus {bus} group {group} fade_time {fade_time}")))
                                }
                            };

                            if let Err(e) = command_result {
                                // `{:#}`: the whole chain, down to the cause (the plain form stops
                                // at the outermost context, which says only which command failed).
                                let error_message = serde_json::to_string(&format!(
                                    "Command {:?} completed with error {:#}",
                                    command, e
                                ))
                                .change_context_lazy(into_context)?;

                                // Command execution failed: this is a DALI-bus / validation
                                // failure — not a code bug, but operator attention is warranted.
                                // Not `external_failure`, which is a broker outage past its
                                // threshold: the command failed, the link did not.
                                warn!(kind = "command_rejected", error = format!("{e:#}"),
                                      "DALI command failed");

                                publish_with_trace(
                                    mqtt_client,
                                    status_topic,
                                    QoS::AtMostOnce,
                                    false,
                                    error_message.into_bytes(),
                                )
                                .await
                                .map_err(|e| CommandError::MqttError(e.to_string()))
                                .change_context_lazy(into_context)?;

                                *status_ok = false;
                            } else {
                                if !*status_ok {
                                    publish_with_trace(
                                        mqtt_client,
                                        status_topic,
                                        QoS::AtLeastOnce,
                                        false,
                                        b"\"OK\"".to_vec(),
                                    )
                                    .await
                                    .map_err(|e| CommandError::MqttError(e.to_string()))
                                    .change_context_lazy(into_context)?;
                                    *status_ok = true;
                                }

                                if republish_config {
                                    MqttDali::publish_config(
                                        mqtt_client,
                                        config_topic,
                                        self.dali_config,
                                    )
                                    .await
                                    .change_context_lazy(into_context)?;

                                    // The command took effect and the config is published; only
                                    // the file is behind. Reported, never a panic (finding C-35),
                                    // and bounded: a stalled write never holds the session (F1).
                                    if let Err(e) = self.saver.save_config(&config.config_filename, self.dali_config).await {
                                        warn!(kind = "config_save_failed", path = %config.config_filename, error = %e,
                                              "could not save the DALI configuration");
                                        let error_message = serde_json::to_string(&format!(
                                            "Command {:?} completed, but saving the configuration to {} failed: {}",
                                            command, config.config_filename, e
                                        ))
                                        .change_context_lazy(into_context)?;
                                        publish_with_trace(
                                            mqtt_client,
                                            status_topic,
                                            QoS::AtMostOnce,
                                            false,
                                            error_message.into_bytes(),
                                        )
                                        .await
                                        .map_err(|e| CommandError::MqttError(e.to_string()))
                                        .change_context_lazy(into_context)?;
                                        *status_ok = false;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!(kind = "decode_error", topic = %topic, error = %e,
                                  "invalid command payload received");
                        }
                    }
                    Ok(())
            }
            .instrument(span)
            .await
        } else {
            warn!(kind = "protocol_mismatch", topic = %topic,
                  "publish received on unexpected topic");
            Ok(())
        }
    }

    /// The session's goodbye, bounded by GOODBYE_BOUND: a retained Active=false and a DISCONNECT,
    /// queued without waiting, then a wait for the pump to have written the DISCONNECT. When the
    /// request channel is full or the write does not come in time, the goodbye is skipped: the
    /// session returns, its pump is dropped, the connection closes, and the broker's will
    /// publishes the same Active=false (no-hang §14.3: shutdown is bounded as a whole).
    async fn goodbye(&self, client: &AsyncClient, incoming: &mut Incoming) {
        let active_topic = MqttDali::get_is_active_topic(&self.dali_config.name);
        let queued = client
            .try_publish(&active_topic, QoS::AtLeastOnce, true, b"false".to_vec())
            .map_err(|e| e.to_string())
            .and_then(|()| client.try_disconnect().map_err(|e| e.to_string()));
        if let Err(e) = queued {
            info!(error = %e, "MQTT goodbye skipped (the request channel is full); the broker's will reports the bridge inactive");
            return;
        }
        // WAIT: mqtt-goodbye
        let written = tokio::time::timeout(GOODBYE_BOUND, async {
            loop {
                match incoming.recv().await {
                    Some(PumpEvent::Disconnected) | Some(PumpEvent::Ended(_)) | None => return,
                    // A command arriving now is not run: the bridge is stopping.
                    Some(_) => {}
                }
            }
        })
        .await;
        if written.is_err() {
            info!(bound_ms = GOODBYE_BOUND.as_millis() as u64,
                  "MQTT goodbye not written in time; the broker's will reports the bridge inactive");
        }
    }

    pub fn new(
        dali_manager: &'a mut DaliManager<'a>,
        dali_config: &'a mut DaliConfig,
    ) -> MqttDali<'a> {
        MqttDali {
            dali_config,
            dali_manager,
            outage: Outage::default(),
            saver: ConfigSaver::default(),
        }
    }

    /// Runs sessions until `stop` is set, reconnecting after each failure.
    pub async fn run(
        config: &Config,
        dali_manager: &'a mut DaliManager<'a>,
        dali_config: &'a mut DaliConfig,
        mqtt_broker: &str,
        mut stop: watch::Receiver<bool>,
    ) -> Result<()> {
        let name = dali_config.name.clone();
        let mut mqtt = MqttDali::new(dali_manager, dali_config);

        loop {
            if *stop.borrow() {
                return Ok(());
            }
            debug!("connecting to MQTT broker");

            let client_id = format!("DALI-{}", name);
            let (host, port) = broker_host_port(mqtt_broker);
            let mut mqtt_options = MqttOptions::new(client_id, host, port);
            let last_will = LastWill::new(
                MqttDali::get_is_active_topic(&name),
                "false".as_bytes().to_vec(),
                QoS::AtLeastOnce,
                true,
                None,
            );
            mqtt_options
                .set_keep_alive(Duration::from_secs(6))
                .set_last_will(last_will)
                .set_max_packet_size(Some(50 * 1024))
                .set_request_channel_capacity(200);

            let (mqtt_client, mqtt_events) = AsyncClient::new(mqtt_options, 200);

            match mqtt.run_session(config, mqtt_client, mqtt_events, &mut stop).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    // Session ended due to broker/network failure — designed recovery path.
                    mqtt.outage.failed(Instant::now(), &e.to_string());
                    // WAIT: mqtt-reconnect-delay
                    tokio::select! {
                        () = tokio::time::sleep(RECONNECT_DELAY) => {}
                        () = stopped(&mut stop) => return Ok(()),
                    }
                }
            }
        }
    }
}

/// Resolves once the bridge is asked to stop, or once nothing is left that could ask.
async fn stopped(stop: &mut watch::Receiver<bool>) {
    // WAIT: dali-stop
    let _ = stop.wait_for(|stop| *stop).await;
}

/// The broker's reachability across sessions, logged as one episode per outage (fleet class F2):
/// INFO `connection_lost` on its first failed attempt, DEBUG for every later one, ONE WARN
/// `external_failure` once it has lasted OUTAGE_WARN_AFTER (with `attempts` and `down_for_ms`), and
/// INFO `external_recovered` (with `down_for_ms` and `attempts`) once a connection has held for
/// STABLE_AFTER. A ConnAck alone ends nothing: a broker that accepts a connection and then drops it
/// is still down.
#[derive(Default)]
struct Outage {
    /// When the current outage began; `None` while the broker is reachable.
    since: Option<Instant>,
    /// The outage's failed attempts: sessions that failed, or never connected.
    attempts: u32,
    warned: bool,
    /// When the connection that may end the outage was accepted.
    reconnected: Option<Instant>,
}

/// What `Outage` logged, for its tests.
#[derive(Debug, PartialEq)]
enum OutageLog {
    /// The first failure of an outage: the INFO.
    Lost,
    /// A later failure: DEBUG.
    Retry,
    /// The failure that took the outage past OUTAGE_WARN_AFTER: the WARN.
    Warned,
    /// A connection accepted during an outage, which has yet to hold: DEBUG.
    Reconnected,
    /// That connection held: the outage is over.
    Recovered { down_for: Duration, attempts: u32 },
    /// A connection accepted outside any outage.
    Connected,
    /// Nothing to say.
    Quiet,
}

impl Outage {
    /// A session ended (or never connected) with `error`.
    fn failed(&mut self, now: Instant, error: &str) -> OutageLog {
        // A connection that ends before it held proved nothing: the outage goes on.
        self.reconnected = None;
        let Some(since) = self.since else {
            self.since = Some(now);
            self.attempts = 1;
            self.warned = false;
            info!(kind = "connection_lost", error, "MQTT connection lost; reconnecting every 10 s");
            return OutageLog::Lost;
        };
        self.attempts += 1;
        let attempts = self.attempts;
        let down_for = now.saturating_duration_since(since);
        let down_for_ms = down_for.as_millis() as u64;
        if !self.warned && down_for >= OUTAGE_WARN_AFTER {
            self.warned = true;
            warn!(kind = "external_failure", down_for_ms, attempts, error,
                  "MQTT broker unreachable, or each connection fails; still reconnecting every 10 s");
            OutageLog::Warned
        } else {
            debug!(down_for_ms, attempts, error, "MQTT reconnect failed");
            OutageLog::Retry
        }
    }

    /// The broker accepted a connection (its ConnAck).
    fn connected(&mut self, now: Instant) -> OutageLog {
        if self.since.is_none() {
            info!("connected to the MQTT broker");
            return OutageLog::Connected;
        }
        self.reconnected = Some(now);
        debug!(holds_for_ms = STABLE_AFTER.as_millis() as u64,
               "MQTT broker accepted a connection; the outage ends once it holds");
        OutageLog::Reconnected
    }

    /// The connection accepted at the last `connected` has held for STABLE_AFTER.
    fn held(&mut self) -> OutageLog {
        let (Some(since), Some(at)) = (self.since, self.reconnected) else {
            return OutageLog::Quiet;
        };
        self.since = None;
        self.reconnected = None;
        let down_for = at.saturating_duration_since(since);
        let attempts = self.attempts;
        info!(kind = "external_recovered", down_for_ms = down_for.as_millis() as u64, attempts,
              "MQTT broker reachable again");
        OutageLog::Recovered { down_for, attempts }
    }
}

/// What the bridge runs: the DALI controller, its config, and where the broker is.
pub struct Bridge {
    pub config: Config,
    pub controller: Box<dyn DaliController>,
    pub dali_config: DaliConfig,
    pub broker: String,
}

/// Runs the bridge on a thread of its own until `stop` is set; the receiver gets `run`'s result,
/// or closes without one if the session panicked (logged here, at ERROR).
///
/// The session's DALI calls are synchronous: while one runs, the thread running the session waits
/// on the bus (each wait bounded by `hat_line`, but one command can make many). On a thread of its
/// own that holds nothing else: the pump still runs on the runtime's workers, so the connection
/// stays alive, and `main`'s shutdown deadline still runs, so a stop is bounded however long the
/// command is (findings C-7, C-34).
pub fn spawn(
    runtime: tokio::runtime::Handle,
    bridge: Bridge,
    stop: watch::Receiver<bool>,
) -> std::io::Result<oneshot::Receiver<Result<()>>> {
    let (done_tx, done_rx) = oneshot::channel();
    std::thread::Builder::new().name("dali-session".into()).spawn(move || {
        let Bridge { config, mut controller, mut dali_config, broker } = bridge;
        let mut dali_manager = DaliManager::new(&mut *controller);
        let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // WAIT: dali-session-thread
            runtime.block_on(MqttDali::run(&config, &mut dali_manager, &mut dali_config, &broker, stop))
        }));
        match run {
            Ok(result) => {
                let _ = done_tx.send(result);
            }
            Err(panic) => {
                let payload = panic
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| panic.downcast_ref::<String>().cloned())
                    .unwrap_or_default();
                error!(kind = "panic", payload, "the DALI session panicked");
            }
        }
    })?;
    Ok(done_rx)
}

/// `host` or `host:port` (default 1883).
fn broker_host_port(broker: &str) -> (&str, u16) {
    match broker.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() => match port.parse() {
            Ok(port) => (host, port),
            Err(_) => (broker, 1883),
        },
        _ => (broker, 1883),
    }
}

#[cfg(test)]
mod tests;
