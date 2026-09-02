use async_trait::async_trait;
use sqlx::{FromRow, PgPool};

use crate::application::{AuthStore, AuthStoreError, StoredUser};
use crate::domain::auth::{
    AccountStatus, ClassGroup, CreateClassInput, CreateUserInput, UserIdentity, UserRole,
};

#[derive(Clone)]
pub struct PgAuthStore {
    pool: PgPool,
}

impl PgAuthStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(Debug, FromRow)]
struct ClassRow {
    id: i64,
    name: String,
    created_at: String,
}

#[derive(Debug, FromRow)]
struct UserRow {
    id: i64,
    username: String,
    display_name: String,
    role: String,
    status: String,
    must_change_password: bool,
    student_number: Option<String>,
    class_name: Option<String>,
    created_at: String,
    last_login_at: Option<String>,
    password_hash: Option<String>,
    locked: bool,
}

const USER_COLUMNS: &str = r#"
    users.id,
    users.username,
    users.display_name,
    users.role::text AS role,
    users.status::text AS status,
    users.must_change_password,
    users.student_number,
    users.class_name,
    to_char(users.created_at AT TIME ZONE 'Asia/Shanghai', 'YYYY-MM-DD"T"HH24:MI:SS.MS"+08:00"') AS created_at,
    to_char(users.last_login_at AT TIME ZONE 'Asia/Shanghai', 'YYYY-MM-DD"T"HH24:MI:SS.MS"+08:00"') AS last_login_at,
    users.password_hash,
    (users.locked_until IS NOT NULL AND users.locked_until > now()) AS locked
"#;

fn store_error(error: sqlx::Error) -> AuthStoreError {
    if let sqlx::Error::Database(database_error) = &error {
        if database_error.code().as_deref() == Some("23505") {
            return AuthStoreError::Conflict(database_error.message().to_owned());
        }
    }
    AuthStoreError::Database(error.to_string())
}

fn map_user(row: UserRow) -> Result<StoredUser, AuthStoreError> {
    let role = row
        .role
        .parse::<UserRole>()
        .map_err(|error| AuthStoreError::InvalidData(error.to_string()))?;
    let status = row
        .status
        .parse::<AccountStatus>()
        .map_err(|error| AuthStoreError::InvalidData(format!("invalid account status: {error}")))?;
    Ok(StoredUser {
        identity: UserIdentity {
            id: row.id,
            username: row.username,
            display_name: row.display_name,
            role,
            status,
            must_change_password: row.must_change_password,
            student_number: row.student_number,
            class_name: row.class_name,
            created_at: row.created_at,
            last_login_at: row.last_login_at,
        },
        password_hash: row.password_hash,
        locked: row.locked,
    })
}

fn insert_audit_sql() -> &'static str {
    r#"
INSERT INTO audit_logs (actor_user_id, action, entity_type, entity_id, details)
VALUES ($1, $2, 'user', $3, $4::jsonb)
"#
}

#[async_trait]
impl AuthStore for PgAuthStore {
    async fn find_user_by_username(
        &self,
        username: &str,
    ) -> Result<Option<StoredUser>, AuthStoreError> {
        let sql = format!(
            "SELECT {USER_COLUMNS} FROM users WHERE username = $1",
            USER_COLUMNS = USER_COLUMNS
        );
        sqlx::query_as::<_, UserRow>(&sql)
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(store_error)?
            .map(map_user)
            .transpose()
    }

    async fn find_user_by_session(
        &self,
        token_hash: &str,
    ) -> Result<Option<StoredUser>, AuthStoreError> {
        let sql = format!(
            "SELECT {USER_COLUMNS} \
             FROM user_sessions AS sessions \
             JOIN users ON users.id = sessions.user_id \
             WHERE sessions.token_hash = $1 \
               AND sessions.revoked_at IS NULL \
               AND sessions.expires_at > now() \
               AND users.status::text = 'active'",
            USER_COLUMNS = USER_COLUMNS
        );
        sqlx::query_as::<_, UserRow>(&sql)
            .bind(token_hash)
            .fetch_optional(&self.pool)
            .await
            .map_err(store_error)?
            .map(map_user)
            .transpose()
    }

