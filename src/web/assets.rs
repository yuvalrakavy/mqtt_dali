//! Explicit embedded assets; no runtime filesystem or public application chunks.
use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
};

pub(super) async fn login() -> Html<&'static str> {
    Html(include_str!("../../web/login.html"))
}
pub(super) async fn application() -> Html<&'static str> {
    Html(include_str!("../../web/app.html"))
}
pub(super) async fn bus(Path(bus): Path<String>) -> Response {
    if matches!(bus.as_str(), "0" | "1" | "2" | "3") {
        application().await.into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
pub(super) async fn bus_tab(Path((bus, tab)): Path<(String, String)>) -> Response {
    if matches!(bus.as_str(), "0" | "1" | "2" | "3") && matches!(tab.as_str(), "groups" | "scenes")
    {
        application().await.into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
macro_rules! asset {
    ($name:ident, $mime:literal, $file:literal) => {
        pub(super) async fn $name() -> impl IntoResponse {
            (
                [(header::CONTENT_TYPE, $mime)],
                include_bytes!($file).as_slice(),
            )
        }
    };
}
asset!(
    login_js,
    "text/javascript; charset=utf-8",
    "../../web/dist/login/login.js"
);
asset!(
    login_css,
    "text/css; charset=utf-8",
    "../../web/dist/login/login.css"
);
asset!(
    app_js,
    "text/javascript; charset=utf-8",
    "../../web/dist/app/app.js"
);
asset!(
    app_css,
    "text/css; charset=utf-8",
    "../../web/dist/app/app.css"
);
