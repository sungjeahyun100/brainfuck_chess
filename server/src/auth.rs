use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use crate::account::{
    normalize_display_name, normalize_public_id, AccountUpdateError, LoginResult,
    ProfileVisibility, UserProfile, VerifiedIdentity,
};
use crate::app_state::AppState;

const COOKIE_NAME: &str = "deck_chess_session";
const MIN_SIGNING_KEY_BYTES: usize = 32;
const DEFAULT_SESSION_TTL_SECONDS: u64 = 60 * 60 * 24 * 30;
const MAX_ID_TOKEN_BYTES: usize = 16 * 1024;
const GOOGLE_CERTS_URL: &str =
    "https://www.googleapis.com/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com";

#[derive(Clone)]
pub(crate) struct AuthState {
    signing_key: Arc<[u8]>,
    secure_cookie: bool,
    session_ttl: Duration,
    token_verifier: Arc<dyn IdTokenVerifier>,
}

impl AuthState {
    pub(crate) fn from_env(app_env: &str) -> Result<Self, String> {
        let signing_key = match std::env::var("AUTH_SIGNING_KEY") {
            Ok(value) if value.len() >= MIN_SIGNING_KEY_BYTES => value.into_bytes(),
            Ok(_) => {
                return Err(format!(
                    "AUTH_SIGNING_KEY must contain at least {MIN_SIGNING_KEY_BYTES} bytes"
                ))
            }
            Err(_) if app_env == "local" => b"deck-chess-local-test-signing-key-only".to_vec(),
            Err(_) => {
                return Err(format!(
                    "AUTH_SIGNING_KEY is required for APP_ENV={app_env}"
                ))
            }
        };
        let project_id = std::env::var("IDENTITY_PLATFORM_PROJECT_ID")
            .ok()
            .filter(|value| !value.trim().is_empty());
        if app_env != "local" && project_id.is_none() {
            return Err(format!(
                "IDENTITY_PLATFORM_PROJECT_ID is required for APP_ENV={app_env}"
            ));
        }
        let token_verifier: Arc<dyn IdTokenVerifier> = match project_id {
            Some(project_id) => Arc::new(GoogleIdTokenVerifier::new(project_id)?),
            None => Arc::new(DisabledIdTokenVerifier),
        };
        let session_ttl = std::env::var("SESSION_TTL_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|seconds| (300..=60 * 60 * 24 * 90).contains(seconds))
            .unwrap_or(DEFAULT_SESSION_TTL_SECONDS);
        Ok(Self {
            signing_key: Arc::from(signing_key),
            secure_cookie: app_env != "local",
            session_ttl: Duration::from_secs(session_ttl),
            token_verifier,
        })
    }

    #[cfg(test)]
    pub(crate) fn for_tests() -> Self {
        Self {
            signing_key: Arc::from(b"deck-chess-local-test-signing-key-only".as_slice()),
            secure_cookie: false,
            session_ttl: Duration::from_secs(DEFAULT_SESSION_TTL_SECONDS),
            token_verifier: Arc::new(DisabledIdTokenVerifier),
        }
    }

    pub(crate) fn authenticate(&self, headers: &HeaderMap) -> Result<String, String> {
        #[cfg(test)]
        if let Some(value) = headers
            .get("x-user-id")
            .and_then(|value| value.to_str().ok())
        {
            return validate_user_id(value);
        }
        let token = cookie_value(headers, COOKIE_NAME)
            .ok_or_else(|| "로그인 세션이 필요합니다.".to_string())?;
        self.verify_token(token)
    }

    fn issue_token(&self, user_id: &str) -> Result<String, String> {
        let issued_at = unix_time()?;
        let payload = serde_json::to_vec(&SessionClaims {
            user_id,
            issued_at,
            expires_at: issued_at.saturating_add(self.session_ttl.as_secs()),
            nonce: Uuid::new_v4().to_string(),
        })
        .map_err(|_| "세션을 생성할 수 없습니다.".to_string())?;
        let encoded = URL_SAFE_NO_PAD.encode(payload);
        let signed = format!("v1.{encoded}");
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.signing_key)
            .expect("HMAC accepts keys of any size");
        mac.update(signed.as_bytes());
        Ok(format!(
            "{signed}.{}",
            URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
        ))
    }

