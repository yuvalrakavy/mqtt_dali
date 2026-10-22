//! One concrete owner for input reads, output operations and commissioning on the HAT.
use super::{
    commissioning, discovery, groups, input::readback, output, reassignment, restoration,
    scene_settings, scenes, ServiceError,
};
use crate::atx::{serial::Serial, transactions::Transactions};
use tokio::time::Instant;

pub(super) enum Work {
    Restore(Box<restoration::Request>, restoration::Route),
    Reassignment(reassignment::Request, reassignment::Route),
    SceneSettings(Box<scene_settings::Request>, scene_settings::Route),
    Scenes(scenes::Request, scenes::Route),
    Groups(groups::Request, groups::Route),
    Commission(commissioning::Request, commissioning::Route),
    Discovery(discovery::Request, discovery::Route),
    Output(output::Request, output::Route),
    Input(readback::Request, readback::Route),
}
impl Work {
    pub fn bus(&self) -> u8 {
        match self {
            Self::Restore(_, r) => r.bus,
            Self::Reassignment(_, r) => r.bus,
            Self::SceneSettings(_, r) => r.bus,
            Self::Scenes(_, r) => r.bus,
            Self::Groups(_, r) => r.bus,
            Self::Output(_, r) => r.bus,
            Self::Input(_, r) => r.bus,
            Self::Discovery(_, r) => r.bus,
            Self::Commission(_, r) => r.bus,
        }
    }
    pub fn required_stamp(&self) -> Option<output::Stamp> {
        match self {
            Self::Restore(_, r) => r.original_stamp,
            Self::Reassignment(_, r) => Some(r.original_stamp),
            Self::SceneSettings(
                _,
                scene_settings::Route {
                    plan: scene_settings::Plan::Program { original_stamp, .. },
                    ..
                },
            ) => Some(*original_stamp),
            Self::Output(q, _) => q.required_stamp,
            Self::Groups(
                _,
                groups::Route {
                    plan: groups::Plan::Program { original_stamp, .. },
                    ..
                },
            ) => Some(*original_stamp),
            Self::Commission(
                _,
                commissioning::Route {
                    plan: commissioning::Plan::Program(c),
                    ..
                },
            ) => Some(c.stamp),
            _ => None,
        }
    }
    pub fn deadline(&self) -> Instant {
        match self {
            Self::Restore(q, _) => q.deadline,
            Self::Reassignment(q, _) => q.deadline,
            Self::SceneSettings(q, _) => q.deadline,
            Self::Scenes(q, _) => q.deadline,
            Self::Groups(q, _) => q.deadline,
            Self::Output(q, _) => q.deadline,
            Self::Input(q, _) => q.deadline,
            Self::Discovery(q, _) => q.deadline,
            Self::Commission(q, _) => q.deadline,
        }
    }
    pub fn is_closed(&self) -> bool {
        match self {
            Self::Restore(q, _) => q.reply.is_closed(),
            Self::Reassignment(q, _) => q.reply.is_closed(),
            Self::SceneSettings(q, _) => q.reply.is_closed(),
            Self::Scenes(q, _) => q.reply.is_closed(),
            Self::Groups(q, _) => q.reply.is_closed(),
            Self::Output(q, _) => q.reply.is_closed(),
            Self::Input(q, _) => q.reply.is_closed(),
            Self::Discovery(q, _) => q.reply.is_closed(),
            Self::Commission(q, _) => q.reply.is_closed(),
        }
    }
    pub fn mutating(&self) -> bool {
        if matches!(self, Self::Restore(_, r) if matches!(r.plan,restoration::Plan::Unique | restoration::Plan::Clear))
        {
            return true;
        }
        if matches!(self, Self::Reassignment(..)) {
            return true;
        }
        matches!(self,Self::SceneSettings(_,r) if matches!(r.plan,scene_settings::Plan::Program { .. }))
            || matches!(self, Self::Scenes(q,_) if q.action.writing())
            || matches!(self, Self::Groups(_,r) if matches!(r.plan,groups::Plan::Program{..}))
            || matches!(self, Self::Output(q,_) if q.intent.is_some())
            || matches!(
                self,
                Self::Commission(
                    _,
                    commissioning::Route {
                        plan: commissioning::Plan::Program(_),
                        ..
                    }
                )
            )
    }
    pub fn span(&self) -> tracing::Span {
        match self {
            Self::Restore(q, r) => r.context.operation_span(&r.controller, q.action.name()),
            Self::Reassignment(_, r) => r
                .context
                .operation_span(&r.controller, "reset_address_space"),
            Self::SceneSettings(_, r) => r.context.operation_span(&r.controller, "scene_settings"),
            Self::Scenes(_, r) => r.context.operation_span(&r.controller, "scenes"),
            Self::Groups(_, r) => r.context.operation_span(&r.controller, "groups"),
            Self::Output(_, r) => r.context.operation_span(&r.controller, "output"),
            Self::Input(_, r) => r.context.operation_span(&r.controller, "input_read"),
            Self::Discovery(_, r) => r.context.operation_span(&r.controller, "discover_address"),
            Self::Commission(_, r) => r.context.operation_span(
                &r.controller,
                match r.space {
                    discovery::Space::Outputs => "assign_next_output",
                    discovery::Space::Inputs => "assign_next_input",
                },
            ),
        }
    }
    pub async fn wait_for_receiver_close(&mut self) {
        match self {
            Self::Restore(q, _) => {
                // WAIT: v2-hat-loop
                q.reply.closed().await
            }
            Self::Reassignment(q, _) => {
                // WAIT: v2-hat-loop
                q.reply.closed().await
            }
            Self::SceneSettings(q, _) => {
                // WAIT: v2-hat-loop
                q.reply.closed().await
            }
            Self::Scenes(q, _) => {
                // WAIT: v2-hat-loop
                q.reply.closed().await
            }
            Self::Groups(q, _) => {
                // WAIT: v2-hat-loop
                q.reply.closed().await
            }
            Self::Commission(q, _) => {
                // WAIT: v2-hat-loop
                q.reply.closed().await
            }
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
            Self::Restore(q, r) => Run::Restore(r.clone(), q.deadline),
            Self::Reassignment(q, r) => Run::Reassignment(r.clone(), q.deadline),
            Self::SceneSettings(q, r) => Run::SceneSettings(r.clone(), q.deadline),
            Self::Scenes(q, r) => Run::Scenes(r.clone(), q.deadline),
            Self::Groups(q, r) => Run::Groups(
                r.clone(),
                q.deadline.min(
                    Instant::now()
                        + std::time::Duration::from_secs(
                            if matches!(r.plan, groups::Plan::Probe) {
                                3
                            } else {
                                4
                            },
                        ),
                ),
            ),
            Self::Commission(q, r) => Run::Commission(
                r.clone(),
                q.deadline.min(
                    Instant::now()
                        + std::time::Duration::from_secs(
                            if matches!(r.plan, commissioning::Plan::Search(_)) {
                                10
                            } else {
                                4
                            },
                        ),
                ),
            ),
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
                Run::Restore(route, deadline) => {
                    let value = restoration::execute::execute(link, route, stamp, deadline).await;
                    Outcome {
                        sample: Sample::Restore(value.observation),
                        error: value.error,
                        recovery: value.recovery,
                    }
                }
                Run::Reassignment(route, deadline) => {
                    let value = reassignment::execute(link, route, stamp, deadline).await;
                    Outcome {
                        sample: Sample::Reassignment(value.observation),
                        error: value.error,
                        recovery: value.recovery,
                    }
                }
                Run::SceneSettings(route, deadline) => {
                    let value = scene_settings::execute::run(link, route, stamp, deadline).await;
                    Outcome {
                        sample: Sample::SceneSettings(value.observation),
                        error: value.error,
                        recovery: value.recovery,
                    }
                }
                Run::Scenes(route, deadline) => {
                    let value = scenes::execute(link, route, stamp, deadline).await;
                    Outcome {
                        sample: Sample::Scenes(value.observation),
                        error: value.error,
                        recovery: value.recovery,
                    }
                }
                Run::Groups(route, deadline) => {
                    let value = groups::execute(link, route, stamp, deadline).await;
                    Outcome {
                        sample: Sample::Groups(value.observation),
                        error: value.error,
                        recovery: value.recovery,
                    }
                }
                Run::Commission(route, deadline) => {
                    let value = commissioning::execute(link, route, stamp, deadline).await;
                    Outcome {
                        sample: Sample::Commission(value.observation),
                        error: value.error,
                        recovery: value.recovery,
                    }
                }
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
            (Self::Restore(request, _), sample) => Completion::Restore(restoration::Completion {
                request,
                observation: sample.map(|s| match s {
                    Sample::Restore(o) => o,
                    _ => unreachable!("physical result kind"),
                }),
                error,
            }),
            (Self::Reassignment(request, _), sample) => {
                Completion::Reassignment(reassignment::Completion {
                    request,
                    observation: sample.map(|s| match s {
                        Sample::Reassignment(o) => o,
                        _ => unreachable!("physical result kind"),
                    }),
                    error,
                })
            }
            (Self::SceneSettings(request, _), sample) => {
                Completion::SceneSettings(scene_settings::Completion {
                    request,
                    observation: sample.map(|s| match s {
                        Sample::SceneSettings(o) => o,
                        _ => unreachable!("physical result kind"),
                    }),
                    error,
                })
            }
            (Self::Scenes(request, _), sample) => Completion::Scenes(scenes::Completion {
                request,
                observation: sample.map(|s| match s {
                    Sample::Scenes(o) => o,
                    _ => unreachable!("physical result kind"),
                }),
                error,
            }),
            (Self::Groups(request, _), sample) => Completion::Groups(groups::Completion {
                request,
                observation: sample.map(|s| match s {
                    Sample::Groups(o) => o,
                    _ => unreachable!("physical result kind"),
                }),
                error,
            }),
            (Self::Commission(request, _), sample) => {
                Completion::Commission(commissioning::Completion {
                    request,
                    observation: sample.map(|s| match s {
                        Sample::Commission(o) => o,
                        _ => unreachable!("physical result kind"),
                    }),
                    error,
                })
            }
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
    Restore(restoration::Route, Instant),
    Reassignment(reassignment::Route, Instant),
    SceneSettings(scene_settings::Route, Instant),
    Scenes(scenes::Route, Instant),
    Groups(groups::Route, Instant),
    Commission(commissioning::Route, Instant),
    Discovery(discovery::Route, Instant),
    Output(output::Route, Option<output::WriteIntent>, bool, Instant),
    Input(readback::Route, Instant),
}
pub(super) enum Sample {
    Restore(restoration::Observation),
    Reassignment(reassignment::Observation),
    SceneSettings(scene_settings::Observation),
    Scenes(scenes::Observation),
    Groups(groups::Observation),
    Commission(commissioning::Observation),
    Discovery(discovery::Observation),
    Output(output::Observation),
    Input(super::input::Observation),
}
impl Sample {
    pub fn invalidate(&mut self, reason: &str) {
        match self {
            Self::Restore(value) => value.stale = true,
            Self::Reassignment(value) => value.stale = true,
            Self::SceneSettings(value) => value.stale = true,
            Self::Scenes(value) => value.stale = true,
            Self::Groups(value) => {
                if let Some(p) = &mut value.probe {
                    p.stale = true;
                }
            }
            Self::Output(value) => value.invalidate(reason),
            Self::Input(value) => value.stale = true,
            Self::Discovery(value) => value.stale = true,
            Self::Commission(value) => value.stale = true,
        }
    }
}
pub(super) struct Outcome {
    pub sample: Sample,
    pub error: Option<ServiceError>,
    pub recovery: bool,
}
pub(super) enum Completion {
    Restore(restoration::Completion),
    Reassignment(reassignment::Completion),
    SceneSettings(scene_settings::Completion),
    Scenes(scenes::Completion),
    Groups(groups::Completion),
    Commission(commissioning::Completion),
    Discovery(discovery::Completion),
    Output(output::Completion),
    Input(readback::Completion),
}
impl Completion {
    pub fn reject(self) {
        match self {
            Self::Restore(q) => {
                restoration::finish(q.request, Err(q.error.unwrap_or(ServiceError::Closed)))
            }
            Self::Reassignment(q) => {
                reassignment::finish(q.request, Err(q.error.unwrap_or(ServiceError::Closed)))
            }
            Self::SceneSettings(q) => {
                scene_settings::finish(q.request, Err(q.error.unwrap_or(ServiceError::Closed)))
            }
            Self::Scenes(q) => {
                scenes::finish(q.request, Err(q.error.unwrap_or(ServiceError::Closed)))
            }
            Self::Groups(q) => {
                groups::finish(q.request, Err(q.error.unwrap_or(ServiceError::Closed)))
            }
            Self::Commission(q) => {
                commissioning::finish(q.request, Err(q.error.unwrap_or(ServiceError::Closed)))
            }
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
