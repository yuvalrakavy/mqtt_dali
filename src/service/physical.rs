//! One concrete owner for input reads and output operations on the same HAT.
use super::{discovery, input::readback, output, ServiceError};
use crate::atx::{serial::Serial, transactions::Transactions};
use tokio::time::Instant;

pub(super) enum Work {
    Discovery(discovery::Request, discovery::Route),
    Output(output::Request, output::Route),
    Input(readback::Request, readback::Route),
}
impl Work {
    pub fn bus(&self) -> u8 {
        match self {
            Self::Output(_, r) => r.bus,
            Self::Input(_, r) => r.bus,
            Self::Discovery(_, r) => r.bus,
        }
    }
    pub fn deadline(&self) -> Instant {
        match self {
            Self::Output(q, _) => q.deadline,
            Self::Input(q, _) => q.deadline,
            Self::Discovery(q, _) => q.deadline,
        }
    }
    pub fn is_closed(&self) -> bool {
        match self {
            Self::Output(q, _) => q.reply.is_closed(),
            Self::Input(q, _) => q.reply.is_closed(),
            Self::Discovery(q, _) => q.reply.is_closed(),
        }
    }
    pub fn mutating(&self) -> bool {
        matches!(self, Self::Output(q,_) if q.intent.is_some())
    }
    pub fn span(&self) -> tracing::Span {
        match self {
            Self::Output(_, r) => r.context.operation_span(&r.controller, "output"),
            Self::Input(_, r) => r.context.operation_span(&r.controller, "input_read"),
            Self::Discovery(_, r) => r.context.operation_span(&r.controller, "discover_address"),
        }
    }
    pub async fn wait_for_receiver_close(&mut self) {
        match self {
            Self::Discovery(q, _) => {
                // WAIT: v2-hat-loop
                q.reply.closed().await
            }
            Self::Output(q, _) => {
                // WAIT: v2-hat-loop
                q.reply.closed().await
            }
            Self::Input(q, _) => {
                // WAIT: v2-hat-loop
                q.reply.closed().await
            }
        }
    }
    pub fn execution(
        &self,
        link: Transactions<Serial>,
        stamp: output::Stamp,
    ) -> impl std::future::Future<Output = Outcome> + Send + 'static {
        // Capture values only: the owner retains requests, replies and permits.
        let run = match self {
            Self::Output(q, r) => Run::Output(r.clone(), q.intent, q.include_colour, q.deadline),
            Self::Input(q, r) => Run::Input(r.clone(), q.deadline),
            Self::Discovery(q, r) => Run::Discovery(
                r.clone(),
                q.deadline
                    .min(Instant::now() + std::time::Duration::from_secs(8)),
            ),
        };
        async move {
            match run {
                Run::Discovery(route, deadline) => {
                    let value = discovery::execute(link, route, stamp, deadline).await;
                    Outcome {
                        sample: Sample::Discovery(value.observation),
                        error: value.error,
                        recovery: value.recovery,
                    }
                }
                Run::Output(route, intent, include_colour, deadline) => {
                    let value =
                        output::execute(link, route, stamp, intent, include_colour, deadline).await;
                    Outcome {
                        sample: Sample::Output(value.observation),
                        error: value.error,
                        recovery: value.recovery,
                    }
                }
                Run::Input(route, deadline) => {
                    let value = readback::execute(link, route, stamp, deadline).await;
                    Outcome {
                        sample: Sample::Input(value.observation),
                        error: value.error,
                        recovery: value.recovery,
                    }
                }
            }
        }
    }
    pub fn finish(self, sample: Option<Sample>, error: Option<ServiceError>) -> Completion {
        match (self, sample) {
            (Self::Discovery(request, _), sample) => Completion::Discovery(discovery::Completion {
                request,
                observation: sample.map(|s| match s {
                    Sample::Discovery(o) => o,
                    _ => unreachable!("physical result kind"),
                }),
                error,
            }),
            (Self::Output(request, _), sample) => Completion::Output(output::Completion {
                request,
                observation: sample.map(|s| match s {
                    Sample::Output(o) => o,
                    _ => unreachable!("physical result kind"),
                }),
                error,
            }),
            (Self::Input(request, _), sample) => Completion::Input(readback::Completion {
                request,
                observation: sample.map(|s| match s {
                    Sample::Input(o) => o,
                    _ => unreachable!("physical result kind"),
                }),
                error,
            }),
        }
    }
    pub fn reject(self, error: ServiceError) {
        self.finish(None, Some(error)).reject();
    }
}
enum Run {
    Discovery(discovery::Route, Instant),
    Output(output::Route, Option<output::WriteIntent>, bool, Instant),
    Input(readback::Route, Instant),
}
pub(super) enum Sample {
    Discovery(discovery::Observation),
    Output(output::Observation),
    Input(super::input::Observation),
}
impl Sample {
    pub fn invalidate(&mut self, reason: &str) {
        match self {
            Self::Output(value) => value.invalidate(reason),
            Self::Input(value) => value.stale = true,
            Self::Discovery(value) => value.stale = true,
        }
    }
}
pub(super) struct Outcome {
    pub sample: Sample,
    pub error: Option<ServiceError>,
    pub recovery: bool,
}
pub(super) enum Completion {
    Discovery(discovery::Completion),
    Output(output::Completion),
    Input(readback::Completion),
}
impl Completion {
    pub fn reject(self) {
        match self {
            Self::Discovery(q) => {
                let _ = q
                    .request
                    .reply
                    .send(Err(q.error.unwrap_or(ServiceError::Closed)));
            }
            Self::Output(q) => {
                let _ = q
                    .request
                    .reply
                    .send(Err(q.error.unwrap_or(ServiceError::Closed)));
            }
            Self::Input(q) => {
                let _ = q
                    .request
                    .reply
                    .send(Err(q.error.unwrap_or(ServiceError::Closed)));
            }
        }
    }
}
pub(super) struct Active {
    pub work: Work,
    pub stamp: output::Stamp,
    pub stage: Stage,
    pub outcome: Option<Outcome>,
    pub cleanup: Option<Instant>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
    Pending,
    Running,
    Settling,
    Closing,
}
