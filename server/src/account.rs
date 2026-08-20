use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::database::DataSchema;
use crate::stores::CustomPieceStore;

const RESERVED_PUBLIC_IDS: &[&str] = &[
    "admin",
    "administrator",
    "api",
    "deckchess",
    "mod",
    "moderator",
    "staff",
    "support",
    "system",
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UserProfile {
    pub(crate) id: String,
    pub(crate) public_id: Option<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) avatar_url: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AccountUpdateError {
    NotFound,
    PublicIdTaken,
    Unavailable,
}

#[derive(Clone, Debug)]
struct StoredUser {
    profile: UserProfile,
    registered: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct VerifiedIdentity {
    pub(crate) issuer: String,
    pub(crate) subject: String,
    pub(crate) provider: String,
    pub(crate) email: Option<String>,
    pub(crate) email_verified: bool,
    pub(crate) display_name: Option<String>,
    pub(crate) avatar_url: Option<String>,
}

pub(crate) enum LoginResult {
    Complete {
        user: UserProfile,
        imported_guest_data: bool,
    },
    ImportRequired,
}

#[async_trait]
pub(crate) trait AccountRepository: Send + Sync {
    async fn ensure_guest(&self, user_id: &str) -> Result<(), &'static str>;
    async fn authenticated_user(&self, user_id: &str) -> Result<Option<UserProfile>, &'static str>;
    async fn update_profile(
        &self,
        user_id: &str,
        public_id: Option<&str>,
        display_name: Option<&str>,
    ) -> Result<UserProfile, AccountUpdateError>;
    async fn complete_google_login(
        &self,
        current_user_id: &str,
        identity: &VerifiedIdentity,
        import_guest_data: Option<bool>,
    ) -> Result<LoginResult, &'static str>;
}

pub(crate) struct InMemoryAccountRepository {
    users: RwLock<HashMap<String, StoredUser>>,
    identities: RwLock<HashMap<(String, String), String>>,
    custom_pieces: CustomPieceStore,
}

impl InMemoryAccountRepository {
    pub(crate) fn new(custom_pieces: CustomPieceStore) -> Self {
        Self {
            users: RwLock::new(HashMap::new()),
            identities: RwLock::new(HashMap::new()),
            custom_pieces,
        }
    }
}

#[async_trait]
impl AccountRepository for InMemoryAccountRepository {
    async fn ensure_guest(&self, user_id: &str) -> Result<(), &'static str> {
        self.users
            .write()
            .map_err(|_| "unavailable")?
            .entry(user_id.to_owned())
            .or_insert_with(|| StoredUser {
                profile: UserProfile {
                    id: user_id.to_owned(),
                    public_id: None,
                    display_name: None,
                    avatar_url: None,
                },
                registered: false,
            });
        Ok(())
    }

    async fn authenticated_user(&self, user_id: &str) -> Result<Option<UserProfile>, &'static str> {
        Ok(self
            .users
            .read()
            .map_err(|_| "unavailable")?
            .get(user_id)
            .filter(|user| user.registered)
            .map(|user| user.profile.clone()))
    }

    async fn update_profile(
        &self,
        user_id: &str,
        public_id: Option<&str>,
        display_name: Option<&str>,
    ) -> Result<UserProfile, AccountUpdateError> {
        let mut users = self
            .users
            .write()
            .map_err(|_| AccountUpdateError::Unavailable)?;
        if public_id.is_some_and(|public_id| {
            users.iter().any(|(id, user)| {
                id != user_id && user.profile.public_id.as_deref() == Some(public_id)
            })
        }) {
            return Err(AccountUpdateError::PublicIdTaken);
        }
        let user = users
            .get_mut(user_id)
            .filter(|user| user.registered)
            .ok_or(AccountUpdateError::NotFound)?;
        if let Some(public_id) = public_id {
            user.profile.public_id = Some(public_id.to_owned());
        }
        if let Some(display_name) = display_name {
            user.profile.display_name = Some(display_name.to_owned());
        }
        Ok(user.profile.clone())
    }

    async fn complete_google_login(
        &self,
        current_user_id: &str,
        identity: &VerifiedIdentity,
        import_guest_data: Option<bool>,
    ) -> Result<LoginResult, &'static str> {
        self.ensure_guest(current_user_id).await?;
        let identity_key = (identity.issuer.clone(), identity.subject.clone());
        let target = self
            .identities
            .read()
            .map_err(|_| "unavailable")?
            .get(&identity_key)
            .cloned();

        let target_id = target.unwrap_or_else(|| current_user_id.to_owned());
        let source_is_guest = !self
            .users
            .read()
            .map_err(|_| "unavailable")?
            .get(current_user_id)
            .is_some_and(|user| user.registered);
        let needs_import = target_id != current_user_id
            && source_is_guest
            && self.custom_pieces.has_owned_data(current_user_id).await?;
        if needs_import && import_guest_data.is_none() {
            return Ok(LoginResult::ImportRequired);
        }
        let imported = needs_import && import_guest_data == Some(true);
        if imported {
            self.custom_pieces
                .transfer_owner(current_user_id, &target_id)
                .await?;
        }

        {
            let mut users = self.users.write().map_err(|_| "unavailable")?;
            let user = users
                .entry(target_id.clone())
                .or_insert_with(|| StoredUser {
                    profile: UserProfile {
                        id: target_id.clone(),
                        public_id: None,
                        display_name: None,
                        avatar_url: None,
                    },
                    registered: true,
                });
            user.registered = true;
            if user.profile.display_name.is_none() {
                user.profile.display_name = identity.display_name.clone();
            }
            user.profile.avatar_url = identity.avatar_url.clone();
        }
        self.identities
            .write()
            .map_err(|_| "unavailable")?
            .entry(identity_key)
            .or_insert_with(|| target_id.clone());
        let user = self
            .users
            .read()
            .map_err(|_| "unavailable")?
            .get(&target_id)
            .ok_or("unavailable")?
            .profile
            .clone();
        Ok(LoginResult::Complete {
            user,
            imported_guest_data: imported,
        })
    }
}

