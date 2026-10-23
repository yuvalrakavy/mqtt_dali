//! Local-only emulator browser. No bridge services, MQTT or browser file paths.
use super::session::{Control, Emulator};
use axum::{
    extract::{DefaultBodyLimit, Request, State},
    http::{header, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{net::TcpListener, sync::Semaphore, task::JoinHandle};
use tokio_util::sync::CancellationToken;
#[cfg(test)]
#[path = "web_tests.rs"]
mod tests;
#[path = "web_transport.rs"]
mod transport;
struct Context {
    emulator: Emulator,
    authority: String,
    token: String,
    slots: Semaphore,
}
pub struct Server {
    pub address: SocketAddr,
    stop: CancellationToken,
    task: JoinHandle<std::io::Result<()>>,
    connections: Arc<Semaphore>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.cancel();
        self.task.abort();
    }
}
impl Server {
    pub async fn bind(address: SocketAddr, emulator: Emulator) -> Result<Self, String> {
        if !address.ip().is_loopback() {
            return Err(
                "emulator web listener requires loopback; remote access is not supported".into(),
            );
        }
        // WAIT: emulator-web-serve
        let listener = TcpListener::bind(address)
            .await
            .map_err(|e| e.to_string())?;
        let address = listener.local_addr().map_err(|e| e.to_string())?;
        let token = rand::random::<[u8; 32]>()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let context = Arc::new(Context {
            emulator,
            authority: address.to_string(),
            token,
            slots: Semaphore::new(16),
        });
        let router = Router::new()
            .route("/", get(index))
            .route("/favicon.ico", get(|| async { StatusCode::NO_CONTENT }))
            .route("/assets/app.js", get(js))
            .route("/assets/app.css", get(css))
            .route("/api/session", get(session))
            .route("/api/snapshot", get(snapshot))
            .route("/api/scenario", get(scenario))
            .route("/api/control", post(control))
            .route("/api/examples", get(examples))
            .layer(DefaultBodyLimit::max(4 * 1024 * 1024))
            .layer(middleware::from_fn_with_state(context.clone(), protect))
            .with_state(context);
        let stop = CancellationToken::new();
        let connections = Arc::new(Semaphore::new(transport::CONNECTIONS as usize));
        let owned = transport::Listener {
            listener,
            slots: connections.clone(),
            force: stop.clone(),
        };
        let stopping = stop.clone();
        let task = tokio::spawn(async move {
            // WAIT: emulator-web-serve
            axum::serve(owned, router)
                .with_graceful_shutdown(stopping.cancelled_owned())
                .await
        });
        Ok(Self {
            address,
            stop,
            task,
            connections,
        })
    }
    pub async fn close(&mut self) -> Result<(), String> {
        self.stop.cancel();
        // WAIT: emulator-web-close
        tokio::time::timeout(Duration::from_secs(1), &mut self.task)
            .await
            .map_err(|_| "emulator HTTP shutdown deadline")?
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        // WAIT: emulator-web-close
        let _ = tokio::time::timeout(
            Duration::from_secs(1),
            self.connections
                .clone()
                .acquire_many_owned(transport::CONNECTIONS),
        )
        .await
        .map_err(|_| "emulator HTTP connections did not close")?;
        Ok(())
    }
}
fn fail(status: StatusCode, error: impl Into<String>) -> Response {
    (status, Json(serde_json::json!({"error":error.into()}))).into_response()
}
async fn protect(State(context): State<Arc<Context>>, request: Request, next: Next) -> Response {
    let headers = request.headers();
    let hosts: Vec<_> = headers.get_all(header::HOST).iter().collect();
    let expected_origin = format!("http://{}", context.authority);
    let valid_host = hosts.len() == 1 && hosts[0].as_bytes() == context.authority.as_bytes();
    let origins: Vec<_> = headers.get_all(header::ORIGIN).iter().collect();
    let cross_origin = !origins.is_empty()
        && (origins.len() != 1 || origins[0].as_bytes() != expected_origin.as_bytes());
    let tokens: Vec<_> = headers.get_all("x-emulator-csrf").iter().collect();
    let mutation = request.method() != Method::GET && request.method() != Method::HEAD;
    let csrf = tokens.len() == 1 && tokens[0].as_bytes() == context.token.as_bytes();
    let mut response = if !valid_host || cross_origin || (mutation && (origins.len() != 1 || !csrf))
    {
        fail(
            StatusCode::FORBIDDEN,
            "invalid emulator origin, host or CSRF token",
        )
    } else if let Ok(_permit) = context.slots.try_acquire() {
        // WAIT: emulator-web-handler
        tokio::time::timeout(Duration::from_secs(2), next.run(request))
            .await
            .unwrap_or_else(|_| fail(StatusCode::REQUEST_TIMEOUT, "request deadline"))
    } else {
        fail(StatusCode::SERVICE_UNAVAILABLE, "emulator web is busy")
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
async fn session(State(c): State<Arc<Context>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({"csrf":c.token}))
}
async fn execute(c: &Context, control: Control) -> Response {
    match c.emulator.control(control, Duration::from_secs(1)).await {
        Ok(value) => Json(value).into_response(),
        Err(error) => fail(StatusCode::UNPROCESSABLE_ENTITY, error),
    }
}
async fn snapshot(State(c): State<Arc<Context>>) -> Response {
    execute(&c, Control::Snapshot).await
}
async fn scenario(State(c): State<Arc<Context>>) -> Response {
    execute(&c, Control::Scenario).await
}
async fn control(State(c): State<Arc<Context>>, Json(control): Json<Control>) -> Response {
    execute(&c, control).await
}
async fn examples() -> Json<serde_json::Value> {
    Json(
        serde_json::json!({"Basic I4":serde_json::from_str::<serde_json::Value>(include_str!("../../examples/emulator/basic-i4.json")).unwrap(),"Colour I4":serde_json::from_str::<serde_json::Value>(include_str!("../../examples/emulator/colour-i4.json")).unwrap(),"Empty installation":{"version":1,"buses":[{"number":0}]}}),
    )
}
async fn index() -> impl IntoResponse {
    ([(header::CONTENT_TYPE,"text/html; charset=utf-8")],"<!doctype html><html lang=\"en\"><head><meta charset=\"UTF-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>DALI Emulator</title><link rel=\"stylesheet\" href=\"/assets/app.css\"></head><body><div id=\"app\"></div><script type=\"module\" src=\"/assets/app.js\"></script></body></html>")
}
async fn js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../../emulator-web/dist/app.js"),
    )
}
async fn css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../../emulator-web/dist/app.css"),
    )
}