    fn verify_token(&self, token: &str) -> Result<String, String> {
        if !token.starts_with("v1.") {
            return self.verify_legacy_token(token);
        }
        let mut parts = token.split('.');
        let version = parts.next();
        let payload = parts.next();
        let signature = parts.next();
        if version != Some("v1") || parts.next().is_some() {
            return Err("세션 형식이 올바르지 않습니다.".into());
        }
        let payload = payload.ok_or_else(|| "세션 형식이 올바르지 않습니다.".to_string())?;
        let signature = URL_SAFE_NO_PAD
            .decode(signature.unwrap_or_default())
            .map_err(|_| "세션 서명이 올바르지 않습니다.".to_string())?;
        let signed = format!("v1.{payload}");
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.signing_key)
            .expect("HMAC accepts keys of any size");
        mac.update(signed.as_bytes());
        mac.verify_slice(&signature)
            .map_err(|_| "세션 서명이 올바르지 않습니다.".to_string())?;
        let claims: OwnedSessionClaims = serde_json::from_slice(
            &URL_SAFE_NO_PAD
                .decode(payload)
                .map_err(|_| "세션 형식이 올바르지 않습니다.".to_string())?,
        )
        .map_err(|_| "세션 형식이 올바르지 않습니다.".to_string())?;
        let now = unix_time()?;
        if claims.issued_at > now.saturating_add(30) || claims.expires_at <= now {
            return Err("세션이 만료되었습니다.".into());
        }
        validate_user_id(&claims.user_id)
    }

    /// Accepts the immediately preceding `user_id.signature` format only so
    /// existing guest ownership remains recoverable. `/auth/session` rotates
    /// it to the expiring v1 format on the next frontend startup.
    fn verify_legacy_token(&self, token: &str) -> Result<String, String> {
        let (user_id, signature) = token
            .split_once('.')
            .ok_or_else(|| "세션 형식이 올바르지 않습니다.".to_string())?;
        let user_id = validate_user_id(user_id)?;
        let signature = URL_SAFE_NO_PAD
            .decode(signature)
            .map_err(|_| "세션 서명이 올바르지 않습니다.".to_string())?;
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.signing_key)
            .expect("HMAC accepts keys of any size");
        mac.update(user_id.as_bytes());
        mac.verify_slice(&signature)
            .map_err(|_| "세션 서명이 올바르지 않습니다.".to_string())?;
        Ok(user_id)
    }

    fn set_cookie(&self, user_id: &str) -> Result<HeaderValue, String> {
        let secure = if self.secure_cookie { "; Secure" } else { "" };
        HeaderValue::from_str(&format!(
            "{COOKIE_NAME}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{secure}",
            self.issue_token(user_id)?,
            self.session_ttl.as_secs()
        ))
        .map_err(|_| "세션 쿠키를 생성할 수 없습니다.".into())
    }

    fn clear_cookie(&self) -> HeaderValue {
        let secure = if self.secure_cookie { "; Secure" } else { "" };
        HeaderValue::from_str(&format!(
            "{COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure}"
        ))
        .expect("static cookie is valid")
    }

    fn validate_origin(&self, headers: &HeaderMap) -> Result<(), String> {
        let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
            return if self.secure_cookie {
                Err("요청 origin을 확인할 수 없습니다.".into())
            } else {
                Ok(())
            };
        };
        let host = headers
            .get("x-forwarded-host")
            .or_else(|| headers.get(header::HOST))
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| "요청 host를 확인할 수 없습니다.".to_string())?;
        let proto = headers
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(if self.secure_cookie { "https" } else { "http" });
        if origin == format!("{proto}://{host}") {
            Ok(())
        } else {
            Err("다른 origin에서 온 요청은 허용되지 않습니다.".into())
        }
    }
}

#[derive(Serialize)]
struct SessionClaims<'a> {
    user_id: &'a str,
    issued_at: u64,
    expires_at: u64,
    nonce: String,
}

#[derive(Deserialize)]
struct OwnedSessionClaims {
    user_id: String,
    issued_at: u64,
    expires_at: u64,
    #[allow(dead_code)]
    nonce: String,
}

fn unix_time() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| "시스템 시간이 올바르지 않습니다.".into())
}

