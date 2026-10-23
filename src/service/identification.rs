//! Managed, transient output identification. Public intent never refreshes authority.
use super::*;
use crate::{
    dali::gear::{Intent, Level, LevelValue, QueryReply, QueryValue},
    inventory::{Location, Origin},
};
use std::{
    future::{pending, Future},
    pin::Pin,
    time::Duration,
};

const DWELL: Duration = Duration::from_millis(750);
const SESSION: Duration = Duration::from_secs(120);
const STEP: Duration = Duration::from_secs(2);
const COLOR_STEP: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Starting,
    Blinking,
    Restoring,
    Restored,
    Interrupted,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Status {
    pub target: String,
    pub generation: String,
    pub phase: Stage,
    pub actual_level: Option<u8>,
}
impl Status {
    fn active(&self) -> bool {
        matches!(
            self.phase,
            Stage::Starting | Stage::Blinking | Stage::Restoring
        )
    }
}
#[derive(Clone)]
pub(super) struct Claim {
    pub status: Status,
    origin: Origin,
    location: Location,
}
pub(super) struct Request {
    target: String,
    stop: bool,
    context: RequestContext,
    deadline: Instant,
    pub reply: oneshot::Sender<Result<Status, ServiceError>>,
}
pub(super) fn active(handle: &ServiceHandle) -> bool {
    handle
        .identification
        .borrow()
        .as_ref()
        .is_some_and(|c| c.status.active())
}
pub(super) fn blocks(handle: &ServiceHandle, target: &str) -> bool {
    let Some(snapshot) = handle.snapshot() else {
        return false;
    };
    let claim = handle.identification.borrow();
    let Some(claim) = claim.as_ref().filter(|c| c.status.active()) else {
        return false;
    };
    snapshot.model.projections().iter().any(|p| {
        p.target == target
            && p.location == claim.location
            && snapshot.model.origin(p.location) == Some(&claim.origin)
    })
}
pub(super) fn blocks_bus(handle: &ServiceHandle, bus: u8) -> bool {
    let Some(snapshot) = handle.snapshot() else {
        return false;
    };
    handle
        .identification
        .borrow()
        .as_ref()
        .is_some_and(|claim| {
            claim.status.active()
                && matches!(claim.location,Location::Output{bus:index,..} if index==bus)
                && snapshot.model.origin(claim.location) == Some(&claim.origin)
        })
}
pub(super) fn admit(
    snapshot: &Snapshot,
    target: &str,
    context: &RequestContext,
) -> Result<(), ServiceError> {
    output::resolve(snapshot, target, context, true).map(|_| ())
}
impl ServiceHandle {
    /// Start or stop one controller-owned Blink session. Stop confirms actual restore.
    pub async fn identify_output(
        &self,
        target: &str,
        stop: bool,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<Status, ServiceError> {
        if Instant::now() >= deadline {
            return Err(ServiceError::Expired);
        }
        if self.permit.is_closed() {
            return Err(ServiceError::Closed);
        }
        admit(
            &self.snapshot().ok_or(ServiceError::Closed)?,
            target,
            &context,
        )?;
        let (reply, result) = oneshot::channel();
        self.send
            .try_send(Message::Identify(Request {
                target: target.into(),
                stop,
                context,
                deadline,
                reply,
            }))
            .map_err(|_| ServiceError::Busy)?;
        // WAIT: v2-identification-request
        let result = timeout_at(deadline, result).await;
        if Instant::now() >= deadline {
            return Err(ServiceError::Uncertain(
                "identification caller deadline".into(),
            ));
        }
        result
            .map_err(|_| ServiceError::Uncertain("identification caller deadline".into()))?
            .map_err(|_| ServiceError::Closed)?
    }
    async fn identification_output(
        &self,
        target: &str,
        intent: Option<output::WriteIntent>,
        stamp: Option<output::Stamp>,
        context: RequestContext,
        deadline: Instant,
    ) -> Result<output::Observation, ServiceError> {
        // Fair acquisition lets an already-waiting rename precede the next pulse.
        // WAIT: v2-identification-permit
        let permit = timeout_at(deadline, self.permit.clone().acquire_owned())
            .await
            .map_err(|_| ServiceError::Expired)?
            .map_err(|_| ServiceError::Closed)?;
        if Instant::now() >= deadline {
            return Err(ServiceError::Expired);
        }
        output::resolve(
            &self.snapshot().ok_or(ServiceError::Closed)?,
            target,
            &context,
            true,
        )?;
        let (reply, result) = oneshot::channel();
        self.send
            .try_send(Message::Output(output::Request {
                target: target.into(),
                intent,
                include_colour: intent.is_none()
                    || matches!(intent, Some(output::WriteIntent::Combined(_, _))),
                identification: true,
                required_stamp: stamp,
                context,
                deadline,
                reply,
                _permit: permit,
            }))
            .map_err(|_| ServiceError::Closed)?;
        // WAIT: v2-identification-request
        let result = timeout_at(deadline, result).await;
        if Instant::now() >= deadline {
            return Err(ServiceError::Uncertain(
                "identification physical deadline".into(),
            ));
        }
        result
            .map_err(|_| ServiceError::Uncertain("identification physical deadline".into()))?
            .map_err(|_| ServiceError::Closed)?
    }
}

type OwnedFuture = Pin<Box<dyn Future<Output = Result<Status, ServiceError>> + Send>>;
pub(super) struct Manager {
    handle: ServiceHandle,
    state: watch::Sender<Option<Claim>>,
    changes: watch::Receiver<Option<Claim>>,
    future: Option<OwnedFuture>,
    stop: Option<watch::Sender<Option<Instant>>>,
    reply: Option<Request>,
}
impl Manager {
    pub fn new(handle: ServiceHandle, state: watch::Sender<Option<Claim>>) -> Self {
        Self {
            handle,
            changes: state.subscribe(),
            state,
            future: None,
            stop: None,
            reply: None,
        }
    }
    pub fn status(&self) -> Option<Status> {
        self.state.borrow().as_ref().map(|c| c.status.clone())
    }
    pub fn command(&mut self, request: Request) {
        if request.reply.is_closed() || Instant::now() >= request.deadline {
            let _ = request.reply.send(Err(ServiceError::Expired));
            return;
        }
        let snapshot = match self.handle.snapshot() {
            Some(s) => s,
            None => {
                let _ = request.reply.send(Err(ServiceError::Closed));
                return;
            }
        };
        if let Err(e) = admit(&snapshot, &request.target, &request.context) {
            let _ = request.reply.send(Err(e));
            return;
        }
        if request.stop {
            if self.future.is_none() {
                let _ = request.reply.send(Ok(Status {
                    target: request.target,
                    generation: request.context.generation().unwrap().into(),
                    phase: Stage::Restored,
                    actual_level: None,
                }));
            } else if self.status().is_some_and(|s| s.target == request.target)
                && self.reply.is_none()
            {
                self.stop
                    .as_ref()
                    .unwrap()
                    .send_replace(Some(request.deadline));
                self.reply = Some(request);
            } else {
                let _ = request.reply.send(Err(ServiceError::Busy));
            }
            return;
        }
        if self.future.is_some() {
            let _ = request.reply.send(Err(ServiceError::Busy));
            return;
        }
        let location = snapshot
            .model
            .projections()
            .into_iter()
            .find(|p| p.target == request.target)
            .unwrap()
            .location;
        let claim = Claim {
            status: Status {
                target: request.target.clone(),
                generation: request.context.generation().unwrap().into(),
                phase: Stage::Starting,
                actual_level: None,
            },
            origin: snapshot.model.origin(location).unwrap().clone(),
            location,
        };
        self.state.send_replace(Some(claim.clone()));
        let (stop, receive) = watch::channel(None);
        self.stop = Some(stop);
        self.future = Some(Box::pin(run(
            self.handle.clone(),
            self.state.clone(),
            claim,
            request,
            receive,
        )));
    }
    pub async fn changed(&mut self) {
        let Some(future) = self.future.as_mut() else {
            // WAIT: v2-identification-owner
            pending::<()>().await;
            return;
        };
        // WAIT: v2-identification-owner
        let result =
            tokio::select! {biased;result=future=>Some(result),_=self.changes.changed()=>None};
        if let Some(result) = result {
            self.future = None;
            self.stop = None;
            if let Some(request) = self.reply.take() {
                let result = if Instant::now() >= request.deadline {
                    Err(ServiceError::Expired)
                } else {
                    result
                };
                let _ = request.reply.send(result);
            }
        }
    }
}
impl Drop for Manager {
    fn drop(&mut self) {
        self.future = None;
        let claim = self.state.borrow().clone().filter(|c| c.status.active());
        if let Some(mut claim) = claim {
            claim.status.phase = Stage::Interrupted;
            self.state.send_replace(Some(claim));
        }
    }
}

fn continuing(
    handle: &ServiceHandle,
    claim: &Claim,
    cause: &RequestContext,
) -> Result<(String, RequestContext), ServiceError> {
    let snapshot = handle.snapshot().ok_or(ServiceError::Closed)?;
    if snapshot.model.origin(claim.location) != Some(&claim.origin) {
        return Err(ServiceError::Rejected(
            "identified fixture was removed or replaced".into(),
        ));
    }
    let projection = snapshot
        .model
        .projections()
        .into_iter()
        .find(|p| p.location == claim.location)
        .ok_or(ServiceError::Closed)?;
    let context = RequestContext::new(
        routing::generations(&snapshot.model).targets[&projection.target].clone(),
    )
    .map_err(ServiceError::Rejected)?
    .with_traceparent(cause.traceparent());
    admit(&snapshot, &projection.target, &context)?;
    Ok((projection.target, context))
}
fn publish(
    state: &watch::Sender<Option<Claim>>,
    claim: &mut Claim,
    target: String,
    context: &RequestContext,
    phase: Stage,
    level: Option<Level>,
) -> Status {
    claim.status = Status {
        target,
        generation: context.generation().unwrap().into(),
        phase,
        actual_level: level.map(Level::raw),
    };
    state.send_replace(Some(claim.clone()));
    claim.status.clone()
}
async fn run(
    handle: ServiceHandle,
    state: watch::Sender<Option<Claim>>,
    mut claim: Claim,
    request: Request,
    mut stop: watch::Receiver<Option<Instant>>,
) -> Result<Status, ServiceError> {
    let Request {
        context,
        deadline,
        reply,
        ..
    } = request;
    let mut reply = Some(reply);
    let mut captured = None;
    let mut captured_colour = None;
    let ends = Instant::now() + SESSION;
    let operation:Result<(),ServiceError>=async {
        let (target,cause)=continuing(&handle,&claim,&context)?;
        let sample=handle.identification_output(&target,None,None,cause,deadline.min(Instant::now()+COLOR_STEP)).await?;
        proxy::accept_output(&handle.snapshot().ok_or(ServiceError::Closed)?,&target,&sample)?;
        let Some(LevelValue::Known(level))=sample.level() else {return Err(ServiceError::Rejected("identification needs a known actual level".into()))};
        if sample.quality!=output::Quality::Current || !matches!(sample.status,Some(QueryReply::Value(QueryValue::Status(s))) if !s.fade_running()) {
            return Err(ServiceError::Rejected("identification needs stable current output evidence".into()))
        }
        captured_colour=sample.colour.as_ref()
            .ok_or_else(||ServiceError::Rejected("Blink needs confirmed color capabilities".into()))?
            .restoration().map_err(ServiceError::Rejected)?;
        captured=Some((level,sample.stamp));
        let mut on=true;
        let mut changes=handle.state.clone();
        loop {
            let (target,cause)=continuing(&handle,&claim,&context)?;
            if stop.borrow().is_some() || Instant::now()>=ends {break}
            if reply.as_ref().is_some_and(|r|r.is_closed()) {break}
            // Rename during initial capture also ends identification before its first write.
            if cause.generation()!=context.generation() {break}
            let until=if reply.is_some(){deadline}else{ends}.min(Instant::now()+STEP);
            let observed=handle.identification_output(&target,Some(output::WriteIntent::Base(Intent::new(Some(on),None).unwrap())),Some(sample.stamp),cause.clone(),until).await?;
            proxy::accept_output(&handle.snapshot().ok_or(ServiceError::Closed)?,&target,&observed)?;
            let status=publish(&state,&mut claim,target,&cause,Stage::Blinking,observed.level().and_then(|v|if let LevelValue::Known(l)=v {Some(l)}else{None}));
            if let Some(reply)=reply.take() {if reply.send(Ok(status)).is_err() {break}}
            let wake=ends.min(Instant::now()+DWELL);
            loop {
                // WAIT: v2-identification-dwell
                tokio::select! {biased;
                    _=stop.changed()=>break,
                    _=tokio::time::sleep_until(wake)=>break,
                    _=changes.changed()=>{let (_,current)=continuing(&handle,&claim,&context)?;if current.generation()!=context.generation(){break}},
                }
            }
            on = !on;
        }
        Ok(())
    }.await;
    // A queued pulse may lose its token to the rename that deliberately ends it.
    let operation = if operation.is_err()
        && continuing(&handle, &claim, &context)
            .is_ok_and(|(_, c)| c.generation() != context.generation())
    {
        Ok(())
    } else {
        operation
    };
    // One restoration attempt, with the original Origin and physical epoch.
    let result = if let Some((level, stamp)) = captured {
        match continuing(&handle, &claim, &context) {
            Ok((target, cause)) => {
                publish(
                    &state,
                    &mut claim,
                    target.clone(),
                    &cause,
                    Stage::Restoring,
                    None,
                );
                let until = stop
                    .borrow()
                    .unwrap_or(Instant::now() + COLOR_STEP)
                    .min(Instant::now() + COLOR_STEP);
                let intent = if level.raw() == 0 {
                    Intent::new(Some(false), None)
                } else {
                    Intent::new(None, Some(level.normalized()))
                }
                .unwrap();
                let intent = captured_colour.map_or(output::WriteIntent::Base(intent), |colour| {
                    output::WriteIntent::Combined(intent, colour)
                });
                handle
                    .identification_output(&target, Some(intent), Some(stamp), cause.clone(), until)
                    .await
                    .and_then(|sample| {
                        proxy::accept_output(
                            &handle.snapshot().ok_or(ServiceError::Closed)?,
                            &target,
                            &sample,
                        )?;
                        if let Some(original) = captured_colour {
                            let restored = sample
                                .colour
                                .as_ref()
                                .and_then(|colour| colour.restoration().ok())
                                .flatten();
                            if restored != Some(original) {
                                return Err(ServiceError::Uncertain(
                                    "original color restoration was not confirmed".into(),
                                ));
                            }
                        }
                        let actual = match sample.level() {
                            Some(LevelValue::Known(l)) => Some(l),
                            _ => None,
                        };
                        let status =
                            publish(&state, &mut claim, target, &cause, Stage::Restored, actual);
                        operation.map(|_| status)
                    })
            }
            Err(e) => Err(e),
        }
    } else {
        operation.and_then(|_| {
            Err(ServiceError::Rejected(
                "identification has no captured level".into(),
            ))
        })
    };
    if result.is_err() {
        claim.status.phase = Stage::Interrupted;
        state.send_replace(Some(claim));
    }
    if let Some(reply) = reply {
        let response = match &result {
            Ok(_) => Err(ServiceError::Rejected(
                "identification ended before starting".into(),
            )),
            Err(e) => Err(e.clone()),
        };
        let _ = reply.send(response);
    }
    result
}

#[cfg(test)]
mod tests;