pub(crate) struct PostgresAccountRepository {
    pool: PgPool,
    data_schema: DataSchema,
}

impl PostgresAccountRepository {
    pub(crate) fn new(pool: PgPool, data_schema: DataSchema) -> Self {
        Self { pool, data_schema }
    }
}

fn timestamp() -> Result<i64, &'static str> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "unavailable")?
            .as_secs(),
    )
    .map_err(|_| "unavailable")
}

#[async_trait]
impl AccountRepository for PostgresAccountRepository {
    async fn ensure_guest(&self, user_id: &str) -> Result<(), &'static str> {
        let now = timestamp()?;
        sqlx::query(
            "INSERT INTO shared.users (id, account_kind, status, created_at, updated_at) \
             VALUES ($1, 'guest', 'active', $2, $2) ON CONFLICT (id) DO NOTHING",
        )
        .bind(user_id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|_| "unavailable")?;
        Ok(())
    }

    async fn authenticated_user(&self, user_id: &str) -> Result<Option<UserProfile>, &'static str> {
        let row = sqlx::query(
            "SELECT id, public_id, display_name, avatar_url FROM shared.users \
             WHERE id = $1 AND account_kind = 'registered' AND status = 'active'",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| "unavailable")?;
        row.map(profile_from_row).transpose()
    }

    async fn update_profile(
        &self,
        user_id: &str,
        public_id: Option<&str>,
        display_name: Option<&str>,
    ) -> Result<UserProfile, AccountUpdateError> {
        let now = timestamp().map_err(|_| AccountUpdateError::Unavailable)?;
        let row = sqlx::query(
            "UPDATE shared.users SET public_id = COALESCE($2, public_id), \
             display_name = COALESCE($3, display_name), updated_at = $4 \
             WHERE id = $1 AND account_kind = 'registered' AND status = 'active' \
             RETURNING id, public_id, display_name, avatar_url",
        )
        .bind(user_id)
        .bind(public_id)
        .bind(display_name)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| {
            if error
                .as_database_error()
                .and_then(|database_error| database_error.code())
                .as_deref()
                == Some("23505")
            {
                AccountUpdateError::PublicIdTaken
            } else {
                AccountUpdateError::Unavailable
            }
        })?;
        profile_from_row(row.ok_or(AccountUpdateError::NotFound)?)
            .map_err(|_| AccountUpdateError::Unavailable)
    }

    async fn complete_google_login(
        &self,
        current_user_id: &str,
        identity: &VerifiedIdentity,
        import_guest_data: Option<bool>,
    ) -> Result<LoginResult, &'static str> {
        let now = timestamp()?;
        let mut tx = self.pool.begin().await.map_err(|_| "unavailable")?;
        sqlx::query(
            "INSERT INTO shared.users (id, account_kind, status, created_at, updated_at) \
             VALUES ($1, 'guest', 'active', $2, $2) ON CONFLICT (id) DO NOTHING",
        )
        .bind(current_user_id)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|_| "unavailable")?;

        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("{}\n{}", identity.issuer, identity.subject))
            .execute(&mut *tx)
            .await
            .map_err(|_| "unavailable")?;

        let current_kind = sqlx::query_scalar::<_, String>(
            "SELECT account_kind FROM shared.users WHERE id = $1 FOR UPDATE",
        )
        .bind(current_user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| "unavailable")?;
        let target_id = sqlx::query_scalar::<_, String>(
            "SELECT user_id FROM shared.auth_identities WHERE issuer = $1 AND subject = $2 FOR UPDATE",
        )
        .bind(&identity.issuer)
        .bind(&identity.subject)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| "unavailable")?
        .unwrap_or_else(|| current_user_id.to_owned());

        let needs_import = if target_id != current_user_id && current_kind == "guest" {
            let versions = self.data_schema.table("custom_piece_versions");
            let images = self.data_schema.table("custom_piece_images");
            sqlx::query_scalar::<_, bool>(&format!(
                "SELECT EXISTS (SELECT 1 FROM {versions} WHERE owner_id = $1) \
                 OR EXISTS (SELECT 1 FROM {images} WHERE owner_id = $1)"
            ))
            .bind(current_user_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| "unavailable")?
        } else {
            false
        };
        if needs_import && import_guest_data.is_none() {
            return Ok(LoginResult::ImportRequired);
        }
        let imported = needs_import && import_guest_data == Some(true);
        if imported {
            let versions = self.data_schema.table("custom_piece_versions");
            sqlx::query(&format!(
                "UPDATE {versions} SET owner_id = $1 WHERE owner_id = $2"
            ))
            .bind(&target_id)
            .bind(current_user_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| "unavailable")?;
            let images = self.data_schema.table("custom_piece_images");
            sqlx::query(&format!(
                "UPDATE {images} SET owner_id = $1 WHERE owner_id = $2"
            ))
            .bind(&target_id)
            .bind(current_user_id)
            .execute(&mut *tx)
            .await
            .map_err(|_| "unavailable")?;
        }

        sqlx::query(
            "UPDATE shared.users SET account_kind = 'registered', \
             display_name = COALESCE(display_name, $2), avatar_url = $3, \
             updated_at = $4 WHERE id = $1 AND status = 'active'",
        )
        .bind(&target_id)
        .bind(&identity.display_name)
        .bind(&identity.avatar_url)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|_| "unavailable")?;

        sqlx::query(
            "INSERT INTO shared.auth_identities \
             (id, user_id, issuer, subject, provider, email, email_verified, created_at, updated_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$8) \
             ON CONFLICT (issuer, subject) DO UPDATE SET email = EXCLUDED.email, \
             email_verified = EXCLUDED.email_verified, updated_at = EXCLUDED.updated_at",
        )
        .bind(Uuid::new_v4())
        .bind(&target_id)
        .bind(&identity.issuer)
        .bind(&identity.subject)
        .bind(&identity.provider)
        .bind(&identity.email)
        .bind(identity.email_verified)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|_| "unavailable")?;

        let row = sqlx::query(
            "SELECT id, public_id, display_name, avatar_url FROM shared.users WHERE id = $1",
        )
        .bind(&target_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| "unavailable")?;
        let user = profile_from_row(row)?;
        tx.commit().await.map_err(|_| "unavailable")?;
        Ok(LoginResult::Complete {
            user,
            imported_guest_data: imported,
        })
    }
}

