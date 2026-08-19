use axum::body::Body;
use axum::http::{header, HeaderValue, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

const SENSITIVE_NAMES: &[&str] = &[
    "cargo.toml",
    "cargo.lock",
    "composer.json",
    "composer.lock",
    "dockerfile",
    "docker-compose.yml",
    "docker-compose.yaml",
    "package.json",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "web.config",
    "wp-config.php",
    "wp-login.php",
    "xmlrpc.php",
];

const PROBE_SEGMENTS: &[&str] = &[
    "actuator",
    "cgi-bin",
    "phpmyadmin",
    "server-status",
    "vendor",
    "wp-admin",
    "wp-content",
    "wp-includes",
];

/// Reject common secret/source probes before the SPA fallback can turn them
/// into a misleading 200 response. This is deliberately path-based; normal
/// application routes and `/.well-known` challenges remain available.
pub(crate) async fn block_sensitive_paths(request: Request<Body>, next: Next) -> Response {
    if is_forbidden_path(request.uri().path()) {
        let mut response = StatusCode::FORBIDDEN.into_response();
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response.headers_mut().insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        return response;
    }
    next.run(request).await
}

fn is_forbidden_path(raw_path: &str) -> bool {
    let Ok(decoded) = repeatedly_decode_path(raw_path) else {
        return true;
    };
    let normalized = decoded.replace('\\', "/");
    for (index, segment) in normalized
        .split('/')
        .filter(|segment| !segment.is_empty())
        .enumerate()
    {
        let lower = segment.to_ascii_lowercase();
        if lower == "." || lower == ".." {
            return true;
        }
        if lower.starts_with('.') && !(index == 0 && lower == ".well-known") {
            return true;
        }
        if SENSITIVE_NAMES.contains(&lower.as_str())
            || PROBE_SEGMENTS.contains(&lower.as_str())
            || matches!(
                lower.rsplit_once('.').map(|(_, extension)| extension),
                Some("php" | "asp" | "aspx" | "cgi")
            )
        {
            return true;
        }
    }
    false
}

fn repeatedly_decode_path(raw_path: &str) -> Result<String, ()> {
    let mut current = raw_path.to_owned();
    for _ in 0..3 {
        let decoded = percent_decode_once(&current)?;
        if decoded == current {
            return Ok(decoded);
        }
        current = decoded;
    }
    Ok(current)
}

fn percent_decode_once(value: &str) -> Result<String, ()> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            decoded.push(bytes[index]);
            index += 1;
            continue;
        }
        let high = *bytes.get(index + 1).ok_or(())?;
        let low = *bytes.get(index + 2).ok_or(())?;
        decoded.push(hex_value(high)? * 16 + hex_value(low)?);
        index += 3;
    }
    String::from_utf8(decoded).map_err(|_| ())
}

fn hex_value(value: u8) -> Result<u8, ()> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::is_forbidden_path;

    #[test]
    fn blocks_dotfiles_encoded_bypasses_and_common_probes() {
        for path in [
            "/.env",
            "/.env.production",
            "/.git/config",
            "/foo/.ssh/id_rsa",
            "/%2eenv",
            "/%252eenv",
            "/foo%5c.env",
            "/.well-known/%2eenv",
            "/wp-login.php",
            "/phpmyadmin/index.php",
            "/server-status",
            "/Cargo.toml",
            "/bad%2",
        ] {
            assert!(is_forbidden_path(path), "expected forbidden: {path}");
        }
    }

    #[test]
    fn permits_application_assets_routes_and_well_known_challenges() {
        for path in [
            "/",
            "/custom-piece-workshop",
            "/api/health",
            "/api/auth/google",
            "/api/auth/me",
            "/api/auth/logout",
            "/config.js",
            "/assets/index-BiXqmH8u.css",
            "/.well-known/acme-challenge/token",
        ] {
            assert!(!is_forbidden_path(path), "expected allowed: {path}");
        }
    }
}
