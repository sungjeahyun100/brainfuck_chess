mod app_state;
mod dto;
mod handlers;
mod mappers;
mod routes;
mod services;
mod stores;

use std::net::SocketAddr;

use app_state::AppState;

#[tokio::main]
async fn main() {
    let state = AppState::new();
    let app = routes::router(state);

    let port = std::env::var("PORT")
        .ok()
        .and_then(|raw| raw.parse::<u16>().ok())
        .unwrap_or(8080);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    axum::serve(listener, app).await.unwrap();
}
