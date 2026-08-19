use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::stores::CustomPieceStore;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UserProfile {
    pub(crate) id: String,
    pub(crate) display_name: Option<String>,
    pub(crate) avatar_url: Option<String>,
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
                        display_name: None,
                        avatar_url: None,
                    },
                    registered: true,
                });
            user.registered = true;
            user.profile.display_name = identity.display_name.clone();
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
}

impl PostgresAccountRepository {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
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
            "INSERT INTO users (id, account_kind, status, created_at, updated_at) \
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
            "SELECT id, display_name, avatar_url FROM users \
             WHERE id = $1 AND account_kind = 'registered' AND status = 'active'",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| "unavailable")?;
        row.map(profile_from_row).transpose()
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
            "INSERT INTO users (id, account_kind, status, created_at, updated_at) \
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
            "SELECT account_kind FROM users WHERE id = $1 FOR UPDATE",
        )
        .bind(current_user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| "unavailable")?;
        let target_id = sqlx::query_scalar::<_, String>(
            "SELECT user_id FROM auth_identities WHERE issuer = $1 AND subject = $2 FOR UPDATE",
        )
        .bind(&identity.issuer)
        .bind(&identity.subject)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| "unavailable")?
        .unwrap_or_else(|| current_user_id.to_owned());

        let needs_import = if target_id != current_user_id && current_kind == "guest" {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM custom_piece_versions WHERE owner_id = $1) \
                 OR EXISTS (SELECT 1 FROM custom_piece_images WHERE owner_id = $1)",
            )
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
            sqlx::query("UPDATE custom_piece_versions SET owner_id = $1 WHERE owner_id = $2")
                .bind(&target_id)
                .bind(current_user_id)
                .execute(&mut *tx)
                .await
                .map_err(|_| "unavailable")?;
            sqlx::query("UPDATE custom_piece_images SET owner_id = $1 WHERE owner_id = $2")
                .bind(&target_id)
                .bind(current_user_id)
                .execute(&mut *tx)
                .await
                .map_err(|_| "unavailable")?;
        }

        sqlx::query(
            "UPDATE users SET account_kind = 'registered', display_name = $2, avatar_url = $3, \
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
            "INSERT INTO auth_identities \
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

        let row = sqlx::query("SELECT id, display_name, avatar_url FROM users WHERE id = $1")
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
        display_name: row.try_get("display_name").map_err(|_| "unavailable")?,
        avatar_url: row.try_get("avatar_url").map_err(|_| "unavailable")?,
    })
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
}
