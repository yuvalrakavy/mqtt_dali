//! Explicit read-only memory operations under the existing physical HAT owner.
use super::*;
use crate::{
    atx::{
        connection::Connection,
        transactions::{TransactionError, Transactions},
    },
    dali::{energy, memory as codec},
};
use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub(super) enum Action {
    Identity,
    Luminaire,
    Energy(energy::Bank),
    Read { bank: u8, window: codec::Window },
}
impl Action {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Identity => "read_identity",
            Self::Luminaire => "read_luminaire_data",
            Self::Energy(_) => "read_energy_data",
            Self::Read { .. } => "read_memory",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocationReading {
    pub address: u8,
    pub reading: Option<scenes::Reading>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub operation: &'static str,
    pub outcome: &'static str,
    pub target: String,
    pub target_generation: String,
    pub revision: u64,
    pub bus: u8,
    pub address: u8,
    pub space: discovery::Space,
    pub bank: u8,
    pub start: u8,
    pub count: u8,
    pub quality: &'static str,
    pub version: Option<scenes::Reading>,
    pub readings: Vec<LocationReading>,
    pub identity: Option<codec::Identity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub luminaire: Option<crate::dali::luminaire::Data>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub energy: Option<EnergyReport>,
    pub reason: Option<String>,
    pub configuration_changed: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnergyReport {
    #[serde(flatten)]
    pub data: energy::Data,
    pub snapshot: Option<SnapshotProof>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SnapshotProof {
    pub start: scenes::Reading,
    pub end: scenes::Reading,
    pub release: scenes::Reading,
}
#[derive(Clone)]
pub(super) struct Route {
    pub controller: String,
    pub bus: u8,
    pub context: RequestContext,
    pub revision: u64,
    pub action: Action,
    pub space: discovery::Space,
    native: codec::Target,
}
pub(super) struct Request {
    pub target: String,
    pub action: Action,
    pub context: RequestContext,
    pub deadline: Instant,
    pub revision: u64,
    pub reply: oneshot::Sender<Result<Report, ServiceError>>,
    pub _permit: OwnedSemaphorePermit,
}
pub(super) struct Observation {
    pub report: Report,
    pub stamp: output::Stamp,
    pub stale: bool,
}
pub(super) struct Outcome {
    pub observation: Observation,
    pub error: Option<ServiceError>,
    pub recovery: bool,
}
pub(super) struct Completion {
    pub request: Request,
    pub observation: Option<Observation>,
    pub error: Option<ServiceError>,
}
fn reject(reason: impl ToString) -> ServiceError {
    ServiceError::Rejected(reason.to_string())
}
pub(super) fn resolve(
    snapshot: &Snapshot,
    target: &str,
    action: Action,
    cause: &RequestContext,
) -> Result<Route, ServiceError> {
    if snapshot.phase != Phase::Ready {
        return Err(reject("routing is not ready"));
    }
    let projection = snapshot
        .model
        .projections()
        .into_iter()
        .find(|projection| projection.target == target)
        .ok_or_else(|| reject("unknown memory target"))?;
    let (bus, native, space) = match projection.location {
        crate::inventory::Location::Output { bus, short_address } => (
            bus,
            codec::Target::Output(
                crate::dali::gear::ShortAddress::new(short_address).map_err(reject)?,
            ),
            discovery::Space::Outputs,
        ),
        crate::inventory::Location::Input { bus, short_address } => (
            bus,
            codec::Target::Input(crate::dali::input::Address::new(short_address).map_err(reject)?),
            discovery::Space::Inputs,
        ),
        _ => {
            return Err(reject(
                "memory requires an individual output or input module",
            ))
        }
    };
    if matches!(action, Action::Luminaire | Action::Energy(_)) && space != discovery::Space::Outputs
    {
        return Err(reject(
            "luminaire/energy data requires an individual output fixture",
        ));
    }
    if !snapshot
        .model
        .inventory()
        .buses
        .iter()
        .any(|saved| saved.bus == bus && saved.enabled)
    {
        return Err(reject("bus is disabled"));
    }
    let generations = routing::generations(&snapshot.model);
    let token = &generations.targets[target];
    if cause
        .generation()
        .is_some_and(|generation| generation != token)
    {
        return Err(reject("retired memory target generation"));
    }
    Ok(Route {
        controller: snapshot.model.inventory().name.clone(),
        bus,
        native,
        space,
        revision: snapshot.model.inventory().revision,
        action,
        context: RequestContext::new(token.clone())
            .map_err(reject)?
            .with_traceparent(cause.traceparent()),
    })
}
fn raw(reading: &scenes::Reading) -> Option<u8> {
    match reading {
        scenes::Reading::Value { raw } => u8::try_from(*raw).ok(),
        _ => None,
    }
}
async fn location(
    bus: &commissioning::Bus<'_>,
    native: codec::Target,
    address: u8,
) -> Result<LocationReading, commissioning::Error> {
    bus.command(native.offset(address), false).await?;
    Ok(LocationReading {
        address,
        reading: Some(scenes::Reading::byte(
            bus.query(native.read_frame()).await?,
            false,
        )),
    })
}
fn extend_capture(report: &mut Report, end: u8) {
    report
        .readings
        .extend((report.count..=end).map(|address| LocationReading {
            address,
            reading: None,
        }));
    report.count = end + 1;
}
fn captured_bytes(report: &Report) -> Vec<Option<u8>> {
    report
        .readings
        .iter()
        .map(|item| item.reading.as_ref().and_then(raw))
        .collect()
}
async fn read_pending(
    report: &mut Report,
    bus: &commissioning::Bus<'_>,
    native: codec::Target,
) -> Result<(), commissioning::Error> {
    for item in &mut report.readings {
        if item.reading.is_none() {
            *item = location(bus, native, item.address).await?;
        }
    }
    Ok(())
}
async fn latch_read(
    bus: &commissioning::Bus<'_>,
    native: codec::Target,
) -> Result<scenes::Reading, commissioning::Error> {
    Ok(location(bus, native, 2)
        .await?
        .reading
        .expect("location was queried"))
}
async fn latch_write(
    bus: &commissioning::Bus<'_>,
    native: codec::Target,
    latch: energy::Latch,
) -> Result<scenes::Reading, commissioning::Error> {
    let codec::Target::Output(address) = native else {
        return Err(commissioning::Error::Evidence(
            "energy latch requires an output",
        ));
    };
    bus.command(energy::enable_write_memory(address), true)
        .await?;
    bus.command(native.offset(2), false).await?;
    bus.command(latch.frame(), false).await?;
    latch_read(bus, native).await
}
async fn capture_energy(
    report: &mut Report,
    bus: &commissioning::Bus<'_>,
    native: codec::Target,
    cleanup_recovery: &mut bool,
) -> Result<SnapshotProof, commissioning::Error> {
    // A previous interrupted read may have left AA. Prove a fresh FF→AA
    // transition rather than treating an old snapshot as a new capture.
    if report.readings[2].reading.as_ref().and_then(raw) != Some(255) {
        let baseline = latch_write(bus, native, energy::Latch::Release).await?;
        if raw(&baseline) != Some(255) {
            return Err(commissioning::Error::Evidence(
                "energy latch baseline was not confirmed",
            ));
        }
    }
    let result: Result<(scenes::Reading, scenes::Reading), commissioning::Error> = async {
        let start = latch_write(bus, native, energy::Latch::Freeze).await?;
        if raw(&start) != Some(170) {
            return Err(commissioning::Error::Evidence(
                "energy latch was not confirmed",
            ));
        }
        let last = report.readings[0]
            .reading
            .as_ref()
            .and_then(raw)
            .expect("header checked");
        extend_capture(report, last.min(15));
        read_pending(report, bus, native).await?;
        let end = latch_read(bus, native).await?;
        if raw(&end) != Some(170) {
            return Err(commissioning::Error::Evidence(
                "energy latch changed during capture",
            ));
        }
        Ok((start, end))
    }
    .await;
    // Once AA may have been submitted, release on success and failed evidence.
    // The same Bus enforces the original stamp/deadline, including cleanup.
    let release = latch_write(bus, native, energy::Latch::Release).await;
    *cleanup_recovery |= matches!(
        &release,
        Err(commissioning::Error::Transaction(
            TransactionError::StreamLoss
        ))
    );
    let (start, end) = result?;
    let release = release?;
    if raw(&release) != Some(255) {
        return Err(commissioning::Error::Evidence(
            "energy latch release was not confirmed",
        ));
    }
    Ok(SnapshotProof {
        start,
        end,
        release,
    })
}
pub(super) async fn execute(
    link: Transactions<Connection>,
    route: Route,
    stamp: output::Stamp,
    deadline: Instant,
    target: String,
) -> Outcome {
    let bus = commissioning::Bus {
        link: &link,
        bus: route.bus,
        space: route.space,
        stamp,
        deadline,
    };
    let (bank, window) = match route.action {
        Action::Read { bank, window } => (bank, window),
        Action::Identity => (0, codec::Window::new(0, 1).unwrap()),
        Action::Luminaire => (1, codec::Window::new(0, 1).unwrap()),
        Action::Energy(bank) => (bank.number(), codec::Window::new(0, 1).unwrap()),
    };
    let mut report = Report {
        operation: route.action.name(),
        outcome: "observed",
        target,
        target_generation: route.context.generation().unwrap().into(),
        revision: route.revision,
        bus: route.bus,
        address: route.native.address(),
        space: route.space,
        bank,
        start: window.start(),
        count: window.count(),
        quality: "unavailable",
        version: None,
        readings: window
            .addresses()
            .map(|address| LocationReading {
                address,
                reading: None,
            })
            .collect(),
        identity: None,
        luminaire: matches!(route.action, Action::Luminaire)
            .then(|| crate::dali::luminaire::decode(&[])),
        energy: match route.action {
            Action::Energy(bank) => Some(EnergyReport {
                data: energy::decode(bank, &[]),
                snapshot: None,
            }),
            _ => None,
        },
        reason: None,
        configuration_changed: false,
    };
    let mut cleanup_recovery = false;
    let result: Result<(), commissioning::Error> = async {
        if matches!(route.action, Action::Identity) {
            report.version = Some(scenes::Reading::byte(
                bus.query(route.native.version()).await?,
                false,
            ));
        }
        bus.command(route.native.bank(bank), false).await?;
        if !matches!(route.action, Action::Read { .. }) {
            report.readings[0] = location(&bus, route.native, 0).await?;
            let Some(last) = report.readings[0].reading.as_ref().and_then(raw) else {
                return Ok(());
            };
            if last == 255 {
                report.quality = "partial";
                report.reason = Some(format!(
                    "bank{bank} last accessible location is reserved FF"
                ));
                return Ok(());
            }
            let end = match route.action {
                Action::Identity => {
                    codec::Layout::from_version(report.version.as_ref().and_then(raw))
                        .last_location()
                }
                Action::Energy(_) => 3,
                _ => 0x12,
            };
            extend_capture(&mut report, last.min(end));
        }
        read_pending(&mut report, &bus, route.native).await?;
        if let Action::Energy(bank) = route.action {
            let data = energy::decode(bank, &captured_bytes(&report));
            let snapshot = if data.format_status == energy::FormatStatus::Supported {
                Some(capture_energy(&mut report, &bus, route.native, &mut cleanup_recovery).await?)
            } else {
                None
            };
            report.energy = Some(EnergyReport {
                data: energy::decode(bank, &captured_bytes(&report)),
                snapshot,
            });
        }
        if matches!(route.action, Action::Luminaire) {
            use crate::dali::luminaire::{decode, FormatStatus};
            if decode(&captured_bytes(&report)).format_status == FormatStatus::Supported {
                let last = report.readings[0]
                    .reading
                    .as_ref()
                    .and_then(raw)
                    .expect("header checked above");
                extend_capture(&mut report, last.min(0x77));
                read_pending(&mut report, &bus, route.native).await?;
            }
            report.luminaire = Some(decode(&captured_bytes(&report)));
        }
        let complete = report
            .readings
            .iter()
            .all(|item| item.reading.as_ref().and_then(raw).is_some());
        report.quality = if complete {
            "complete"
        } else if report
            .readings
            .iter()
            .any(|item| item.reading.as_ref().and_then(raw).is_some())
        {
            "partial"
        } else {
            "unavailable"
        };
        if matches!(route.action, Action::Identity) {
            let version = report.version.as_ref().and_then(raw);
            let identity = codec::identity(version, &captured_bytes(&report));
            if identity.layout == codec::Layout::Unknown
                || report.count <= identity.layout.last_location()
            {
                report.quality = "partial";
            }
            report.identity = Some(identity);
        }
        if report.luminaire.as_ref().is_some_and(|data| {
            data.format_status != crate::dali::luminaire::FormatStatus::Supported
                || report.count < 120
        }) {
            report.quality = "partial";
        }
        if report.energy.as_ref().is_some_and(|data| {
            data.data.format_status != energy::FormatStatus::Supported || report.count < 16
        }) {
            report.quality = "partial";
        }
        Ok(())
    }
    .await;
    let recovery = cleanup_recovery
        || matches!(
            &result,
            Err(commissioning::Error::Transaction(
                TransactionError::StreamLoss
            ))
        );
    let error = result.err().map(|error| {
        reject(match error {
            commissioning::Error::Transaction(error) => error.to_string(),
            commissioning::Error::Evidence(reason) => reason.into(),
        })
    });
    Outcome {
        observation: Observation {
            report,
            stamp,
            stale: false,
        },
        error,
        recovery,
    }
}
pub(super) fn accept(
    snapshot: &Snapshot,
    completed: &mut Completion,
    hat: &hat::Hat,
) -> Result<Report, ServiceError> {
    if let Some(error) = completed.error.take() {
        return Err(error);
    }
    let observation = completed
        .observation
        .take()
        .ok_or_else(|| reject("memory read produced no final evidence"))?;
    let request = &completed.request;
    let route = resolve(snapshot, &request.target, request.action, &request.context)?;
    if Instant::now() >= request.deadline
        || observation.stale
        || !hat.evidence_current(route.bus, observation.stamp)
        || route.revision != request.revision
        || route.revision != observation.report.revision
        || route.context.generation() != Some(observation.report.target_generation.as_str())
    {
        return Err(reject("memory evidence changed before acceptance"));
    }
    Ok(observation.report)
}
pub(super) fn finish(request: Request, result: Result<Report, ServiceError>) {
    drop(request._permit);
    let _ = request.reply.send(result);
}
impl ServiceHandle {
    /// Capture one standard Part252 bank using a verified temporary latch.
    pub async fn read_energy_data(
        &self,
        target: &str,
        bank: energy::Bank,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.memory(target, Action::Energy(bank), context, deadline, None)
            .await
    }
    /// Read a bounded bank range. No memory write or persistent setting is sent.
    pub async fn read_memory(
        &self,
        target: &str,
        bank: u8,
        window: codec::Window,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.memory(
            target,
            Action::Read { bank, window },
            context,
            deadline,
            None,
        )
        .await
    }
    /// Read bank0 identity using the observed standard version's layout.
    pub async fn read_identity(
        &self,
        target: &str,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.memory(target, Action::Identity, context, deadline, None)
            .await
    }
    /// Read supported Part251 OEM/ratings/text data without programming memory.
    pub async fn read_luminaire_data(
        &self,
        target: &str,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Report, ServiceError> {
        self.memory(target, Action::Luminaire, context, deadline, None)
            .await
    }
    pub(super) async fn memory(
        &self,
        target: &str,
        action: Action,
        context: RequestContext,
        deadline: Instant,
        admitted_revision: Option<u64>,
    ) -> Result<Report, ServiceError> {
        let deadline = deadline.min(Instant::now() + Duration::from_secs(15));
        if Instant::now() >= deadline {
            return Err(ServiceError::Expired);
        }
        if self.permit.is_closed() {
            return Err(ServiceError::Closed);
        }
        let permit = self
            .permit
            .clone()
            .try_acquire_owned()
            .map_err(|_| ServiceError::Busy)?;
        let route = resolve(
            &self.snapshot().ok_or(ServiceError::Closed)?,
            target,
            action,
            &context,
        )?;
        // Prepared MQTT/HTTP commands carry their parse-time revision through
        // any queue delay. Typed calls capture it here before their first await.
        if admitted_revision.is_some_and(|revision| revision != route.revision) {
            return Err(reject(
                "original memory revision changed before service admission",
            ));
        }
        let revision = route.revision;
        let original = route.context;
        let (reply, receive) = oneshot::channel();
        self.send
            .try_send(Message::Memory(Request {
                target: target.into(),
                action,
                context: original.clone(),
                deadline,
                revision,
                reply,
                _permit: permit,
            }))
            .map_err(|_| ServiceError::Closed)?;
        // WAIT: v2-service-request
        let result = timeout_at(deadline, receive)
            .await
            .map_err(|_| ServiceError::Expired)?
            .map_err(|_| ServiceError::Closed)??;
        if Instant::now() >= deadline {
            return Err(ServiceError::Expired);
        }
        let route = resolve(
            &self.snapshot().ok_or(ServiceError::Closed)?,
            target,
            action,
            &original,
        )?;
        if route.revision != revision {
            return Err(reject("memory mapping changed before reply"));
        }
        Ok(result)
    }
}
