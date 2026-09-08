use super::*;
use axum::{
    body::{to_bytes, Body},
    http::{HeaderValue, Request},
};
use std::{collections::HashMap, sync::Mutex};
use tower::Service;

#[derive(Default)]
pub(crate) struct MemoryDeckRepository(
    Mutex<HashMap<(String, String), (SavedDeck, String, String)>>,
);
#[async_trait]
impl DeckRepository for MemoryDeckRepository {
    async fn list(&self, owner: &str) -> Result<Vec<SavedDeck>> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|((o, _), _)| o == owner)
            .map(|(_, (d, _, _))| d.clone())
            .collect())
    }
    async fn get(&self, owner: &str, id: &str) -> Result<SavedDeck> {
        self.0
            .lock()
            .unwrap()
            .get(&(owner.into(), id.into()))
            .map(|(d, _, _)| d.clone())
            .ok_or(DeckError::NotFound)
    }
    async fn create(&self, owner: &str, key: &str, mut input: DeckInput) -> Result<SavedDeck> {
        let mut data = self.0.lock().unwrap();
        let hash = fingerprint(&input);
        if let Some((d, _, h)) = data
            .iter()
            .find(|((o, _), (_, k, _))| o == owner && k == key)
            .map(|(_, v)| v)
        {
            return if h == &hash {
                Ok(d.clone())
            } else {
                Err(DeckError::Conflict)
            };
        }
        if data.keys().filter(|(o, _)| o == owner).count() >= MAX_DECKS as usize {
            return Err(DeckError::Limit);
        }
        if key.starts_with("import:") {
            let names = data
                .iter()
                .filter(|((o, _), _)| o == owner)
                .map(|(_, (d, _, _))| d.name.clone())
                .collect::<Vec<_>>();
            input.name = imported_name(&input.name, &names);
        }
        let deck = SavedDeck {
            id: Uuid::new_v4().to_string(),
            name: input.name,
            data: input.deck_data,
            created_at: 1,
            updated_at: 1,
            version: 1,
        };
        data.insert(
            (owner.into(), deck.id.clone()),
            (deck.clone(), key.into(), hash),
        );
        Ok(deck)
    }
    async fn update(
        &self,
        owner: &str,
        id: &str,
        version: i64,
        input: DeckInput,
    ) -> Result<SavedDeck> {
        let mut data = self.0.lock().unwrap();
        let (deck, _, _) = data
            .get_mut(&(owner.into(), id.into()))
            .ok_or(DeckError::NotFound)?;
        if deck.version != version {
            return Err(DeckError::Conflict);
        }
        deck.name = input.name;
        deck.data = input.deck_data;
        deck.version += 1;
        deck.updated_at += 1;
        Ok(deck.clone())
    }
    async fn delete(&self, owner: &str, id: &str, version: i64) -> Result<()> {
        let mut data = self.0.lock().unwrap();
        let key = (owner.into(), id.into());
        if data.get(&key).ok_or(DeckError::NotFound)?.0.version != version {
            return Err(DeckError::Conflict);
        }
        data.remove(&key);
        Ok(())
    }
}
fn input() -> DeckInput {
    let mut starting = vec![Placement {
        piece_type: "king".into(),
        square: Position { file: 4, rank: 0 },
    }];
    starting.extend((0..8).map(|file| Placement {
        piece_type: "pawn".into(),
        square: Position { file, rank: 1 },
    }));
    DeckInput {
        name: "테스트 덱".into(),
        deck_data: DeckData {
            map_id: "standard-8x8".into(),
            board_size: 8,
            starting,
            pocket: BTreeMap::from([("knight".into(), 2)]),
            custom_pieces: vec![],
        },
    }
}
async fn account(app: &AppState, owner: &str) {
    app.accounts
        .complete_google_login(
            owner,
            &crate::account::VerifiedIdentity {
                issuer: "test".into(),
                subject: owner.into(),
                provider: "google".into(),
                email: None,
                email_verified: true,
                display_name: Some(owner.into()),
                avatar_url: None,
            },
            Some(false),
        )
        .await
        .unwrap();
}
async fn request(
    app: &AppState,
    owner: &str,
    method: &str,
    path: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = crate::routes::api(app.clone())
        .call(
            Request::builder()
                .method(method)
                .uri(path)
                .header("x-user-id", owner)
                .header("x-deck-account", app.auth.account_context(owner))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    if path.starts_with("/decks") {
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
    }
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 200_000).await.unwrap();
    let data = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, data)
}
fn create_input() -> serde_json::Value {
    let mut value = serde_json::to_value(input()).unwrap();
    value["requestId"] = Uuid::new_v4().to_string().into();
    value
}
#[tokio::test]
async fn account_crud_cross_browser_ownership_and_stale_writes() {
    let app = AppState::in_memory();
    account(&app, "alice").await;
    account(&app, "bob").await;
    let payload = create_input();
    let (status, created) = request(&app, "alice", "POST", "/decks", payload.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert!(created.get("owner_id").is_none());
    assert!(created.get("ownerId").is_none());
    assert_eq!(created["pocket"]["knight"], 2);
    let path = format!("/decks/{}", created["id"].as_str().unwrap());
    let (_, retry) = request(&app, "alice", "POST", "/decks", payload).await;
    assert_eq!(retry["id"], created["id"]);
    // A fresh router (another browser/session) sees the same account data.
    let (_, listed) = request(&app, "alice", "GET", "/decks", serde_json::Value::Null).await;
    assert_eq!(listed["items"].as_array().unwrap().len(), 1);
    let (_, read) = request(&app, "alice", "GET", &path, serde_json::Value::Null).await;
    assert_eq!(read, created);
    let mut update = serde_json::to_value(input()).unwrap();
    update["expectedVersion"] = 1.into();
    update["name"] = "수정".into();
    for (method, body) in [
        ("GET", serde_json::Value::Null),
        ("PUT", update.clone()),
        ("DELETE", serde_json::json!({"expectedVersion":1})),
    ] {
        assert_eq!(
            request(&app, "bob", method, &path, body).await.0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        request(&app, "alice", "PUT", &path, update.clone()).await.0,
        StatusCode::OK
    );
    assert_eq!(
        request(&app, "alice", "PUT", &path, update).await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(
            &app,
            "alice",
            "DELETE",
            &path,
            serde_json::json!({"expectedVersion":1})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(
            &app,
            "alice",
            "DELETE",
            &path,
            serde_json::json!({"expectedVersion":2})
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(&app, "alice", "GET", &path, serde_json::Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}
#[tokio::test]
async fn rejects_guest_spoofed_owner_unknown_fields_bad_format_and_limits() {
    let app = AppState::in_memory();
    account(&app, "alice").await;
    app.accounts.ensure_guest("guest").await.unwrap();
    assert_eq!(
        request(&app, "guest", "POST", "/decks", create_input())
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let mut bad = create_input();
    bad["owner_id"] = "bob".into();
    assert_eq!(
        request(&app, "alice", "POST", "/decks", bad).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    for (pointer, value) in [
        ("/deckData/boardSize", 7.into()),
        ("/deckData/mapId", "wrong".into()),
        ("/deckData/pocket/knight", (-1).into()),
        ("/deckData/pocket/knight", 1025.into()),
        ("/deckData/starting/0/pieceType", "unknown".into()),
        ("/deckData/starting/0/square/rank", 8.into()),
        ("/deckData/starting/0/square/rank", i32::MIN.into()),
        ("/deckData/starting/0/square/rank", i32::MAX.into()),
        ("/deckData/starting/0/square/file", 1.5.into()),
        ("/name", "".into()),
    ] {
        let mut bad = create_input();
        if pointer == "/deckData/pocket/king" {
            bad["deckData"]["pocket"]["king"] = value;
        } else {
            *bad.pointer_mut(pointer).unwrap() = value;
        }
        assert!(
            request(&app, "alice", "POST", "/decks", bad)
                .await
                .0
                .is_client_error(),
            "{pointer}"
        );
    }
    let mut bad = create_input();
    bad["name"] = "x".repeat(MAX_DECK_BYTES + 1).into();
    assert_eq!(
        request(&app, "alice", "POST", "/decks", bad).await.0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    let mut bad = input();
    bad.deck_data
        .starting
        .push(bad.deck_data.starting[0].clone());
    assert!(bad.validate(&app, "alice").await.is_err());
    let mut headers = HeaderMap::new();
    headers.insert("x-user-id", HeaderValue::from_static("alice"));
    headers.insert("x-deck-account", HeaderValue::from_static("bob"));
    assert!(matches!(
        owner(&app, &headers, true).await,
        Err(DeckError::IdentityChanged)
    ));
    headers.insert("origin", HeaderValue::from_static("https://evil.example"));
    headers.insert("host", HeaderValue::from_static("good.example"));
    assert!(matches!(
        owner(&app, &headers, true).await,
        Err(DeckError::Origin)
    ));
}
#[tokio::test]
async fn import_is_content_based_idempotent_and_preserves_edited_account_deck() {
    let app = AppState::in_memory();
    account(&app, "alice").await;
    let original = serde_json::to_value(input()).unwrap();
    let (_, a) = request(&app, "alice", "POST", "/decks/import", original.clone()).await;
    let mut reordered = input();
    reordered.deck_data.starting.reverse();
    reordered.deck_data.pocket.insert("rook".into(), 0);
    let (_, b) = request(
        &app,
        "alice",
        "POST",
        "/decks/import",
        serde_json::to_value(reordered).unwrap(),
    )
    .await;
    assert_eq!(a["id"], b["id"]);
    let mut update = original.clone();
    update["name"] = "계정에서 수정".into();
    update["expectedVersion"] = 1.into();
    let path = format!("/decks/{}", a["id"].as_str().unwrap());
    assert_eq!(
        request(&app, "alice", "PUT", &path, update).await.0,
        StatusCode::OK
    );
    let (_, again) = request(&app, "alice", "POST", "/decks/import", original.clone()).await;
    assert_eq!(again["name"], "계정에서 수정");
    let mut different = original;
    different["deckData"]["pocket"]["knight"] = 3.into();
    let (_, other) = request(&app, "alice", "POST", "/decks/import", different).await;
    assert_ne!(other["id"], a["id"]);
}
#[tokio::test]
async fn custom_references_preserve_pinned_version_and_reject_foreign_package() {
    use brainfuck_chess_engine::{
        custom_pieces::CUSTOM_PIECE_SCRIPT_FORMAT, pieces::default_pieces::all_default_definitions,
    };
    let app = AppState::in_memory();
    account(&app, "alice").await;
    account(&app, "bob").await;
    let mut definition = all_default_definitions()
        .into_iter()
        .find(|d| d.id == "knight")
        .unwrap();
    definition.id = "hero".into();
    definition.name = "Hero".into();
    let raw = serde_json::json!({"format":CUSTOM_PIECE_SCRIPT_FORMAT,"definitions":[definition]})
        .to_string();
    let (status,package)=request(&app,"alice","POST","/custom-pieces",serde_json::json!({
        "name":"Hero","description":"", "score":7,"image":{"kind":"built_in","asset_key":"knight"},
        "raw_script":raw,"exposed_piece_key":"hero"
    })).await;
    assert_eq!(status, StatusCode::CREATED, "{package}");
    let mut deck = input();
    let reference = CustomRef {
        id: package["id"].as_str().unwrap().into(),
        version: 1,
        content_hash: package["content_hash"].as_str().unwrap().into(),
        exposed_piece_key: "hero".into(),
    };
    let key = reference.key();
    deck.deck_data.pocket.insert(key, 1);
    deck.deck_data.custom_pieces.push(reference);
    let payload = serde_json::to_value(&deck).unwrap();
    let (status, saved) = request(&app, "alice", "POST", "/decks/import", payload.clone()).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["customPieces"], payload["deckData"]["customPieces"]);
    assert!(deck.clone().validate(&app, "bob").await.is_err());
    app.custom_pieces
        .deactivate("alice", &deck.deck_data.custom_pieces[0].id, 1)
        .await
        .unwrap();
    assert!(deck.clone().validate(&app, "alice").await.is_ok());
    assert_eq!(
        start_game(&app, deck).await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let mut tampered = payload;
    tampered["deckData"]["customPieces"][0]["contentHash"] = "wrong".into();
    assert_eq!(
        request(&app, "alice", "POST", "/decks/import", tampered)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
}
#[tokio::test]
async fn absent_database_is_unavailable_not_empty_or_successful() {
    let store = PostgresDeckRepository::new(None, DataSchema::Test);
    assert!(matches!(
        store.list("alice").await,
        Err(DeckError::Unavailable)
    ));
    assert!(matches!(
        store.create("alice", "key", input()).await,
        Err(DeckError::Unavailable)
    ));
}

#[tokio::test]
#[ignore = "requires TEST_DECK_DATABASE_URL for a disposable split-schema database"]
async fn postgres_persistence_concurrent_import_ownership_and_environment_isolation() {
    let url = std::env::var("TEST_DECK_DATABASE_URL").expect("disposable DB URL required");
    let pool = PgPool::connect(&url).await.unwrap();
    let owner = format!("deck-test-{}", Uuid::new_v4());
    sqlx::query("INSERT INTO shared.users (id) VALUES ($1)")
        .bind(&owner)
        .execute(&pool)
        .await
        .unwrap();
    let test = PostgresDeckRepository::new(Some(pool.clone()), DataSchema::Test);
    let prod = PostgresDeckRepository::new(Some(pool.clone()), DataSchema::Prod);
    let key = format!("import:{}", fingerprint(&input()));
    let (a, b) = tokio::join!(
        test.create(&owner, &key, input()),
        test.create(&owner, &key, input())
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.id, b.id);
    let reconnected =
        PostgresDeckRepository::new(Some(PgPool::connect(&url).await.unwrap()), DataSchema::Test);
    assert_eq!(reconnected.get(&owner, &a.id).await.unwrap().data, a.data);
    assert_eq!(reconnected.list(&owner).await.unwrap().len(), 1);
    assert!(prod.list(&owner).await.unwrap().is_empty());
    // No uniqueness constraint on create_hash: identical deliberate creates survive reconnect.
    let first_copy = test.create(&owner, "create:copy-1", input()).await.unwrap();
    let second_copy = test.create(&owner, "create:copy-2", input()).await.unwrap();
    assert_ne!(first_copy.id, second_copy.id);
    assert_ne!(first_copy.id, a.id);
    assert_eq!(
        reconnected.get(&owner, &second_copy.id).await.unwrap().data,
        a.data
    );
    let mut draft = input();
    draft.deck_data.starting.clear();
    draft.deck_data.pocket.insert("queen".into(), 100);
    let saved_draft = test
        .create(&owner, "create:draft", draft.clone())
        .await
        .unwrap();
    assert_eq!(
        reconnected.get(&owner, &saved_draft.id).await.unwrap().data,
        draft.deck_data
    );
    reconnected
        .update(&owner, &saved_draft.id, 1, input())
        .await
        .unwrap();
    let maximum = size_fixture(256, 144);
    let saved_maximum = test
        .create(&owner, "create:maximum", maximum.clone())
        .await
        .unwrap();
    assert_eq!(
        reconnected
            .get(&owner, &saved_maximum.id)
            .await
            .unwrap()
            .data,
        maximum.deck_data
    );
    let jsonb_bytes: i32 =
        sqlx::query_scalar("SELECT octet_length(deck_data::text) FROM test.decks WHERE id=$1")
            .bind(&saved_maximum.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    println!("maximum actual PostgreSQL JSONB text: {jsonb_bytes} bytes");
    assert_eq!(jsonb_bytes, 109272);
    let mut different = input();
    different.deck_data.pocket.insert("knight".into(), 3);
    let different_key = format!("import:{}", fingerprint(&different));
    let different = test
        .create(&owner, &different_key, different)
        .await
        .unwrap();
    assert_eq!(different.name, format!("{} (가져옴 2)", a.name));
    test.delete(&owner, &different.id, different.version)
        .await
        .unwrap();
    let production = prod.create(&owner, &key, input()).await.unwrap();
    assert_ne!(production.id, a.id);
    assert!(matches!(
        test.get("another-user", &a.id).await,
        Err(DeckError::NotFound)
    ));
    assert!(matches!(
        test.update("another-user", &a.id, 1, input()).await,
        Err(DeckError::NotFound)
    ));
    assert!(matches!(
        test.delete("another-user", &a.id, 1).await,
        Err(DeckError::NotFound)
    ));
    let (first, second) = tokio::join!(
        test.update(&owner, &a.id, 1, input()),
        test.update(&owner, &a.id, 1, input())
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert!(matches!(
        test.delete(&owner, &a.id, 1).await,
        Err(DeckError::Conflict)
    ));
    test.delete(&owner, &a.id, 2).await.unwrap();
    assert_eq!(prod.get(&owner, &production.id).await.unwrap().version, 1);
    // Owner deletion cascades in both environments. Only this test's unique fixture is removed.
    sqlx::query("DELETE FROM shared.users WHERE id=$1")
        .bind(&owner)
        .execute(&pool)
        .await
        .unwrap();
    assert!(test.list(&owner).await.unwrap().is_empty());
    assert!(prod.list(&owner).await.unwrap().is_empty());
}

#[test]
fn imports_with_same_name_receive_a_bounded_distinct_name() {
    assert_eq!(
        imported_name("덱", &["덱".into(), "덱 (가져옴 2)".into()]),
        "덱 (가져옴 3)"
    );
    let name = "가".repeat(100);
    let new_name = imported_name(&name, &[name.clone()]);
    assert_eq!(new_name.chars().count(), 100);
    assert!(new_name.ends_with(" (가져옴 2)"));
}

async fn start_game(app: &AppState, mut deck: DeckInput) -> StatusCode {
    let white = deck.spec().unwrap();
    let black = crate::materialize_neutral_deck(&white, "black", deck.deck_data.board_size);
    request(
        app,
        "alice",
        "POST",
        "/games",
        serde_json::json!({
            "board_size":deck.deck_data.board_size, "map_id":deck.deck_data.map_id,
            "white_deck":white, "black_deck":black
        }),
    )
    .await
    .0
}

#[tokio::test]
async fn drafts_save_reload_edit_but_game_start_requires_a_complete_deck() {
    let app = AppState::in_memory();
    account(&app, "alice").await;
    let mut drafts = Vec::new();
    let mut empty = input();
    empty.deck_data.starting.clear();
    drafts.push(empty);
    let mut king_only = input();
    king_only.deck_data.starting.truncate(1);
    drafts.push(king_only);
    let mut two_kings = input();
    two_kings.deck_data.starting.push(Placement {
        piece_type: "king".into(),
        square: Position { file: 3, rank: 0 },
    });
    drafts.push(two_kings);
    let mut score = input();
    score.deck_data.pocket.insert("queen".into(), 100);
    drafts.push(score);
    let mut pocket_king = input();
    pocket_king.deck_data.pocket.insert("king".into(), 1);
    drafts.push(pocket_king);
    let mut placement = input();
    placement.deck_data.starting[0].square.rank = 7;
    drafts.push(placement);
    for draft in drafts {
        let mut payload = serde_json::to_value(&draft).unwrap();
        payload["requestId"] = Uuid::new_v4().to_string().into();
        let (status, saved) = request(&app, "alice", "POST", "/decks", payload).await;
        assert_eq!(status, StatusCode::CREATED, "{saved}");
        assert_eq!(
            start_game(&app, draft.clone()).await,
            StatusCode::BAD_REQUEST
        );
        let path = format!("/decks/{}", saved["id"].as_str().unwrap());
        let (status, loaded) = request(&app, "alice", "GET", &path, serde_json::Value::Null).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(loaded, saved);
        assert_eq!(
            loaded["starting"],
            serde_json::to_value(&draft.deck_data.starting).unwrap()
        );
        let mut updated = serde_json::to_value(input()).unwrap();
        updated["expectedVersion"] = 1.into();
        let (status, edited) = request(&app, "alice", "PUT", &path, updated).await;
        assert_eq!(status, StatusCode::OK, "{edited}");
        assert_eq!(edited["version"], 2);
    }
    assert_eq!(start_game(&app, input()).await, StatusCode::OK);
}

#[tokio::test]
async fn intentional_identical_creates_are_distinct_from_import_idempotency() {
    let app = AppState::in_memory();
    account(&app, "alice").await;
    let mut ids = HashSet::new();
    for _ in 0..3 {
        let payload = create_input();
        let (status, created) = request(&app, "alice", "POST", "/decks", payload.clone()).await;
        assert_eq!(status, StatusCode::CREATED);
        assert!(ids.insert(created["id"].as_str().unwrap().to_owned()));
        let (_, retry) = request(&app, "alice", "POST", "/decks", payload).await;
        assert_eq!(created["id"], retry["id"]);
    }
    let original = serde_json::to_value(input()).unwrap();
    let (_, first) = request(&app, "alice", "POST", "/decks/import", original.clone()).await;
    let (_, retry) = request(&app, "alice", "POST", "/decks/import", original).await;
    assert_eq!(first["id"], retry["id"]);
    assert!(!ids.contains(first["id"].as_str().unwrap()));
    assert_eq!(app.decks.list("alice").await.unwrap().len(), 4);
}

#[tokio::test]
async fn opaque_account_context_is_not_an_owner_or_authentication_credential() {
    let app = AppState::in_memory();
    account(&app, "alice").await;
    account(&app, "bob").await;
    let context = app.auth.account_context("alice");
    assert_ne!(context, "alice");
    assert_ne!(context, app.auth.account_context("bob"));
    let (_, me) = request(&app, "alice", "GET", "/auth/me", serde_json::Value::Null).await;
    assert_eq!(me["user"]["id"], context);
    assert!(me["user"]["publicId"].is_null());
    let (_, updated) = request(
        &app,
        "alice",
        "PATCH",
        "/auth/profile",
        serde_json::json!({"publicId":"alice_public"}),
    )
    .await;
    assert_eq!(updated["user"]["id"], context);
    assert_eq!(updated["user"]["publicId"], "alice_public");
    // Obtain a real signed cookie, then test without the cfg(test) x-user-id shortcut.
    let response = crate::routes::api(app.clone())
        .call(
            Request::builder()
                .method("POST")
                .uri("/auth/session")
                .header("x-user-id", "alice")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(body["userId"], context);
    for (session, header, expected) in [
        (Some(cookie.as_str()), Some(context.clone()), StatusCode::OK),
        (Some(cookie.as_str()), None, StatusCode::CONFLICT),
        (
            Some(cookie.as_str()),
            Some("alice".into()),
            StatusCode::CONFLICT,
        ),
        (
            Some(cookie.as_str()),
            Some(app.auth.account_context("bob")),
            StatusCode::CONFLICT,
        ),
        (None, Some(context.clone()), StatusCode::UNAUTHORIZED),
    ] {
        let mut req = Request::builder().method("GET").uri("/decks");
        if let Some(cookie) = session {
            req = req.header("cookie", cookie);
        }
        if let Some(header) = header {
            req = req.header("x-deck-account", header);
        }
        let response = crate::routes::api(app.clone())
            .call(req.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
    // The browser comparison value is not accepted as a session token either.
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        format!("deck_chess_session={context}").parse().unwrap(),
    );
    assert!(app.auth.authenticate(&headers).is_err());
}

fn size_fixture(custom_count: usize, placements: usize) -> DeckInput {
    // One owned logical piece may have many pinned historical versions; the
    // 100-logical-piece quota does not limit a deck to 100 version references.
    let refs: Vec<CustomRef> = (0..custom_count)
        .map(|index| CustomRef {
            id: "00000000-0000-4000-8000-000000000000".into(),
            version: i32::MAX as u32 - index as u32,
            content_hash: "fnv1a64:ffffffffffffffff".into(),
            exposed_piece_key: "x".repeat(64),
        })
        .collect();
    DeckInput {
        name: "😀".repeat(100),
        deck_data: DeckData {
            map_id: "standard-12x12".into(),
            board_size: 12,
            starting: (0..placements)
                .map(|index| Placement {
                    piece_type: refs[index % custom_count].key(),
                    square: Position {
                        file: (index % 12) as i32,
                        rank: (index / 12) as i32,
                    },
                })
                .collect(),
            pocket: refs
                .iter()
                .map(|r| (r.key(), 4096 / custom_count as u32))
                .collect(),
            custom_pieces: refs,
        },
    }
}

#[tokio::test]
async fn measured_request_sizes_fit_bounded_limit() {
    let app = AppState::in_memory();
    account(&app, "alice").await;
    for (label, mut deck, expected_bytes) in [
        ("normal", input(), 0),
        ("large", size_fixture(32, 36), 16778),
        ("maximum", size_fixture(256, 144), 106172),
    ] {
        deck.spec().unwrap();
        let mut body = serde_json::to_value(&deck).unwrap();
        body["requestId"] = "00000000-0000-4000-8000-000000000000".into();
        let bytes = serde_json::to_vec(&body).unwrap().len();
        println!("{label} CREATE request: {bytes} bytes");
        if expected_bytes > 0 {
            assert_eq!(bytes, expected_bytes);
        }
        assert!(bytes < MAX_DECK_BYTES);
        if label == "maximum" {
            assert!(bytes > 65_536);
            assert!(MAX_DECK_BYTES as f64 / bytes as f64 > 1.2);
        }
        // Missing owned fixtures may fail reference lookup, but never the body limit.
        assert_ne!(
            request(&app, "alice", "POST", "/decks", body).await.0,
            StatusCode::PAYLOAD_TOO_LARGE
        );
        let mut update = serde_json::to_value(deck).unwrap();
        update["expectedVersion"] = i64::MAX.into();
        println!(
            "{label} UPDATE request: {} bytes",
            serde_json::to_vec(&update).unwrap().len()
        );
        assert!(serde_json::to_vec(&update).unwrap().len() < MAX_DECK_BYTES);
    }
}

#[tokio::test]
async fn maximum_cardinality_deck_with_real_owned_versions_saves_through_api() {
    use brainfuck_chess_engine::{
        custom_pieces::CUSTOM_PIECE_SCRIPT_FORMAT, pieces::default_pieces::all_default_definitions,
    };
    let app = AppState::in_memory();
    account(&app, "alice").await;
    let exposed = "x".repeat(64);
    let mut definition = all_default_definitions()
        .into_iter()
        .find(|d| d.id == "knight")
        .unwrap();
    definition.id = exposed.clone();
    let raw = serde_json::json!({"format":CUSTOM_PIECE_SCRIPT_FORMAT,"definitions":[definition]})
        .to_string();
    let mut piece = serde_json::json!({
        "name":"Large fixture", "description":"", "score":7,
        "image":{"kind":"built_in","asset_key":"knight"},
        "raw_script":raw, "exposed_piece_key":exposed
    });
    let mut references = Vec::new();
    let mut path = "/custom-pieces".to_owned();
    for version in 1..=256 {
        if version > 1 {
            piece["expected_version"] = (version - 1).into();
        }
        let (status, saved) = request(
            &app,
            "alice",
            if version == 1 { "POST" } else { "PUT" },
            &path,
            piece.clone(),
        )
        .await;
        assert_eq!(
            status,
            if version == 1 {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            },
            "{saved}"
        );
        let id = saved["id"].as_str().unwrap().to_owned();
        path = format!("/custom-pieces/{id}");
        references.push(CustomRef {
            id,
            version,
            content_hash: saved["content_hash"].as_str().unwrap().into(),
            exposed_piece_key: exposed.clone(),
        });
    }
    let mut deck = size_fixture(256, 144);
    for (index, placement) in deck.deck_data.starting.iter_mut().enumerate() {
        placement.piece_type = references[index].key();
    }
    deck.deck_data.pocket = references.iter().map(|r| (r.key(), 16)).collect();
    deck.deck_data.custom_pieces = references;
    let mut payload = serde_json::to_value(&deck).unwrap();
    payload["requestId"] = Uuid::new_v4().to_string().into();
    let bytes = serde_json::to_vec(&payload).unwrap().len();
    println!("API-generated maximum-cardinality CREATE request: {bytes} bytes");
    assert!(bytes > 65_536 && bytes < MAX_DECK_BYTES);
    let (status, saved) = request(&app, "alice", "POST", "/decks", payload).await;
    assert_eq!(status, StatusCode::CREATED, "{saved}");
    assert_eq!(
        saved["starting"],
        serde_json::to_value(&deck.deck_data.starting).unwrap()
    );
    assert_eq!(
        saved["customPieces"],
        serde_json::to_value(&deck.deck_data.custom_pieces).unwrap()
    );
    let path = format!("/decks/{}", saved["id"].as_str().unwrap());
    let (status, loaded) = request(&app, "alice", "GET", &path, serde_json::Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(loaded, saved);
    let mut update = serde_json::to_value(deck).unwrap();
    update["expectedVersion"] = 1.into();
    update["name"] = "편집 후 저장".into();
    assert_eq!(
        request(&app, "alice", "PUT", &path, update).await.0,
        StatusCode::OK
    );
}
