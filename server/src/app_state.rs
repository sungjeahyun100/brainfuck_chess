use crate::stores::{AccountStore, CustomPieceStore, GameStore, RoomStore};
use crate::{
    account::{InMemoryAccountRepository, PostgresAccountRepository},
    auth::AuthState,
    custom_piece::{InMemoryCustomPieceRepository, PostgresCustomPieceRepository},
};
use sqlx::postgres::PgPoolOptions;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) games: GameStore,
    pub(crate) rooms: RoomStore,
    pub(crate) custom_pieces: CustomPieceStore,
    pub(crate) accounts: AccountStore,
    pub(crate) auth: AuthState,
}

impl AppState {
    #[cfg(test)]
    pub(crate) fn in_memory() -> Self {
        let custom_pieces: CustomPieceStore =
            std::sync::Arc::new(InMemoryCustomPieceRepository::default());
        Self {
            games: Default::default(),
            rooms: Default::default(),
            accounts: std::sync::Arc::new(InMemoryAccountRepository::new(custom_pieces.clone())),
            custom_pieces,
            auth: AuthState::for_tests(),
        }
    }

    pub(crate) async fn from_env(app_env: &str) -> Result<Self, String> {
        let auth = AuthState::from_env(app_env)?;
        let (custom_pieces, accounts): (CustomPieceStore, AccountStore) =
            match std::env::var("DATABASE_URL") {
                Ok(database_url) => {
                    let pool = PgPoolOptions::new()
                        .max_connections(10)
                        .acquire_timeout(std::time::Duration::from_secs(5))
                        .connect(&database_url)
                        .await
                        .map_err(|error| format!("failed to connect to PostgreSQL: {error}"))?;
                    sqlx::migrate!("./migrations")
                        .run(&pool)
                        .await
                        .map_err(|error| format!("failed to run database migrations: {error}"))?;
                    (
                        std::sync::Arc::new(PostgresCustomPieceRepository::from_pool(pool.clone())),
                        std::sync::Arc::new(PostgresAccountRepository::new(pool)),
                    )
                }
                Err(_) if app_env != "prod" => {
                    eprintln!(
                        "DATABASE_URL is not set; using non-persistent local custom-piece storage"
                    );
                    let custom_pieces: CustomPieceStore =
                        std::sync::Arc::new(InMemoryCustomPieceRepository::default());
                    let accounts: AccountStore =
                        std::sync::Arc::new(InMemoryAccountRepository::new(custom_pieces.clone()));
                    (custom_pieces, accounts)
                }
                Err(_) => return Err("DATABASE_URL is required in production".into()),
            };
        Ok(Self {
            games: Default::default(),
            rooms: Default::default(),
            custom_pieces,
            accounts,
            auth,
        })
    }
}
