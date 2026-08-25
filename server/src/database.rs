use sqlx::PgPool;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DataSchema {
    Prod,
    Test,
}

pub(crate) async fn verify_database_contract(
    pool: &PgPool,
    app_env: &str,
    data_schema: DataSchema,
) -> Result<(), String> {
    let required_objects_exist = sqlx::query_scalar::<_, bool>(&format!(
        "SELECT to_regclass('shared.users') IS NOT NULL \
         AND to_regclass('shared.auth_identities') IS NOT NULL \
         AND to_regclass('{}.custom_piece_versions') IS NOT NULL \
         AND to_regclass('{}.custom_piece_images') IS NOT NULL \
         AND to_regclass('{}.game_records') IS NOT NULL \
         AND EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema='shared' AND table_name='users' AND column_name='profile_visibility') \
         AND EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema='{}' AND table_name='game_records' AND column_name='white_user_id') \
         AND EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema='{}' AND table_name='game_records' AND column_name='black_user_id')",
        data_schema.name(),
        data_schema.name(),
        data_schema.name(),
        data_schema.name(),
        data_schema.name()
    ))
    .fetch_one(pool)
    .await
    .map_err(|error| format!("failed to inspect PostgreSQL schema: {error}"))?;
    if !required_objects_exist {
        return Err(format!(
            "database schema is not provisioned for APP_ENV={app_env}; run the approved admin migration"
        ));
    }

    for required_schema in ["shared", data_schema.name()] {
        let can_use_required =
            sqlx::query_scalar::<_, bool>("SELECT has_schema_privilege(current_user, $1, 'USAGE')")
                .bind(required_schema)
                .fetch_one(pool)
                .await
                .map_err(|error| {
                    format!("failed to inspect PostgreSQL role permissions: {error}")
                })?;
        if !can_use_required {
            return Err(format!(
                "database role for APP_ENV={app_env} cannot use required schema {required_schema}"
            ));
        }
    }

    // Cloud deployments must use a role that cannot even resolve the opposite
    // environment schema. Local owners are exempt so isolated developer DBs remain usable.
    if matches!(app_env, "prod" | "test") {
        let forbidden_schema = match data_schema {
            DataSchema::Prod => "test",
            DataSchema::Test => "prod",
        };
        let can_use_forbidden =
            sqlx::query_scalar::<_, bool>("SELECT has_schema_privilege(current_user, $1, 'USAGE')")
                .bind(forbidden_schema)
                .fetch_one(pool)
                .await
                .map_err(|error| {
                    format!("failed to inspect PostgreSQL role permissions: {error}")
                })?;
        if can_use_forbidden {
            return Err(format!(
                "database role for APP_ENV={app_env} can access forbidden schema {forbidden_schema}"
            ));
        }
    }
    Ok(())
}

impl DataSchema {
    pub(crate) fn for_app_env(app_env: &str) -> Result<Self, String> {
        match app_env {
            "prod" => Ok(Self::Prod),
            // Local development must never default to production data.
            "test" | "local" => Ok(Self::Test),
            _ => Err(format!(
                "APP_ENV must be one of local, test, or prod; got {app_env:?}"
            )),
        }
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Prod => "prod",
            Self::Test => "test",
        }
    }

    pub(crate) fn table(self, table: &str) -> String {
        debug_assert!(matches!(
            table,
            "custom_piece_versions" | "custom_piece_images" | "game_records"
        ));
        format!("{}.{}", self.name(), table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_mapping_never_defaults_local_to_prod() {
        assert_eq!(DataSchema::for_app_env("prod"), Ok(DataSchema::Prod));
        assert_eq!(DataSchema::for_app_env("test"), Ok(DataSchema::Test));
        assert_eq!(DataSchema::for_app_env("local"), Ok(DataSchema::Test));
        assert!(DataSchema::for_app_env("production").is_err());
    }

    #[tokio::test]
    #[ignore = "requires disposable PostgreSQL admin/prod/test URLs"]
    async fn postgres_contract_rejects_opposite_schema_usage() {
        let admin_url =
            std::env::var("TEST_ADMIN_DATABASE_URL").expect("TEST_ADMIN_DATABASE_URL is required");
        let prod_url =
            std::env::var("TEST_PROD_DATABASE_URL").expect("TEST_PROD_DATABASE_URL is required");
        let test_url =
            std::env::var("TEST_APP_DATABASE_URL").expect("TEST_APP_DATABASE_URL is required");
        let admin = PgPool::connect(&admin_url).await.unwrap();
        let prod = PgPool::connect(&prod_url).await.unwrap();
        let test = PgPool::connect(&test_url).await.unwrap();

        verify_database_contract(&prod, "prod", DataSchema::Prod)
            .await
            .unwrap();
        verify_database_contract(&test, "test", DataSchema::Test)
            .await
            .unwrap();

        sqlx::query("GRANT USAGE ON SCHEMA prod TO deck_chess_test")
            .execute(&admin)
            .await
            .unwrap();
        let rejected = verify_database_contract(&test, "test", DataSchema::Test).await;
        let cleanup = sqlx::query("REVOKE USAGE ON SCHEMA prod FROM deck_chess_test")
            .execute(&admin)
            .await;
        cleanup.expect("temporary permission must be revoked");
        assert!(rejected
            .unwrap_err()
            .contains("can access forbidden schema prod"));

        verify_database_contract(&test, "test", DataSchema::Test)
            .await
            .unwrap();
    }
}
