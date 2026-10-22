use super::{
    auth::{Access, Refusal, COOKIE},
    server::State,
};
use crate::service::{Hardware, Phase, PreparationError, PreparedCommand, Snapshot};
use axum::{
    extract::{DefaultBodyLimit, Path, Request, State as AxumState},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{
        sse::{Event, Sse},
        IntoResponse, Redirect, Response,
    },
    routing::{get, post},
    Json, Router,
};
use futures_util::{stream, Stream};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{convert::Infallible, sync::Arc, time::Duration};
use subtle::ConstantTimeEq;
use tokio::{
    sync::{watch, OwnedSemaphorePermit},
    time::{timeout, Instant},
};
use tracing::Instrument;

#[derive(Clone)]
struct Session {
    token: String,
    access: Access,
}
pub(super) fn router(state: Arc<State>) -> Router {
    let protected = Router::new()
        .route("/api/session", get(session))
        .route("/api/session/activity", post(activity))
        .route("/api/logout", post(logout))
        .route("/api/snapshot", get(snapshot))
        .route("/api/events", get(events))
        .route("/api/jobs", get(jobs).post(command))
        .route("/api/jobs/{id}", get(job))
        .fallback(missing)
        .layer(middleware::from_fn_with_state(state.clone(), protect));
    Router::new()
        .route("/api/identity", get(identity))
        .route("/login", get(identity))
        .route("/api/login", post(login))
        .merge(protected)
        .layer(DefaultBodyLimit::max(8192))
        .layer(middleware::from_fn_with_state(state.clone(), limits))
        .with_state(state)
}
fn error(status: StatusCode, message: &'static str) -> Response {
    (status, Json(json!({"error":message}))).into_response()
}
fn origin(headers: &HeaderMap, state: &State) -> bool {
    let values: Vec<_> = headers.get_all(header::ORIGIN).iter().collect();
    values.len() == 1 && values[0].as_bytes() == state.settings.origin.as_bytes()
}
fn cookie(headers: &HeaderMap) -> Option<String> {
    let mut tokens = Vec::new();
    for header in headers.get_all(header::COOKIE) {
        for part in header.to_str().ok()?.split(';') {
            let Some((name, value)) = part.trim().split_once('=') else {
                continue;
            };
            if name == COOKIE {
                tokens.push(value);
            }
        }
    }
    (tokens.len() == 1
        && tokens[0].len() == 64
        && tokens[0]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
    .then(|| tokens[0].to_owned())
}
fn access(state: &State, token: &str) -> Option<Access> {
    // WAIT: web-auth
    state
        .auth
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .access(token, Instant::now())
}
fn set_cookie(state: &State, value: &str, clear: bool) -> HeaderValue {
    let mut value = format!("{COOKIE}={value}; Path=/; HttpOnly; SameSite=Strict");
    if state.settings.secure {
        value.push_str("; Secure");
    }
    if clear {
        value.push_str("; Max-Age=0");
    }
    value.parse().expect("bounded generated cookie")
}
async fn limits(AxumState(state): AxumState<Arc<State>>, request: Request, next: Next) -> Response {
    let mut response = match state.handlers.clone().try_acquire_owned() {
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE, "web access is busy"),
        Ok(_permit) => {
            // WAIT: web-handler
            timeout(Duration::from_secs(5), next.run(request))
                .await
                .unwrap_or_else(|_| error(StatusCode::REQUEST_TIMEOUT, "request expired"))
        }
    };
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    headers.insert("content-security-policy",HeaderValue::from_static("default-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'"));
    response
}
async fn protect(
    AxumState(state): AxumState<Arc<State>>,
    mut request: Request,
    next: Next,
) -> Response {
    let authenticated = cookie(request.headers())
        .and_then(|token| access(&state, &token).map(|access| Session { token, access }));
    let Some(session) = authenticated else {
        return if request.uri().path().starts_with("/api/") {
            error(StatusCode::UNAUTHORIZED, "sign in required")
        } else {
            Redirect::to("/login").into_response()
        };
    };
    if !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) {
        let tokens: Vec<_> = request.headers().get_all("x-csrf-token").iter().collect();
        if !origin(request.headers(), &state)
            || tokens.len() != 1
            || !bool::from(session.access.csrf.as_bytes().ct_eq(tokens[0].as_bytes()))
        {
            return error(
                StatusCode::FORBIDDEN,
                "request origin or CSRF token is invalid",
            );
        }
    }
    let trace = if request.headers().get_all("traceparent").iter().count() == 1 {
        request
            .headers()
            .get("traceparent")
            .and_then(|v| v.to_str().ok())
    } else {
        None
    };
    let context = mqtt_bridge_kit::RequestContext::default().with_traceparent(trace);
    let name = state
        .handle
        .snapshot()
        .map(|s| s.model.inventory().name.clone())
        .unwrap_or_default();
    let span = context.operation_span(&name, "http_request");
    request.extensions_mut().insert(context);
    request.extensions_mut().insert(session);
    // WAIT: web-handler
    next.run(request).instrument(span).await
}
fn identity_value(snapshot: &Snapshot) -> Value {
    json!({"controller":snapshot.model.inventory().name,"version":snapshot.version,"build_utc":snapshot.build_utc})
}
async fn identity(AxumState(state): AxumState<Arc<State>>) -> Response {
    state
        .handle
        .snapshot()
        .map(|s| Json(identity_value(&s)).into_response())
        .unwrap_or_else(|| error(StatusCode::SERVICE_UNAVAILABLE, "service is starting"))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Login {
    password: String,
}
async fn login(
    AxumState(state): AxumState<Arc<State>>,
    headers: HeaderMap,
    body: Result<Json<Login>, axum::extract::rejection::JsonRejection>,
) -> Response {
    if !origin(&headers, &state) {
        return error(StatusCode::FORBIDDEN, "request origin is invalid");
    }
    let Ok(Json(body)) = body else {
        return error(StatusCode::BAD_REQUEST, "invalid login JSON");
    };
    // WAIT: web-auth
    let result = state.auth.lock().unwrap_or_else(|p| p.into_inner()).login(
        &body.password,
        cookie(&headers).as_deref(),
        Instant::now(),
    );
    match result {
        Ok((token, access)) => {
            state
                .revoked
                .send_modify(|value| *value = value.wrapping_add(1));
            let mut response = Json(json!({"csrf":access.csrf})).into_response();
            response
                .headers_mut()
                .insert(header::SET_COOKIE, set_cookie(&state, &token, false));
            response
        }
        Err(Refusal::Password) => error(StatusCode::UNAUTHORIZED, "invalid password"),
        Err(Refusal::Capacity) => error(StatusCode::TOO_MANY_REQUESTS, "too many active sessions"),
        Err(Refusal::Rate) => {
            let mut response = error(StatusCode::TOO_MANY_REQUESTS, "too many login attempts");
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from_static("60"));
            response
        }
    }
}
async fn session(axum::Extension(session): axum::Extension<Session>) -> Json<Value> {
    let now = Instant::now();
    Json(
        json!({"csrf":session.access.csrf,"idle_remaining_ms":session.access.expires.saturating_duration_since(now).as_millis(),"absolute_remaining_ms":session.access.absolute.saturating_duration_since(now).as_millis()}),
    )
}
async fn activity(
    AxumState(state): AxumState<Arc<State>>,
    axum::Extension(session): axum::Extension<Session>,
) -> Response {
    // WAIT: web-auth
    if state
        .auth
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .activity(&session.token, Instant::now())
    {
        StatusCode::NO_CONTENT.into_response()
    } else {
        error(StatusCode::UNAUTHORIZED, "sign in required")
    }
}
async fn logout(
    AxumState(state): AxumState<Arc<State>>,
    axum::Extension(session): axum::Extension<Session>,
) -> Response {
    // WAIT: web-auth
    state
        .auth
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .logout(&session.token);
    state
        .revoked
        .send_modify(|value| *value = value.wrapping_add(1));
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, set_cookie(&state, "", true));
    response
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    generation: String,
    target: Option<String>,
    command: String,
    params: mqtt_bridge_kit::PropertyMap,
}
async fn command(
    AxumState(state): AxumState<Arc<State>>,
    axum::Extension(session): axum::Extension<Session>,
    axum::Extension(cause): axum::Extension<mqtt_bridge_kit::RequestContext>,
    body: Result<Json<Command>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(Json(body)) = body else {
        return error(StatusCode::BAD_REQUEST, "invalid command JSON");
    };
    // Body extraction may have waited while this session expired or was revoked.
    // There is no await from this recheck through preparation and job admission.
    // WAIT: web-auth
    if !state
        .auth
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .activity(&session.token, Instant::now())
    {
        return error(StatusCode::UNAUTHORIZED, "sign in required");
    }
    let Ok(context) = mqtt_bridge_kit::RequestContext::new(body.generation) else {
        return error(StatusCode::BAD_REQUEST, "invalid command generation");
    };
    let Some(snapshot) = state.handle.snapshot() else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "service is starting");
    };
    let call = mqtt_bridge_kit::CommandCall {
        command: body.command.clone(),
        params: body.params,
    };
    let prepared = match PreparedCommand::new(
        &snapshot,
        body.target.clone(),
        &call,
        context.with_traceparent(cause.traceparent()),
    ) {
        Ok(prepared) => prepared,
        Err(PreparationError::Invalid) => {
            return error(StatusCode::BAD_REQUEST, "invalid command or parameters")
        }
        Err(PreparationError::Conflict) => {
            return error(
                StatusCode::CONFLICT,
                "configuration or selection changed; refresh before retrying",
            )
        }
    };
    if state.stop.is_cancelled() {
        return error(StatusCode::SERVICE_UNAVAILABLE, "web access is stopping");
    }
    let Some(record) = state.jobs.admit(body.command, body.target, prepared) else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "too many active commands");
    };
    let location = format!("/api/jobs/{}", record.id);
    (
        StatusCode::ACCEPTED,
        [(header::LOCATION, location)],
        Json(record),
    )
        .into_response()
}
async fn jobs(AxumState(state): AxumState<Arc<State>>) -> Json<Value> {
    Json(json!({"jobs":state.jobs.list()}))
}
async fn job(AxumState(state): AxumState<Arc<State>>, Path(id): Path<String>) -> Response {
    match state.jobs.get(&id) {
        Some(record) => Json(record).into_response(),
        None => error(StatusCode::NOT_FOUND, "job not found"),
    }
}
fn snapshot_value(snapshot: &Snapshot) -> Value {
    let phase = match snapshot.phase {
        Phase::AwaitingRuntime => "awaiting_runtime",
        Phase::Ready => "ready",
        Phase::Writing => "writing",
        Phase::Uncertain(_) => "uncertain",
        Phase::Stopping => "stopping",
        Phase::Closed => "closed",
    };
    let hardware = match &snapshot.hardware {
        Hardware::NotStarted => json!({"state":"disabled"}),
        Hardware::Closed => json!({"state":"closed"}),
        Hardware::Connecting => json!({"state":"connecting"}),
        Hardware::Initializing => json!({"state":"initializing"}),
        Hardware::Resynchronizing => json!({"state":"resynchronizing"}),
        Hardware::ResetRequired => json!({"state":"reset_required"}),
        Hardware::WaitingLateReply { remaining } => {
            json!({"state":"waiting_late_reply","remaining":remaining})
        }
        Hardware::Unavailable { retrying, .. } => {
            json!({"state":"unavailable","retrying":retrying})
        }
        Hardware::Ready { buses, .. } => {
            json!({"state":"ready","buses":buses.iter().map(|b|json!({"bus":b.bus,"status":b.status,"channels":b.channels})).collect::<Vec<_>>()})
        }
    };
    json!({"identity":identity_value(snapshot),"phase":phase,"hardware":hardware,"inventory":serde_json::from_slice::<Value>(&snapshot.model.state_bytes().expect("validated model")).expect("validated JSON")})
}
fn encoded_snapshot(state: &State) -> Option<String> {
    let data = serde_json::to_string(&snapshot_value(&state.handle.snapshot()?)).ok()?;
    (data.len() <= 1024 * 1024).then_some(data)
}
async fn snapshot(AxumState(state): AxumState<Arc<State>>) -> Response {
    match encoded_snapshot(&state) {
        Some(data) => ([(header::CONTENT_TYPE, "application/json")], data).into_response(),
        None => error(StatusCode::SERVICE_UNAVAILABLE, "snapshot is unavailable"),
    }
}
struct StreamState {
    state: Arc<State>,
    token: String,
    updates: tokio::sync::broadcast::Receiver<Snapshot>,
    revoked: watch::Receiver<u64>,
    initial: bool,
    _permit: OwnedSemaphorePermit,
}
async fn events(
    AxumState(state): AxumState<Arc<State>>,
    axum::Extension(session): axum::Extension<Session>,
) -> Response {
    let Ok(permit) = state.streams.clone().try_acquire_owned() else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "too many live streams");
    };
    let stream = stream::unfold(
        StreamState {
            updates: state.handle.subscribe(),
            revoked: state.revoked.subscribe(),
            state,
            token: session.token,
            initial: true,
            _permit: permit,
        },
        |mut stream| async move {
            loop {
                let allowed = access(&stream.state, &stream.token)?;
                if stream.state.stop.is_cancelled() {
                    return None;
                }
                if stream.initial {
                    stream.initial = false;
                    let data = encoded_snapshot(&stream.state)?;
                    return Some((
                        Ok::<_, Infallible>(Event::default().event("snapshot").data(data)),
                        stream,
                    ));
                }
                // WAIT: web-events
                tokio::select! {
                    biased;
                    _=stream.state.stop.cancelled()=>return None,
                    result=stream.revoked.changed()=>if result.is_err() { return None; },
                    _=tokio::time::sleep_until(allowed.expires)=>{},
                    result=stream.updates.recv()=>match result {
                        Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_))=>stream.initial=true,
                        Err(tokio::sync::broadcast::error::RecvError::Closed)=>return None,
                    }
                }
            }
        },
    );
    sse(stream)
}
fn sse(stream: impl Stream<Item = Result<Event, Infallible>> + Send + 'static) -> Response {
    Sse::new(stream).into_response()
}
async fn missing() -> Response {
    error(StatusCode::NOT_FOUND, "route not found")
}