fn profile_from_row(row: sqlx::postgres::PgRow) -> Result<UserProfile, &'static str> {
    Ok(UserProfile {
        id: row.try_get("id").map_err(|_| "unavailable")?,
        public_id: row.try_get("public_id").map_err(|_| "unavailable")?,
        display_name: row.try_get("display_name").map_err(|_| "unavailable")?,
        avatar_url: row.try_get("avatar_url").map_err(|_| "unavailable")?,
    })
}

pub(crate) fn normalize_public_id(value: &str) -> Result<String, ()> {
    let normalized = value.trim().to_ascii_lowercase();
    let bytes = normalized.as_bytes();
    if !(3..=20).contains(&bytes.len())
        || !bytes[0].is_ascii_alphanumeric()
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
        || RESERVED_PUBLIC_IDS.contains(&normalized.as_str())
    {
        return Err(());
    }
    Ok(normalized)
}

pub(crate) fn normalize_display_name(value: &str) -> Result<String, ()> {
    let normalized = value.trim();
    if !(1..=30).contains(&normalized.chars().count()) || normalized.chars().any(char::is_control) {
        return Err(());
    }
    Ok(normalized.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_piece::{CustomPieceRepository, InMemoryCustomPieceRepository};
    use std::sync::Arc;

    fn google(subject: &str) -> VerifiedIdentity {
        VerifiedIdentity {
            issuer: "https://securetoken.google.com/test-project".into(),
            subject: subject.into(),
            provider: "google".into(),
            email: Some("verified@example.com".into()),
            email_verified: true,
            display_name: Some("Deck Player".into()),
            avatar_url: Some("https://example.com/avatar.png".into()),
        }
    }

    async fn seed_postgres_guest_data(pool: &PgPool, schema: DataSchema, owner: &str, tag: &str) {
        let images = schema.table("custom_piece_images");
        sqlx::query(&format!(
            "INSERT INTO {images} \
             (asset_id, owner_id, media_type, width, height, content_hash, bytes) \
             VALUES ($1,$2,'image/png',1,1,$1,'\\x01')"
        ))
        .bind(format!("guest-image-{tag}"))
        .bind(owner)
        .execute(pool)
        .await
        .unwrap();

        let versions = schema.table("custom_piece_versions");
        sqlx::query(&format!(
            "INSERT INTO {versions} \
             (piece_id,version,owner_id,name,description,score,image_kind,image_value,raw_script, \
              exposed_piece_key,internal_piece_keys,validation_status,content_hash,package, \
              created_at,updated_at,active) \
             VALUES ($1,1,$2,'Guest Piece','',1,'built_in','pawn-white','move(1,0);', \
                     'guest-piece','[]','valid',$1,'{{}}',1,1,TRUE)"
        ))
        .bind(format!("guest-piece-{tag}"))
        .bind(owner)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn owned_environment_rows(pool: &PgPool, schema: DataSchema, owner: &str) -> i64 {
        let versions = schema.table("custom_piece_versions");
        let images = schema.table("custom_piece_images");
        sqlx::query_scalar::<_, i64>(&format!(
            "SELECT (SELECT count(*) FROM {versions} WHERE owner_id=$1) \
                  + (SELECT count(*) FROM {images} WHERE owner_id=$1)"
        ))
        .bind(owner)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn first_login_upgrades_the_existing_guest_id() {
        let custom = Arc::new(InMemoryCustomPieceRepository::default());
        custom.seed_owner_for_account_test("guest-a");
        let repository = InMemoryAccountRepository::new(custom.clone());
        let result = repository
            .complete_google_login("guest-a", &google("google-a"), None)
            .await
            .unwrap();
        let LoginResult::Complete { user, .. } = result else {
            panic!("login should complete")
        };
        assert_eq!(user.id, "guest-a");
        assert!(custom.has_owned_data("guest-a").await.unwrap());
        assert!(repository
            .authenticated_user("guest-a")
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn existing_account_requires_opt_in_and_imports_only_when_selected() {
        let custom = Arc::new(InMemoryCustomPieceRepository::default());
        let repository = InMemoryAccountRepository::new(custom.clone());
        repository
            .complete_google_login("account-a", &google("google-a"), None)
            .await
            .unwrap();

        custom.seed_owner_for_account_test("guest-b");
        assert!(matches!(
            repository
                .complete_google_login("guest-b", &google("google-a"), None)
                .await
                .unwrap(),
            LoginResult::ImportRequired
        ));
        let result = repository
            .complete_google_login("guest-b", &google("google-a"), Some(false))
            .await
            .unwrap();
        let LoginResult::Complete {
            user,
            imported_guest_data,
        } = result
        else {
            panic!("login should complete")
        };
        assert_eq!(user.id, "account-a");
        assert!(!imported_guest_data);
        assert!(custom.has_owned_data("guest-b").await.unwrap());

        custom.seed_owner_for_account_test("guest-c");
        let result = repository
            .complete_google_login("guest-c", &google("google-a"), Some(true))
            .await
            .unwrap();
        let LoginResult::Complete {
            imported_guest_data,
            ..
        } = result
        else {
            panic!("login should complete")
        };
        assert!(imported_guest_data);
        assert!(!custom.has_owned_data("guest-c").await.unwrap());
        assert!(custom.has_owned_data("account-a").await.unwrap());
    }

    #[tokio::test]
    async fn public_id_is_unique_and_does_not_replace_the_internal_id() {
        let custom = Arc::new(InMemoryCustomPieceRepository::default());
        let repository = InMemoryAccountRepository::new(custom);
        repository
            .complete_google_login("account-a", &google("google-a"), None)
            .await
            .unwrap();
        repository
            .complete_google_login("account-b", &google("google-b"), None)
            .await
            .unwrap();

        let updated = repository
            .update_profile("account-a", Some("deck_player"), None)
            .await
            .unwrap();
        assert_eq!(updated.id, "account-a");
        assert_eq!(updated.public_id.as_deref(), Some("deck_player"));
        assert!(matches!(
            repository
                .update_profile("account-b", Some("deck_player"), None)
                .await,
            Err(AccountUpdateError::PublicIdTaken)
        ));
    }

    #[test]
    fn public_id_validation_normalizes_and_rejects_unsafe_values() {
        assert_eq!(
            normalize_public_id("  Deck_Player  ").unwrap(),
            "deck_player"
        );
        for invalid in [
            "ab",
            "_player",
            "player-name",
            "한글id",
            "player name",
            "admin",
            "support",
        ] {
            assert!(
                normalize_public_id(invalid).is_err(),
                "expected invalid: {invalid}"
            );
        }
    }

    #[tokio::test]
    async fn edited_display_name_survives_a_later_google_login() {
        let custom = Arc::new(InMemoryCustomPieceRepository::default());
        let repository = InMemoryAccountRepository::new(custom);
        repository
            .complete_google_login("account-a", &google("google-a"), None)
            .await
            .unwrap();
        repository
            .update_profile("account-a", None, Some("새 닉네임"))
            .await
            .unwrap();

        let mut changed_google = google("google-a");
        changed_google.display_name = Some("Changed Google Name".into());
        repository
            .complete_google_login("guest-b", &changed_google, Some(false))
            .await
            .unwrap();

        let profile = repository
            .authenticated_user("account-a")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(profile.display_name.as_deref(), Some("새 닉네임"));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL with the split schema migration applied"]
    async fn postgres_identity_and_profile_are_shared_between_prod_and_test() {
        let database_url = std::env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL is required for this ignored integration test");
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await
            .unwrap();
        let prod = PostgresAccountRepository::new(pool.clone(), DataSchema::Prod);
        let test = PostgresAccountRepository::new(pool.clone(), DataSchema::Test);
        let suffix = Uuid::new_v4().to_string();
        let prod_guest = format!("prod-login-{suffix}");
        let test_guest = format!("test-login-{suffix}");
        let identity = google(&format!("google-{suffix}"));

        let LoginResult::Complete { user: first, .. } = prod
            .complete_google_login(&prod_guest, &identity, None)
            .await
            .unwrap()
        else {
            panic!("first login should complete")
        };
        let LoginResult::Complete { user: second, .. } = test
            .complete_google_login(&test_guest, &identity, Some(false))
            .await
            .unwrap()
        else {
            panic!("second login should complete")
        };
        assert_eq!(first.id, second.id);

        prod.update_profile(&first.id, None, Some("공유 닉네임"))
            .await
            .unwrap();
        assert_eq!(
            test.authenticated_user(&first.id)
                .await
                .unwrap()
                .unwrap()
                .display_name
                .as_deref(),
            Some("공유 닉네임")
        );

        sqlx::query("DELETE FROM shared.auth_identities WHERE issuer = $1 AND subject = $2")
            .bind(&identity.issuer)
            .bind(&identity.subject)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM shared.users WHERE id = ANY($1)")
            .bind(vec![first.id, test_guest])
            .execute(&pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL with the split schema migration applied"]
    async fn postgres_guest_import_moves_only_the_current_environment() {
        let database_url = std::env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL is required for this ignored integration test");
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await
            .unwrap();
        let prod = PostgresAccountRepository::new(pool.clone(), DataSchema::Prod);
        let test = PostgresAccountRepository::new(pool.clone(), DataSchema::Test);
        let suffix = Uuid::new_v4().to_string();

        for (repository, schema, label) in [
            (&prod, DataSchema::Prod, "prod"),
            (&test, DataSchema::Test, "test"),
        ] {
            let account_id = format!("import-account-{label}-{suffix}");
            let guest_id = format!("import-guest-{label}-{suffix}");
            let identity = google(&format!("import-google-{label}-{suffix}"));
            repository
                .complete_google_login(&account_id, &identity, None)
                .await
                .unwrap();
            repository.ensure_guest(&guest_id).await.unwrap();
            seed_postgres_guest_data(&pool, schema, &guest_id, &format!("{label}-{suffix}")).await;

            assert!(matches!(
                repository
                    .complete_google_login(&guest_id, &identity, None)
                    .await
                    .unwrap(),
                LoginResult::ImportRequired
            ));
            let LoginResult::Complete {
                user,
                imported_guest_data,
            } = repository
                .complete_google_login(&guest_id, &identity, Some(true))
                .await
                .unwrap()
            else {
                panic!("guest import should complete")
            };
            assert!(imported_guest_data);
            assert_eq!(user.id, account_id);
            assert_eq!(owned_environment_rows(&pool, schema, &guest_id).await, 0);
            assert_eq!(owned_environment_rows(&pool, schema, &account_id).await, 2);
            let opposite = match schema {
                DataSchema::Prod => DataSchema::Test,
                DataSchema::Test => DataSchema::Prod,
            };
            assert_eq!(
                owned_environment_rows(&pool, opposite, &account_id).await,
                0
            );
        }

        sqlx::query("DELETE FROM prod.custom_piece_versions WHERE piece_id LIKE $1")
            .bind(format!("guest-piece-prod-{suffix}"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM prod.custom_piece_images WHERE asset_id LIKE $1")
            .bind(format!("guest-image-prod-{suffix}"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM test.custom_piece_versions WHERE piece_id LIKE $1")
            .bind(format!("guest-piece-test-{suffix}"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM test.custom_piece_images WHERE asset_id LIKE $1")
            .bind(format!("guest-image-test-{suffix}"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM shared.auth_identities WHERE subject LIKE $1")
            .bind(format!("import-google-%-{suffix}"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM shared.users WHERE id LIKE $1")
            .bind(format!("import-%-{suffix}"))
            .execute(&pool)
            .await
            .unwrap();
    }

    #[test]
    fn display_name_validation_accepts_unicode_and_rejects_controls_or_excess_length() {
        assert_eq!(
            normalize_display_name("  새 닉네임  ").unwrap(),
            "새 닉네임"
        );
        assert!(normalize_display_name("").is_err());
        assert!(normalize_display_name("bad\nname").is_err());
        assert!(normalize_display_name(&"a".repeat(31)).is_err());
    }
}
