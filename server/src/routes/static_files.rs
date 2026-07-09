use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use tower_http::services::{ServeDir, ServeFile};

use crate::app_state::AppState;

pub fn routes() -> Router<AppState> {
    let static_dir = static_dir();
    let index_fallback = index_fallback(&static_dir);
    let spa = ServeDir::new(&static_dir).not_found_service(ServeFile::new(&index_fallback));

    Router::new()
        .route("/api/health", get(health))
        .route_service("/", ServeFile::new(&index_fallback))
        .route("/config.js", get(config_js))
        .fallback_service(spa)
}

pub fn static_dir() -> String {
    std::env::var("STATIC_DIR").unwrap_or_else(|_| "/app/dist".into())
}

pub fn index_fallback(static_dir: &str) -> String {
    format!("{}/index.html", static_dir)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

fn app_env() -> &'static str {
    match std::env::var("APP_ENV").as_deref() {
        Ok("local") => "local",
        Ok("test") => "test",
        Ok("prod") => "prod",
        _ => "prod",
    }
}

async fn config_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        format!(
            "window.APP_CONFIG = Object.freeze({{ appEnv: '{}' }});\n",
            app_env()
        ),
    )
}
