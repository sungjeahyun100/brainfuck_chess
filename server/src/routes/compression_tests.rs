use super::*;
use async_compression::tokio::bufread::{BrotliDecoder, GzipDecoder};
use axum::body::{to_bytes, Body};
use axum::http::{header, Request};
use tokio::io::AsyncReadExt;
use tower::ServiceExt;

fn app() -> Router {
    Router::new().nest("/api", api(AppState::in_memory()))
}

#[tokio::test]
async fn compression_lab_json_round_trips_and_negotiates_encodings() {
    let payload = serde_json::json!({
        "board_size": 8,
        "selected_piece_id": "rook",
        "pieces": [{"id": "rook", "piece_type": "rook", "owner": "white",
                    "square": {"file": 3, "rank": 3}}]
    });
    // Compare with the handler before any HTTP middleware, preserving the API contract.
    let Json(expected) = get_lab_piece_options(
        State(AppState::in_memory()),
        HeaderMap::new(),
        Json(serde_json::from_value(payload.clone()).unwrap()),
    )
    .await
    .unwrap_or_else(|_| panic!("valid lab fixture must succeed"));
    let expected = serde_json::to_value(expected).unwrap();
    let mut plain_size = 0;
    for (accept, encoding) in [
        (None, None),
        (Some("gzip"), Some("gzip")),
        (Some("br"), Some("br")),
        (Some("gzip, br"), Some("br")),
        (Some("gzip;q=1, br;q=0"), Some("gzip")),
    ] {
        let mut request = Request::post("/api/lab/piece-options")
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(accept) = accept {
            request = request.header(header::ACCEPT_ENCODING, accept);
        }
        let response = app()
            .oneshot(request.body(Body::from(payload.to_string())).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_ENCODING)
                .map(|v| v.to_str().unwrap()),
            encoding
        );
        if encoding.is_some() {
            assert!(response.headers().get_all(header::VARY).iter().any(|v| v
                .to_str()
                .unwrap()
                .split(',')
                .any(|v| v.trim().eq_ignore_ascii_case("accept-encoding"))));
        }
        let wire = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let mut decoded = Vec::new();
        match encoding {
            Some("gzip") => {
                GzipDecoder::new(&wire[..])
                    .read_to_end(&mut decoded)
                    .await
                    .unwrap();
            }
            Some("br") => {
                BrotliDecoder::new(&wire[..])
                    .read_to_end(&mut decoded)
                    .await
                    .unwrap();
            }
            None => {
                plain_size = wire.len();
                decoded.extend_from_slice(&wire);
            }
            _ => unreachable!(),
        }
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&decoded).unwrap(),
            expected
        );
        if encoding.is_some() {
            assert!(wire.len() < plain_size);
            eprintln!(
                "lab fixture: identity={plain_size} bytes, {}={} bytes",
                encoding.unwrap(),
                wire.len()
            );
        }
    }
}

#[tokio::test]
async fn compression_preserves_small_responses_auth_and_live_cache_headers() {
    for accept in ["gzip", "br"] {
        let response = app()
            .oneshot(
                Request::get("/api/health")
                    .header(header::ACCEPT_ENCODING, accept)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
        assert_eq!(
            &to_bytes(response.into_body(), usize::MAX).await.unwrap()[..],
            b"{\"status\":\"ok\"}"
        );

        let response = app()
            .oneshot(
                Request::get("/api/decks")
                    .header(header::ACCEPT_ENCODING, accept)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");

        let response = app()
            .oneshot(
                Request::get("/api/games/missing")
                    .header(header::ACCEPT_ENCODING, accept)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert!(response
            .headers()
            .get_all(header::VARY)
            .iter()
            .any(|v| v == "x-game-client-id"));
    }
}

#[tokio::test]
async fn compression_leaves_static_file_service_outside_api_unchanged() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let expected = tokio::fs::read(&path).await.unwrap();
    let app = app().fallback_service(ServeFile::new(path));
    let response = app
        .oneshot(
            Request::get("/static-fixture")
                .header(header::ACCEPT_ENCODING, "gzip, br")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
    assert_eq!(
        &to_bytes(response.into_body(), usize::MAX).await.unwrap()[..],
        expected
    );
}