fn validate_user_id(value: &str) -> Result<String, String> {
    let value = value.trim();
    if (1..=128).contains(&value.len())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        Ok(value.to_owned())
    } else {
        Err("세션 사용자 ID가 올바르지 않습니다.".into())
    }
}

fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|part| part.trim().split_once('='))
        .find_map(|(key, value)| (key == name).then_some(value))
}

#[async_trait]
trait IdTokenVerifier: Send + Sync {
    async fn verify(&self, token: &str) -> Result<VerifiedIdentity, String>;
}

struct DisabledIdTokenVerifier;

#[async_trait]
impl IdTokenVerifier for DisabledIdTokenVerifier {
    async fn verify(&self, _token: &str) -> Result<VerifiedIdentity, String> {
        Err("Identity Platform이 설정되지 않았습니다.".into())
    }
}

struct CachedKeys {
    values: HashMap<String, String>,
    expires_at: Instant,
}

struct GoogleIdTokenVerifier {
    project_id: String,
    issuer: String,
    client: reqwest::Client,
    keys: RwLock<CachedKeys>,
}

impl GoogleIdTokenVerifier {
    fn new(project_id: String) -> Result<Self, String> {
        if project_id.len() > 128
            || !project_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | ':' | '.'))
        {
            return Err("IDENTITY_PLATFORM_PROJECT_ID is invalid".into());
        }
        Ok(Self {
            issuer: format!("https://securetoken.google.com/{project_id}"),
            project_id,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .map_err(|error| format!("failed to create ID token verifier: {error}"))?,
            keys: RwLock::new(CachedKeys {
                values: HashMap::new(),
                expires_at: Instant::now(),
            }),
        })
    }

    async fn key(&self, kid: &str) -> Result<String, String> {
        if let Ok(keys) = self.keys.read() {
            if keys.expires_at > Instant::now() {
                if let Some(key) = keys.values.get(kid) {
                    return Ok(key.clone());
                }
            }
        }
        let response = self
            .client
            .get(GOOGLE_CERTS_URL)
            .send()
            .await
            .map_err(|_| "Identity Platform 공개키를 가져오지 못했습니다.".to_string())?;
        if !response.status().is_success() {
            return Err("Identity Platform 공개키를 가져오지 못했습니다.".into());
        }
        let max_age = response
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_max_age)
            .unwrap_or(300)
            .clamp(60, 86_400);
        let values = response
            .json::<HashMap<String, String>>()
            .await
            .map_err(|_| "Identity Platform 공개키 응답이 올바르지 않습니다.".to_string())?;
        let key = values
            .get(kid)
            .cloned()
            .ok_or_else(|| "ID token의 서명 키가 올바르지 않습니다.".to_string())?;
        *self
            .keys
            .write()
            .map_err(|_| "ID token 검증기를 사용할 수 없습니다.".to_string())? = CachedKeys {
            values,
            expires_at: Instant::now() + Duration::from_secs(max_age),
        };
        Ok(key)
    }
}

#[derive(Deserialize)]
struct FirebaseClaims {
    sign_in_provider: String,
}

#[derive(Deserialize)]
struct IdentityTokenClaims {
    iss: String,
    aud: String,
    sub: String,
    exp: u64,
    iat: u64,
    auth_time: u64,
    email: Option<String>,
    email_verified: Option<bool>,
    name: Option<String>,
    picture: Option<String>,
    firebase: FirebaseClaims,
}

#[async_trait]
impl IdTokenVerifier for GoogleIdTokenVerifier {
    async fn verify(&self, token: &str) -> Result<VerifiedIdentity, String> {
        if token.is_empty() || token.len() > MAX_ID_TOKEN_BYTES {
            return Err("ID token 형식이 올바르지 않습니다.".into());
        }
        let header =
            decode_header(token).map_err(|_| "ID token 형식이 올바르지 않습니다.".to_string())?;
        if header.alg != Algorithm::RS256 {
            return Err("ID token 서명 알고리즘이 올바르지 않습니다.".into());
        }
        let kid = header
            .kid
            .ok_or_else(|| "ID token 서명 키가 없습니다.".to_string())?;
        let pem = self.key(&kid).await?;
        let key = DecodingKey::from_rsa_pem(pem.as_bytes())
            .map_err(|_| "ID token 공개키가 올바르지 않습니다.".to_string())?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[&self.project_id]);
        validation.set_issuer(&[&self.issuer]);
        validation.set_required_spec_claims(&["exp", "iat", "aud", "iss", "sub"]);
        validation.leeway = 30;
        let claims = decode::<IdentityTokenClaims>(token, &key, &validation)
            .map_err(|_| "ID token을 검증할 수 없습니다.".to_string())?
            .claims;
        verified_identity_from_claims(claims, &self.project_id, &self.issuer, unix_time()?)
    }
}

