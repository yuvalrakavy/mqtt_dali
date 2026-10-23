//! One managed owner for durable inventory and physical routing.
use crate::{
    config::ConfigPaths,
    inventory::{Edit, Model},
};
use mqtt_bridge_kit::{RequestContext, TargetGenerations};
use std::sync::Arc;
use tokio::{
    sync::{broadcast, mpsc, oneshot, watch, OwnedSemaphorePermit, Semaphore},
    time::{timeout_at, Instant},
};

pub mod commissioning;
pub mod discovery;
pub mod group_output;
pub mod groups;
mod hat;
pub mod identification;
pub mod input;
pub mod output;
mod owner;
mod physical;
mod proxy;
pub mod reassignment;
pub mod restoration;
mod routing;
pub mod scene_settings;
pub mod scenes;
pub use proxy::DaliProxy;
pub(crate) use proxy::{projected_states, PreparationError, PreparedCommand};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    AwaitingRuntime,
    Ready,
    Writing,
    Uncertain(String),
    Stopping,
    Closed,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hardware {
    NotStarted,
    Closed,
    Connecting,
    Initializing,
    Resynchronizing,
    Ready {
        version: crate::atx::wire::Version,
        buses: Vec<BusHealth>,
    },
    Unavailable {
        reason: String,
        retrying: bool,
    },
    WaitingLateReply {
        remaining: u8,
    },
    ResetRequired,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusHealth {
    pub bus: u8,
    pub status: u8,
    pub channels: u8,
}
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub model: Arc<Model>,
    pub phase: Phase,
    pub hardware: Hardware,
    pub outputs: std::collections::BTreeMap<String, output::Observation>,
    pub group_outputs: std::collections::BTreeMap<String, group_output::Observation>,
    pub inputs: std::collections::BTreeMap<String, input::Observation>,
    /// Routes with an installed receive boundary, independently of device presence.
    pub input_listeners: std::collections::BTreeSet<String>,
    pub identification: Option<identification::Status>,
    pub version: &'static str,
    pub build_utc: &'static str,
}
#[allow(dead_code)]
mod build_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}
/// Public metadata changes. Physical changes require verified commissioning jobs.
#[derive(Debug, Clone)]
pub enum MetadataEdit {
    CreateScene {
        bus: u8,
        slot: u8,
        name: String,
        description: Option<String>,
    },
    RenameScene {
        bus: u8,
        slot: u8,
        name: String,
    },
    DescribeScene {
        bus: u8,
        slot: u8,
        description: Option<String>,
    },
    /// Forget the name only; native scene contents are not cleared.
    RemoveScene {
        bus: u8,
        slot: u8,
    },
    CreateGroup {
        bus: u8,
        group: u8,
        name: String,
    },
    /// Forget the saved fixture and its saved group references; no hardware write.
    RemoveOutput {
        bus: u8,
        address: u8,
    },
    /// Forget a saved module and every child instance; no hardware write.
    RemoveInput {
        bus: u8,
        address: u8,
    },
    ConfigureBus {
        bus: u8,
        name: String,
        enabled: bool,
    },
    OutputColorRepresentation {
        bus: u8,
        address: u8,
        representation: Option<crate::config::ColorRepresentation>,
    },
    RenameInput {
        bus: u8,
        address: u8,
        name: String,
    },
    RenameInputInstance {
        bus: u8,
        address: u8,
        instance: u8,
        name: String,
    },
    RenameOutput {
        bus: u8,
        address: u8,
        name: String,
    },
    RenameGroup {
        bus: u8,
        address: u8,
        name: String,
    },
    RenameBus {
        bus: u8,
        name: String,
    },
    Description(Option<String>),
}
impl MetadataEdit {
    fn into_edit(self) -> Edit {
        match self {
            Self::CreateScene {
                bus,
                slot,
                name,
                description,
            } => Edit::AddScene {
                bus,
                scene: crate::config::Scene {
                    slot,
                    name,
                    description,
                    contents: Vec::new(),
                },
            },
            Self::RenameScene { bus, slot, name } => Edit::RenameScene { bus, slot, name },
            Self::DescribeScene {
                bus,
                slot,
                description,
            } => Edit::SceneDescription {
                bus,
                slot,
                description,
            },
            Self::RemoveScene { bus, slot } => Edit::RemoveScene { bus, slot },
            Self::CreateGroup { bus, group, name } => Edit::AddGroup {
                bus,
                group: crate::config::Group {
                    group_address: group,
                    name,
                    description: None,
                    members: Vec::new(),
                },
            },
            Self::RemoveOutput { bus, address } => Edit::RemoveOutput { bus, address },
            Self::RemoveInput { bus, address } => Edit::RemoveInput { bus, address },
            Self::ConfigureBus { bus, name, enabled } => Edit::ConfigureBus { bus, name, enabled },
            Self::OutputColorRepresentation {
                bus,
                address,
                representation,
            } => Edit::OutputColorRepresentation {
                bus,
                address,
                representation,
            },
            Self::RenameInput { bus, address, name } => Edit::RenameInput { bus, address, name },
            Self::RenameInputInstance {
                bus,
                address,
                instance,
                name,
            } => Edit::RenameInputInstance {
                bus,
                address,
                instance,
                name,
            },
            Self::RenameOutput { bus, address, name } => Edit::RenameOutput { bus, address, name },
            Self::RenameGroup { bus, address, name } => Edit::RenameGroup { bus, address, name },
            Self::RenameBus { bus, name } => Edit::RenameBus { bus, name },
            Self::Description(description) => Edit::Description { description },
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ServiceError {
    #[error("service is busy")]
    Busy,
    #[error("service is closed")]
    Closed,
    #[error("request expired before admission")]
    Expired,
    #[error("request refused: {0}")]
    Rejected(String),
    #[error("outcome uncertain; inspect status before retrying: {0}")]
    Uncertain(String),
}
#[derive(Clone)]
pub struct ServiceHandle {
    send: mpsc::Sender<Message>,
    state: watch::Receiver<Option<Snapshot>>,
    updates: broadcast::Sender<Snapshot>,
    input_events: broadcast::Sender<input::Notice>,
    permit: Arc<Semaphore>,
    identification: watch::Receiver<Option<identification::Claim>>,
    #[cfg(test)]
    input_event_pause: Arc<tests::Pause>,
}
/// Constructing this object opens no files, UART or network connections.
pub struct DaliService {
    identification: watch::Sender<Option<identification::Claim>>,
    paths: ConfigPaths,
    instance: String,
    receive: mpsc::Receiver<Message>,
    state: watch::Sender<Option<Snapshot>>,
    handle: ServiceHandle,
    uart: Option<hat::Settings>,
    web: Option<crate::web::Settings>,
    #[cfg(test)]
    hooks: crate::config_store::test_hooks::Hooks,
    #[cfg(test)]
    activation_hook: Option<ActivationHook>,
    #[cfg(test)]
    output_acceptance_hook: Option<OutputAcceptanceHook>,
    #[cfg(test)]
    proxy_read_pause: Option<Arc<tests::Pause>>,
    #[cfg(test)]
    proxy_snapshot_pause: Option<Arc<tests::Pause>>,
}
#[cfg(test)]
type ActivationHook = Arc<dyn Fn(&mqtt_bridge_kit::Reporter, &TargetGenerations) + Send + Sync>;
#[cfg(test)]
type OutputAcceptanceHook = Arc<dyn Fn(&mut hat::Hat) + Send + Sync>;
enum Message {
    Restore(Box<restoration::Request>),
    Reassignment(reassignment::Request),
    SceneSetting(Box<scene_settings::Request>),
    Scene(scenes::Request),
    Identify(identification::Request),
    Groups(groups::Request),
    Commission(commissioning::Request),
    Discovery(discovery::Request),
    Output(output::Request),
    OutputSettings(output::settings::Request),
    Input(input::readback::Request),
    InputSettings(input::settings::Request),
    InputFeedback(input::feedback::Request),
    GroupOutput(group_output::Request),
    Attach(
        mqtt_bridge_kit::Reporter,
        oneshot::Sender<Result<(), String>>,
    ),
    Change(Request),
}
enum Change {
    Edit { expected: u64, edit: MetadataEdit },
    Recover,
}
struct Request {
    change: Change,
    context: RequestContext,
    completion: Option<mqtt_bridge_kit::CommandCompletion>,
    deadline: Instant,
    reply: oneshot::Sender<Result<Snapshot, ServiceError>>,
    _permit: OwnedSemaphorePermit,
}
impl DaliService {
    #[cfg(test)]
    pub(crate) fn with_config_test_hooks(
        mut self,
        hooks: crate::config_store::test_hooks::Hooks,
    ) -> Self {
        self.hooks = hooks;
        self
    }
    /// Enable authenticated HTTP inside the managed lifetime; opens no listener.
    pub fn with_web(mut self, settings: crate::web::Settings) -> Self {
        self.web = Some(settings);
        self
    }
    /// Enable a real UART/PTY. Construction performs no I/O; firmware version
    /// never supplies an implicit repeat policy.
    pub fn with_uart(
        mut self,
        path: impl Into<std::path::PathBuf>,
        repeat: crate::atx::transactions::RepeatPolicy,
    ) -> Self {
        self.uart = Some(hat::Settings::new(path.into(), repeat));
        self
    }
    /// Select a raw TCP HAT instead of UART. Each connection must start a fresh
    /// remote HAT parser/output session. Construction performs no IO; the
    /// original repeat policy and quiet/setup handling still apply.
    pub fn with_hat_tcp(
        mut self,
        address: std::net::SocketAddr,
        repeat: crate::atx::transactions::RepeatPolicy,
    ) -> Self {
        self.uart = Some(hat::Settings::tcp(address, repeat));
        self
    }
    /// Select measured quiet recovery after enabling a UART or TCP HAT. Does not open it.
    pub fn with_uart_quiet_period(mut self, quiet: std::time::Duration) -> Result<Self, String> {
        if quiet < std::time::Duration::from_millis(1) || quiet > std::time::Duration::from_secs(5)
        {
            return Err("UART quiet period must be between 1 ms and 5 s".into());
        }
        self.uart
            .as_mut()
            .ok_or("enable a HAT endpoint before configuring its quiet period")?
            .quiet = quiet;
        Ok(self)
    }
    pub fn new(paths: ConfigPaths, instance: String) -> (Self, ServiceHandle) {
        let (send, receive) = mpsc::channel(4);
        let (state, read) = watch::channel(None);
        let (identification, identification_read) = watch::channel(None);
        let handle = ServiceHandle {
            send,
            state: read,
            updates: broadcast::channel(16).0,
            input_events: broadcast::channel(input::EVENT_CAPACITY).0,
            permit: Arc::new(Semaphore::new(1)),
            identification: identification_read,
            #[cfg(test)]
            input_event_pause: Arc::default(),
        };
        (
            Self {
                identification,
                paths,
                instance,
                receive,
                state,
                handle: handle.clone(),
                uart: None,
                web: None,
                #[cfg(test)]
                hooks: Default::default(),
                #[cfg(test)]
                activation_hook: None,
                #[cfg(test)]
                output_acceptance_hook: None,
                #[cfg(test)]
                proxy_read_pause: None,
                #[cfg(test)]
                proxy_snapshot_pause: None,
            },
            handle,
        )
    }
}
impl ServiceHandle {
    /// Owned snapshot. After notification lag, resnapshot instead of replaying edits.
    pub fn snapshot(&self) -> Option<Snapshot> {
        self.state.borrow().clone()
    }
    pub fn subscribe(&self) -> broadcast::Receiver<Snapshot> {
        self.updates.subscribe()
    }
    /// Native actions are non-retained. Lag means lost actions, never replay State.
    pub fn subscribe_inputs(&self) -> broadcast::Receiver<input::Notice> {
        self.input_events.subscribe()
    }
    /// Change saved metadata using the current inventory revision and original
    /// item token (controller token for bus/description edits). A lost response
    /// does not undo a save: inspect the owned snapshot before retrying.
    pub async fn edit(
        &self,
        expected: u64,
        edit: MetadataEdit,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Snapshot, ServiceError> {
        self.submit(Change::Edit { expected, edit }, context, deadline, None)
            .await
    }
    /// Resolve a frozen uncertain save/activation. A Ready snapshot needs no
    /// recovery; clients may have missed its already-confirmed result.
    pub async fn recover(
        &self,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Snapshot, ServiceError> {
        self.submit(Change::Recover, context, deadline, None).await
    }
    async fn submit(
        &self,
        change: Change,
        context: RequestContext,
        deadline: Instant,
        completion: Option<mqtt_bridge_kit::CommandCompletion>,
    ) -> Result<Snapshot, ServiceError> {
        if Instant::now() >= deadline {
            return Err(ServiceError::Expired);
        }
        if self.permit.is_closed() {
            return Err(ServiceError::Closed);
        }
        let _permit = if identification::active(self) {
            // WAIT: v2-identification-permit
            timeout_at(deadline, self.permit.clone().acquire_owned())
                .await
                .map_err(|_| ServiceError::Expired)?
                .map_err(|_| ServiceError::Closed)?
        } else {
            self.permit
                .clone()
                .try_acquire_owned()
                .map_err(|_| ServiceError::Busy)?
        };
        let snapshot = self
            .snapshot()
            .ok_or(ServiceError::Rejected("service is starting".into()))?;
        routing::admit(&snapshot, &change, &context)?;
        let (reply, result) = oneshot::channel();
        self.send
            .try_send(Message::Change(Request {
                change,
                context,
                completion,
                deadline,
                reply,
                _permit,
            }))
            .map_err(|_| ServiceError::Closed)?;
        // WAIT: v2-service-request
        let result = timeout_at(deadline, result).await;
        if Instant::now() >= deadline {
            return Err(ServiceError::Uncertain("caller deadline".into()));
        }
        result
            .map_err(|_| ServiceError::Uncertain("caller deadline".into()))?
            .map_err(|_| ServiceError::Closed)?
    }
    async fn attach(&self, reporter: mqtt_bridge_kit::Reporter) -> Result<(), String> {
        let (reply, result) = oneshot::channel();
        self.send
            .try_send(Message::Attach(reporter, reply))
            .map_err(|_| "service attachment unavailable")?;
        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        // WAIT: v2-service-attach
        let result = timeout_at(deadline, result).await;
        if Instant::now() >= deadline {
            return Err("service attachment deadline".into());
        }
        result
            .map_err(|_| "service attachment deadline")?
            .map_err(|_| "service ended during attachment")?
    }
}
#[cfg(test)]
mod tests;