    async fn create_session(
        &self,
        user_id: i64,
        token_hash: &str,
        ttl_seconds: i64,
    ) -> Result<(), AuthStoreError> {
        sqlx::query(
            r#"
INSERT INTO user_sessions (user_id, token_hash, expires_at)
VALUES ($1, $2, now() + ($3::double precision * interval '1 second'))
"#,
        )
        .bind(user_id)
        .bind(token_hash)
        .bind(ttl_seconds)
        .execute(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(())
    }

    async fn revoke_session(&self, token_hash: &str) -> Result<(), AuthStoreError> {
        sqlx::query(
            "UPDATE user_sessions SET revoked_at = now() WHERE token_hash = $1 AND revoked_at IS NULL",
        )
        .bind(token_hash)
        .execute(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(())
    }

    async fn record_login(&self, user_id: i64) -> Result<(), AuthStoreError> {
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        sqlx::query(
            "UPDATE users SET last_login_at = now(), failed_login_attempts = 0, locked_until = NULL WHERE id = $1",
        )
            .bind(user_id)
            .execute(&mut *transaction)
            .await
            .map_err(store_error)?;
        sqlx::query(insert_audit_sql())
            .bind(user_id)
            .bind("user_login")
            .bind(user_id)
            .bind("{}")
            .execute(&mut *transaction)
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        Ok(())
    }

    async fn record_failed_login(&self, user_id: i64) -> Result<(), AuthStoreError> {
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        sqlx::query(
            r#"
UPDATE users
SET failed_login_attempts = failed_login_attempts + 1,
    locked_until = CASE
        WHEN failed_login_attempts + 1 >= 5 THEN now() + interval '5 minutes'
        ELSE locked_until
    END
WHERE id = $1
"#,
        )
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .map_err(store_error)?;
        sqlx::query(insert_audit_sql())
            .bind(None::<i64>)
            .bind("user_login_failed")
            .bind(user_id)
            .bind("{}")
            .execute(&mut *transaction)
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        Ok(())
    }

    async fn update_password(
        &self,
        user_id: i64,
        password_hash: &str,
        actor_user_id: Option<i64>,
        action: &str,
    ) -> Result<(), AuthStoreError> {
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        let result = sqlx::query(
            r#"
UPDATE users
SET password_hash = $1,
    must_change_password = false,
    failed_login_attempts = 0,
    locked_until = NULL
WHERE id = $2
"#,
        )
        .bind(password_hash)
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .map_err(store_error)?;
        if result.rows_affected() == 0 {
            transaction.rollback().await.map_err(store_error)?;
            return Err(AuthStoreError::InvalidData("user was not found".to_owned()));
        }
        sqlx::query(insert_audit_sql())
            .bind(actor_user_id)
            .bind(action)
            .bind(user_id)
            .bind("{}")
            .execute(&mut *transaction)
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        Ok(())
    }

    async fn create_user(
        &self,
        actor_user_id: i64,
        input: &CreateUserInput,
        password_hash: &str,
    ) -> Result<UserIdentity, AuthStoreError> {
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        let user_id = sqlx::query_scalar::<_, i64>(
            r#"
INSERT INTO users (
    username,
    display_name,
    role,
    password_hash,
    must_change_password,
    status,
    student_number,
    class_id,
    class_name
)
VALUES (
    $1,
    $2,
    $3::user_role,
    $4,
    true,
    'active'::account_status,
    $5,
    $6,
    (SELECT name FROM classes WHERE id = $6)
)
RETURNING id
"#,
        )
        .bind(&input.username)
        .bind(&input.display_name)
        .bind(input.role.to_string())
        .bind(password_hash)
        .bind(&input.student_number)
        .bind(input.class_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(store_error)?;
        sqlx::query(insert_audit_sql())
            .bind(actor_user_id)
            .bind("user_created")
            .bind(user_id)
            .bind("{}")
            .execute(&mut *transaction)
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        self.get_user(user_id)
            .await?
            .ok_or_else(|| AuthStoreError::InvalidData("created user could not be read".to_owned()))
    }

    async fn list_users(&self) -> Result<Vec<UserIdentity>, AuthStoreError> {
        let sql = format!(
            "SELECT {USER_COLUMNS} FROM users ORDER BY id",
            USER_COLUMNS = USER_COLUMNS
        );
        let rows = sqlx::query_as::<_, UserRow>(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(store_error)?;
        rows.into_iter()
            .map(map_user)
            .map(|result| result.map(|user| user.identity))
            .collect()
    }

    async fn list_classes(&self) -> Result<Vec<ClassGroup>, AuthStoreError> {
        let rows = sqlx::query_as::<_, ClassRow>(
            r#"
SELECT
    id,
    name,
    to_char(created_at AT TIME ZONE 'Asia/Shanghai', 'YYYY-MM-DD"T"HH24:MI:SS.MS"+08:00"') AS created_at
FROM classes
ORDER BY name, id
"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(rows
            .into_iter()
            .map(|row| ClassGroup {
                id: row.id,
                name: row.name,
                created_at: row.created_at,
            })
            .collect())
    }

    async fn find_class(&self, class_id: i64) -> Result<Option<ClassGroup>, AuthStoreError> {
        sqlx::query_as::<_, ClassRow>(
            r#"
SELECT
    id,
    name,
    to_char(created_at AT TIME ZONE 'Asia/Shanghai', 'YYYY-MM-DD"T"HH24:MI:SS.MS"+08:00"') AS created_at
FROM classes
WHERE id = $1
"#,
        )
        .bind(class_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)
        .map(|row| {
            row.map(|row| ClassGroup {
                id: row.id,
                name: row.name,
                created_at: row.created_at,
            })
        })
    }

    async fn create_class(
        &self,
        actor_user_id: i64,
        input: &CreateClassInput,
    ) -> Result<ClassGroup, AuthStoreError> {
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        let row = sqlx::query_as::<_, ClassRow>(
            r#"
INSERT INTO classes (name)
VALUES ($1)
RETURNING
    id,
    name,
    to_char(created_at AT TIME ZONE 'Asia/Shanghai', 'YYYY-MM-DD"T"HH24:MI:SS.MS"+08:00"') AS created_at
"#,
        )
        .bind(&input.name)
        .fetch_one(&mut *transaction)
        .await
        .map_err(store_error)?;
        sqlx::query(
            r#"
INSERT INTO audit_logs (actor_user_id, action, entity_type, entity_id, details)
VALUES ($1, 'class_created', 'class', $2, '{}'::jsonb)
"#,
        )
        .bind(actor_user_id)
        .bind(row.id)
        .execute(&mut *transaction)
        .await
        .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        Ok(ClassGroup {
            id: row.id,
            name: row.name,
            created_at: row.created_at,
        })
    }

    async fn get_user(&self, user_id: i64) -> Result<Option<UserIdentity>, AuthStoreError> {
        let sql = format!(
            "SELECT {USER_COLUMNS} FROM users WHERE id = $1",
            USER_COLUMNS = USER_COLUMNS
        );
        sqlx::query_as::<_, UserRow>(&sql)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(store_error)?
            .map(map_user)
            .transpose()
            .map(|user| user.map(|user| user.identity))
    }

    async fn count_active_admins(&self) -> Result<u64, AuthStoreError> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM users WHERE role::text = 'admin' AND status::text = 'active'",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(store_error)?;
        u64::try_from(count)
            .map_err(|_| AuthStoreError::InvalidData("admin count was negative".to_owned()))
    }

    async fn update_status(
        &self,
        actor_user_id: i64,
        user_id: i64,
        status: AccountStatus,
    ) -> Result<UserIdentity, AuthStoreError> {
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        let result = sqlx::query("UPDATE users SET status = $1::account_status WHERE id = $2")
            .bind(status.to_string())
            .bind(user_id)
            .execute(&mut *transaction)
            .await
            .map_err(store_error)?;
        if result.rows_affected() == 0 {
            transaction.rollback().await.map_err(store_error)?;
            return Err(AuthStoreError::InvalidData("user was not found".to_owned()));
        }
        sqlx::query(insert_audit_sql())
            .bind(actor_user_id)
            .bind(match status {
                AccountStatus::Active => "user_enabled",
                AccountStatus::Disabled => "user_disabled",
            })
            .bind(user_id)
            .bind("{}")
            .execute(&mut *transaction)
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        self.get_user(user_id)
            .await?
            .ok_or_else(|| AuthStoreError::InvalidData("updated user could not be read".to_owned()))
    }

    async fn reset_password(
        &self,
        actor_user_id: i64,
        user_id: i64,
        password_hash: &str,
    ) -> Result<UserIdentity, AuthStoreError> {
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        let result = sqlx::query(
            r#"
UPDATE users
SET password_hash = $1,
    must_change_password = true,
    failed_login_attempts = 0,
    locked_until = NULL
WHERE id = $2
"#,
        )
        .bind(password_hash)
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .map_err(store_error)?;
        if result.rows_affected() == 0 {
            transaction.rollback().await.map_err(store_error)?;
            return Err(AuthStoreError::InvalidData("user was not found".to_owned()));
        }
        sqlx::query(
            "UPDATE user_sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL",
        )
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .map_err(store_error)?;
        sqlx::query(insert_audit_sql())
            .bind(actor_user_id)
            .bind("user_password_reset")
            .bind(user_id)
            .bind("{}")
            .execute(&mut *transaction)
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        self.get_user(user_id)
            .await?
            .ok_or_else(|| AuthStoreError::InvalidData("reset user could not be read".to_owned()))
    }

    async fn update_role(
        &self,
        actor_user_id: i64,
        user_id: i64,
        role: UserRole,
    ) -> Result<UserIdentity, AuthStoreError> {
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        let result = sqlx::query("UPDATE users SET role = $1::user_role WHERE id = $2")
            .bind(role.to_string())
            .bind(user_id)
            .execute(&mut *transaction)
            .await
            .map_err(store_error)?;
        if result.rows_affected() == 0 {
            transaction.rollback().await.map_err(store_error)?;
            return Err(AuthStoreError::InvalidData("user was not found".to_owned()));
        }
        sqlx::query(insert_audit_sql())
            .bind(actor_user_id)
            .bind("user_role_changed")
            .bind(user_id)
            .bind(format!(r#"{{"role":"{role}"}}"#))
            .execute(&mut *transaction)
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        self.get_user(user_id)
            .await?
            .ok_or_else(|| AuthStoreError::InvalidData("updated user could not be read".to_owned()))
    }

    async fn ensure_bootstrap_admin(
        &self,
        username: &str,
        display_name: &str,
        password_hash: &str,
    ) -> Result<(), AuthStoreError> {
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        let pending_admin = sqlx::query_as::<_, (i64, String)>(
            "SELECT id, username FROM users WHERE role::text = 'admin' AND password_hash IS NULL ORDER BY id LIMIT 1",
        )
        .fetch_optional(&mut *transaction)
        .await
        .map_err(store_error)?;

        if let Some((user_id, existing_username)) = pending_admin {
            let login_username = if existing_username.starts_with("legacy-") {
                username
            } else {
                &existing_username
            };
            sqlx::query(
                r#"
UPDATE users
SET username = $1,
    password_hash = $2,
    must_change_password = true,
    status = 'active'::account_status
WHERE id = $3
"#,
            )
            .bind(login_username)
            .bind(password_hash)
            .bind(user_id)
            .execute(&mut *transaction)
            .await
            .map_err(store_error)?;
            sqlx::query(insert_audit_sql())
                .bind(user_id)
                .bind("bootstrap_admin_initialized")
                .bind(user_id)
                .bind("{}")
                .execute(&mut *transaction)
                .await
                .map_err(store_error)?;
        } else {
            let has_admin = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM users WHERE role::text = 'admin')",
            )
            .fetch_one(&mut *transaction)
            .await
            .map_err(store_error)?;
            if !has_admin {
                let user_id = sqlx::query_scalar::<_, i64>(
                    r#"
INSERT INTO users (
    username, display_name, role, password_hash, must_change_password, status
)
VALUES ($1, $2, 'admin'::user_role, $3, true, 'active'::account_status)
RETURNING id
"#,
                )
                .bind(username)
                .bind(display_name)
                .bind(password_hash)
                .fetch_one(&mut *transaction)
                .await
                .map_err(store_error)?;
                sqlx::query(insert_audit_sql())
                    .bind(user_id)
                    .bind("bootstrap_admin_initialized")
                    .bind(user_id)
                    .bind("{}")
                    .execute(&mut *transaction)
                    .await
                    .map_err(store_error)?;
            }
        }
        transaction.commit().await.map_err(store_error)?;
        Ok(())
    }
}