fn verified_identity_from_claims(
    claims: IdentityTokenClaims,
    project_id: &str,
    issuer: &str,
    now: u64,
) -> Result<VerifiedIdentity, String> {
    if claims.iss != issuer
        || claims.aud != project_id
        || claims.sub.is_empty()
        || claims.sub.len() > 128
        || claims.exp <= now
        || claims.iat > now.saturating_add(30)
        || claims.auth_time > now.saturating_add(30)
        || claims.firebase.sign_in_provider != "google.com"
    {
        return Err("ID token claim이 올바르지 않습니다.".into());
    }
    Ok(VerifiedIdentity {
        issuer: claims.iss,
        subject: claims.sub,
        provider: "google".into(),
        email: claims.email.filter(|v| v.len() <= 320),
        email_verified: claims.email_verified.unwrap_or(false),
        display_name: claims.name.filter(|v| v.chars().count() <= 120),
        avatar_url: claims
            .picture
            .filter(|v| v.len() <= 2_048 && v.starts_with("https://")),
    })
}

fn parse_max_age(value: &str) -> Option<u64> {
    value.split(',').find_map(|part| {
        part.trim()
            .strip_prefix("max-age=")
            .and_then(|v| v.parse().ok())
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionResponse {
    user_id: String,
}

#[derive(Serialize)]
struct AuthError {
    code: &'static str,
    error: &'static str,
}

fn auth_error(status: StatusCode, code: &'static str, error: &'static str) -> Response {
    (status, Json(AuthError { code, error })).into_response()
}

pub(crate) async fn session(State(app): State<AppState>, headers: HeaderMap) -> Response {
    let user_id = app
        .auth
        .authenticate(&headers)
        .unwrap_or_else(|_| Uuid::new_v4().to_string());
    if app.accounts.ensure_guest(&user_id).await.is_err() {
        return auth_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "account_store_unavailable",
            "계정 저장소를 사용할 수 없습니다.",
        );
    }
    let mut response = Json(SessionResponse {
        user_id: user_id.clone(),
    })
    .into_response();
    match app.auth.set_cookie(&user_id) {
        Ok(cookie) => {
            response.headers_mut().insert(header::SET_COOKIE, cookie);
            response
        }
        Err(_) => auth_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session_issue_failed",
            "세션을 생성할 수 없습니다.",
        ),
    }
}

#[derive(Serialize)]
struct MeResponse {
    authenticated: bool,
    user: Option<UserProfile>,
}

pub(crate) async fn me(State(app): State<AppState>, headers: HeaderMap) -> Response {
    let Ok(user_id) = app.auth.authenticate(&headers) else {
        return Json(MeResponse {
            authenticated: false,
            user: None,
        })
        .into_response();
    };
    match app.accounts.authenticated_user(&user_id).await {
        Ok(user) => Json(MeResponse {
            authenticated: user.is_some(),
            user,
        })
        .into_response(),
        Err(_) => auth_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "account_store_unavailable",
            "계정 저장소를 사용할 수 없습니다.",
        ),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateProfileRequest {
    #[serde(default)]
    public_id: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    profile_visibility: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct UpdateProfileResponse {
    user: UserProfile,
}

pub(crate) async fn update_profile(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<UpdateProfileRequest>,
) -> Response {
    if app.auth.validate_origin(&headers).is_err() {
        return auth_error(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "요청 origin이 허용되지 않습니다.",
        );
    }
    let Ok(user_id) = app.auth.authenticate(&headers) else {
        return auth_error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "로그인이 필요합니다.",
        );
    };
    if input.public_id.is_none()
        && input.display_name.is_none()
        && input.profile_visibility.is_none()
    {
        return auth_error(
            StatusCode::BAD_REQUEST,
            "empty_profile_update",
            "변경할 계정 정보를 입력해야 합니다.",
        );
    }
    let public_id = match input.public_id.as_deref().map(normalize_public_id) {
        Some(Ok(value)) => Some(value),
        Some(Err(())) => {
            return auth_error(
                StatusCode::BAD_REQUEST,
                "invalid_public_id",
                "ID는 예약어를 제외한 영문 소문자, 숫자, 밑줄 3~20자이며 첫 글자는 영문 또는 숫자여야 합니다.",
            )
        }
        None => None,
    };
    let display_name = match input.display_name.as_deref().map(normalize_display_name) {
        Some(Ok(value)) => Some(value),
        Some(Err(())) => {
            return auth_error(
                StatusCode::BAD_REQUEST,
                "invalid_display_name",
                "표시 이름은 제어 문자를 제외하고 1~30자로 입력해야 합니다.",
            )
        }
        None => None,
    };
    let profile_visibility = match input.profile_visibility.as_ref() {
        Some(serde_json::Value::String(value)) => match ProfileVisibility::parse(value) {
            Ok(value) => Some(value),
            Err(()) => {
                return auth_error(
                    StatusCode::BAD_REQUEST,
                    "invalid_profile_visibility",
                    "계정 공개 설정은 public 또는 private이어야 합니다.",
                )
            }
        },
        Some(_) => {
            return auth_error(
                StatusCode::BAD_REQUEST,
                "invalid_profile_visibility",
                "계정 공개 설정은 public 또는 private이어야 합니다.",
            )
        }
        None => None,
    };
    match app
        .accounts
        .update_profile(
            &user_id,
            public_id.as_deref(),
            display_name.as_deref(),
            profile_visibility,
        )
        .await
    {
        Ok(user) => Json(UpdateProfileResponse { user }).into_response(),
        Err(AccountUpdateError::NotFound) => auth_error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "로그인이 필요합니다.",
        ),
        Err(AccountUpdateError::PublicIdTaken) => auth_error(
            StatusCode::CONFLICT,
            "public_id_taken",
            "이미 사용 중인 ID입니다.",
        ),
        Err(AccountUpdateError::Unavailable) => auth_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "account_store_unavailable",
            "계정 저장소를 사용할 수 없습니다.",
        ),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GoogleLoginRequest {
    id_token: String,
    #[serde(default)]
    import_guest_data: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GoogleLoginResponse {
    authenticated: bool,
    user: UserProfile,
    imported_guest_data: bool,
}

pub(crate) async fn google_login(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<GoogleLoginRequest>,
) -> Response {
    if app.auth.validate_origin(&headers).is_err() {
        return auth_error(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "요청 origin이 허용되지 않습니다.",
        );
    }
    let Ok(current_user_id) = app.auth.authenticate(&headers) else {
        return auth_error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "guest 세션이 필요합니다.",
        );
    };
    let identity = match app.auth.token_verifier.verify(&input.id_token).await {
        Ok(identity) => identity,
        Err(_) => {
            return auth_error(
                StatusCode::UNAUTHORIZED,
                "invalid_id_token",
                "Google 로그인 정보를 검증할 수 없습니다.",
            )
        }
    };
    match app
        .accounts
        .complete_google_login(&current_user_id, &identity, input.import_guest_data)
        .await
    {
        Ok(LoginResult::ImportRequired) => auth_error(
            StatusCode::CONFLICT,
            "guest_import_required",
            "이 브라우저의 게스트 커스텀 기물을 계정으로 가져올지 선택해 주세요.",
        ),
        Ok(LoginResult::Complete {
            user,
            imported_guest_data,
        }) => {
            let mut response = Json(GoogleLoginResponse {
                authenticated: true,
                user: user.clone(),
                imported_guest_data,
            })
            .into_response();
            match app.auth.set_cookie(&user.id) {
                Ok(cookie) => {
                    response.headers_mut().insert(header::SET_COOKIE, cookie);
                    response
                }
                Err(_) => auth_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "session_issue_failed",
                    "세션을 생성할 수 없습니다.",
                ),
            }
        }
        Err(_) => auth_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "account_store_unavailable",
            "계정 저장소를 사용할 수 없습니다.",
        ),
    }
}

pub(crate) async fn logout(State(app): State<AppState>, headers: HeaderMap) -> Response {
    if app.auth.validate_origin(&headers).is_err() {
        return auth_error(
            StatusCode::FORBIDDEN,
            "origin_rejected",
            "요청 origin이 허용되지 않습니다.",
        );
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, app.auth.clear_cookie());
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_cookie_round_trip_and_forgery_rejection() {
        let auth = AuthState::for_tests();
        let token = auth.issue_token("alice").unwrap();
        assert_eq!(auth.verify_token(&token).unwrap(), "alice");
        assert!(auth.verify_token("v1.invalid.invalid").is_err());
        let mut tampered = token;
        tampered.push('x');
        assert!(auth.verify_token(&tampered).is_err());
        let mut mac = Hmac::<Sha256>::new_from_slice(&auth.signing_key).unwrap();
        mac.update(b"legacy-guest");
        let legacy = format!(
            "legacy-guest.{}",
            URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
        );
        assert_eq!(auth.verify_token(&legacy).unwrap(), "legacy-guest");
    }

    #[test]
    fn parses_public_key_cache_lifetime() {
        assert_eq!(
            parse_max_age("public, max-age=3600, must-revalidate"),
            Some(3600)
        );
        assert_eq!(parse_max_age("no-cache"), None);
    }

    fn claims(now: u64) -> IdentityTokenClaims {
        IdentityTokenClaims {
            iss: "https://securetoken.google.com/project".into(),
            aud: "project".into(),
            sub: "google-subject".into(),
            exp: now + 300,
            iat: now - 10,
            auth_time: now - 10,
            email: Some("player@example.com".into()),
            email_verified: Some(true),
            name: Some("Player".into()),
            picture: Some("https://example.com/avatar.png".into()),
            firebase: FirebaseClaims {
                sign_in_provider: "google.com".into(),
            },
        }
    }

    #[test]
    fn identity_claims_reject_wrong_audience_issuer_expiry_and_provider() {
        let now = 10_000;
        assert!(verified_identity_from_claims(
            claims(now),
            "project",
            "https://securetoken.google.com/project",
            now
        )
        .is_ok());
        let mut wrong_audience = claims(now);
        wrong_audience.aud = "other".into();
        assert!(verified_identity_from_claims(
            wrong_audience,
            "project",
            "https://securetoken.google.com/project",
            now
        )
        .is_err());
        let mut expired = claims(now);
        expired.exp = now;
        assert!(verified_identity_from_claims(
            expired,
            "project",
            "https://securetoken.google.com/project",
            now
        )
        .is_err());
        let mut wrong_provider = claims(now);
        wrong_provider.firebase.sign_in_provider = "password".into();
        assert!(verified_identity_from_claims(
            wrong_provider,
            "project",
            "https://securetoken.google.com/project",
            now
        )
        .is_err());
    }

    #[test]
    fn state_changing_auth_requests_require_the_same_origin() {
        let mut auth = AuthState::for_tests();
        auth.secure_cookie = true;
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("deck.example"));
        headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://deck.example"),
        );
        assert!(auth.validate_origin(&headers).is_ok());
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://evil.example"),
        );
        assert!(auth.validate_origin(&headers).is_err());
        headers.remove(header::ORIGIN);
        assert!(auth.validate_origin(&headers).is_err());
    }

    #[tokio::test]
    async fn profile_update_rejects_unknown_or_null_visibility() {
        let app = AppState::in_memory();
        let identity = VerifiedIdentity {
            issuer: "issuer".into(),
            subject: "subject".into(),
            provider: "google".into(),
            email: None,
            email_verified: true,
            display_name: Some("Player".into()),
            avatar_url: None,
        };
        app.accounts
            .complete_google_login("privacy-user", &identity, None)
            .await
            .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("x-user-id", HeaderValue::from_static("privacy-user"));

        for value in [serde_json::json!("friends_only"), serde_json::Value::Null] {
            let response = update_profile(
                State(app.clone()),
                headers.clone(),
                Json(UpdateProfileRequest {
                    public_id: None,
                    display_name: Some("Still Player".into()),
                    profile_visibility: Some(value),
                }),
            )
            .await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
        assert_eq!(
            app.accounts
                .authenticated_user("privacy-user")
                .await
                .unwrap()
                .unwrap()
                .profile_visibility,
            ProfileVisibility::Public
        );
    }
}
