use std::sync::Arc;

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use async_trait::async_trait;
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::auth::{
    AccountStatus, ClassGroup, CreateClassInput, CreateUserInput, PasswordValidationError,
    UserIdentity, UserRole, validate_new_password,
};

#[derive(Debug, Clone)]
pub struct StoredUser {
    pub identity: UserIdentity,
    pub password_hash: Option<String>,
    pub locked: bool,
}

#[derive(Debug, Error, Clone)]
pub enum AuthStoreError {
    #[error("database operation failed: {0}")]
    Database(String),
    #[error("resource already exists: {0}")]
    Conflict(String),
    #[error("invalid account data: {0}")]
    InvalidData(String),
}

#[async_trait]
pub trait AuthStore: Send + Sync {
    async fn find_user_by_username(
        &self,
        username: &str,
    ) -> Result<Option<StoredUser>, AuthStoreError>;

    async fn find_user_by_session(
        &self,
        token_hash: &str,
    ) -> Result<Option<StoredUser>, AuthStoreError>;

    async fn create_session(
        &self,
        user_id: i64,
        token_hash: &str,
        ttl_seconds: i64,
    ) -> Result<(), AuthStoreError>;

    async fn revoke_session(&self, token_hash: &str) -> Result<(), AuthStoreError>;

    async fn record_login(&self, user_id: i64) -> Result<(), AuthStoreError>;

    async fn record_failed_login(&self, user_id: i64) -> Result<(), AuthStoreError>;

    async fn update_password(
        &self,
        user_id: i64,
        password_hash: &str,
        actor_user_id: Option<i64>,
        action: &str,
    ) -> Result<(), AuthStoreError>;

    async fn create_user(
        &self,
        actor_user_id: i64,
        input: &CreateUserInput,
        password_hash: &str,
    ) -> Result<UserIdentity, AuthStoreError>;

    async fn list_users(&self) -> Result<Vec<UserIdentity>, AuthStoreError>;

    async fn list_classes(&self) -> Result<Vec<ClassGroup>, AuthStoreError>;

    async fn find_class(&self, class_id: i64) -> Result<Option<ClassGroup>, AuthStoreError>;

    async fn create_class(
        &self,
        actor_user_id: i64,
        input: &CreateClassInput,
    ) -> Result<ClassGroup, AuthStoreError>;

    async fn get_user(&self, user_id: i64) -> Result<Option<UserIdentity>, AuthStoreError>;

    async fn count_active_admins(&self) -> Result<u64, AuthStoreError>;

    async fn update_status(
        &self,
        actor_user_id: i64,
        user_id: i64,
        status: AccountStatus,
    ) -> Result<UserIdentity, AuthStoreError>;

    async fn reset_password(
        &self,
        actor_user_id: i64,
        user_id: i64,
        password_hash: &str,
    ) -> Result<UserIdentity, AuthStoreError>;

    async fn update_role(
        &self,
        actor_user_id: i64,
        user_id: i64,
        role: UserRole,
    ) -> Result<UserIdentity, AuthStoreError>;

    async fn ensure_bootstrap_admin(
        &self,
        username: &str,
        display_name: &str,
        password_hash: &str,
    ) -> Result<(), AuthStoreError>;
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("account is disabled")]
    AccountDisabled,
    #[error("account is temporarily locked")]
    AccountLocked,
    #[error("authentication is required")]
    AuthenticationRequired,
    #[error("password change is required")]
    PasswordChangeRequired,
    #[error("user is not allowed to perform this action")]
    Forbidden,
    #[error("user was not found")]
    UserNotFound,
    #[error("username is invalid: {0}")]
    InvalidUsername(String),
    #[error("display name must not be empty")]
    InvalidDisplayName,
    #[error("username is already in use")]
    UsernameTaken,
    #[error("class name must not be empty")]
    InvalidClassName,
    #[error("class name is already in use")]
    ClassNameTaken,
    #[error("class was not found")]
    ClassNotFound,
    #[error("cannot disable or demote the last active administrator")]
    CannotModifyLastAdmin,
    #[error(transparent)]
    InvalidPassword(#[from] PasswordValidationError),
    #[error("password hashing failed")]
    PasswordHashing,
    #[error(transparent)]
    Store(#[from] AuthStoreError),
}

#[derive(Debug, Clone)]
pub struct AuthenticatedSession {
    pub token: String,
    pub user: UserIdentity,
}

#[derive(Clone)]
pub struct AuthService {
    store: Arc<dyn AuthStore>,
    initial_password: Arc<str>,
    session_ttl_seconds: i64,
}

impl AuthService {
    pub fn new(store: Arc<dyn AuthStore>, initial_password: impl Into<Arc<str>>) -> Self {
        Self {
            store,
            initial_password: initial_password.into(),
            session_ttl_seconds: 12 * 60 * 60,
        }
    }

    pub fn with_session_ttl(mut self, session_ttl_seconds: i64) -> Self {
        self.session_ttl_seconds = session_ttl_seconds.max(60);
        self
    }

    pub fn initial_password(&self) -> &str {
        &self.initial_password
    }

    pub async fn login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<AuthenticatedSession, AuthError> {
        let username = normalize_username(username)?;
        let Some(user) = self.store.find_user_by_username(&username).await? else {
            return Err(AuthError::InvalidCredentials);
        };
        if user.identity.status == AccountStatus::Disabled {
            return Err(AuthError::AccountDisabled);
        }
        if user.locked {
            return Err(AuthError::AccountLocked);
        }
        let Some(password_hash) = user.password_hash.as_deref() else {
            return Err(AuthError::InvalidCredentials);
        };
        if !verify_password(password, password_hash) {
            self.store.record_failed_login(user.identity.id).await?;
            return Err(AuthError::InvalidCredentials);
        }

        let token = new_session_token();
        let token_hash = hash_session_token(&token);
        self.store
            .create_session(user.identity.id, &token_hash, self.session_ttl_seconds)
            .await?;
        self.store.record_login(user.identity.id).await?;

        Ok(AuthenticatedSession {
            token,
            user: user.identity,
        })
    }

    pub async fn authenticate(&self, token: &str) -> Result<UserIdentity, AuthError> {
        if token.trim().is_empty() {
            return Err(AuthError::AuthenticationRequired);
        }
        self.store
            .find_user_by_session(&hash_session_token(token))
            .await?
            .map(|user| user.identity)
            .ok_or(AuthError::AuthenticationRequired)
    }

    pub async fn logout(&self, token: &str) -> Result<(), AuthError> {
        if !token.trim().is_empty() {
            self.store
                .revoke_session(&hash_session_token(token))
                .await?;
        }
        Ok(())
    }

    pub async fn change_password(
        &self,
        token: &str,
        new_password: &str,
    ) -> Result<UserIdentity, AuthError> {
        let user = self.authenticate(token).await?;
        validate_new_password(&user.username, new_password, &self.initial_password)?;
        let password_hash = hash_password(new_password)?;
        self.store
            .update_password(user.id, &password_hash, Some(user.id), "password_changed")
            .await?;

        let mut identity = user;
        identity.must_change_password = false;
        Ok(identity)
    }

    pub async fn create_user(
        &self,
        actor: &UserIdentity,
        mut input: CreateUserInput,
    ) -> Result<UserIdentity, AuthError> {
        require_admin(actor)?;
        input.username = normalize_username(&input.username)?;
        if input.display_name.trim().is_empty() {
            return Err(AuthError::InvalidDisplayName);
        }
        if self.initial_password.as_ref() == input.username {
            return Err(AuthError::InvalidPassword(
                PasswordValidationError::SameAsUsername,
            ));
        }
        if let Some(class_id) = input.class_id {
            if self.store.find_class(class_id).await?.is_none() {
                return Err(AuthError::ClassNotFound);
            }
        }
        let password_hash = hash_password(&self.initial_password)?;
        self.store
            .create_user(actor.id, &input, &password_hash)
            .await
            .map_err(|error| match error {
                AuthStoreError::Conflict(_) => AuthError::UsernameTaken,
                other => AuthError::Store(other),
            })
    }

    pub async fn list_users(&self, actor: &UserIdentity) -> Result<Vec<UserIdentity>, AuthError> {
        require_admin(actor)?;
        Ok(self.store.list_users().await?)
    }

    pub async fn list_classes(&self, actor: &UserIdentity) -> Result<Vec<ClassGroup>, AuthError> {
        require_admin(actor)?;
        Ok(self.store.list_classes().await?)
    }

    pub async fn create_class(
        &self,
        actor: &UserIdentity,
        input: CreateClassInput,
    ) -> Result<ClassGroup, AuthError> {
        require_admin(actor)?;
        let name = input.name.trim();
        if name.is_empty() {
            return Err(AuthError::InvalidClassName);
        }
        let input = CreateClassInput {
            name: name.to_owned(),
        };
        self.store
            .create_class(actor.id, &input)
            .await
            .map_err(|error| match error {
                AuthStoreError::Conflict(_) => AuthError::ClassNameTaken,
                other => AuthError::Store(other),
            })
    }

    pub async fn set_status(
        &self,
        actor: &UserIdentity,
        user_id: i64,
        status: AccountStatus,
    ) -> Result<UserIdentity, AuthError> {
        require_admin(actor)?;
        let Some(target) = self.store.get_user(user_id).await? else {
            return Err(AuthError::UserNotFound);
        };
        if target.role == UserRole::Admin
            && target.status == AccountStatus::Active
            && status == AccountStatus::Disabled
            && self.store.count_active_admins().await? <= 1
        {
            return Err(AuthError::CannotModifyLastAdmin);
        }
        Ok(self.store.update_status(actor.id, user_id, status).await?)
    }

    pub async fn reset_password(
        &self,
        actor: &UserIdentity,
        user_id: i64,
    ) -> Result<UserIdentity, AuthError> {
        require_admin(actor)?;
        let password_hash = hash_password(&self.initial_password)?;
        Ok(self
            .store
            .reset_password(actor.id, user_id, &password_hash)
            .await?)
    }

    pub async fn update_role(
        &self,
        actor: &UserIdentity,
        user_id: i64,
        role: UserRole,
    ) -> Result<UserIdentity, AuthError> {
        require_admin(actor)?;
        let Some(target) = self.store.get_user(user_id).await? else {
            return Err(AuthError::UserNotFound);
        };
        if target.role == UserRole::Admin
            && role != UserRole::Admin
            && target.status == AccountStatus::Active
            && self.store.count_active_admins().await? <= 1
        {
            return Err(AuthError::CannotModifyLastAdmin);
        }
        Ok(self.store.update_role(actor.id, user_id, role).await?)
    }

    pub async fn ensure_bootstrap_admin(
        &self,
        username: &str,
        display_name: &str,
    ) -> Result<(), AuthError> {
        let username = normalize_username(username)?;
        if display_name.trim().is_empty() {
            return Err(AuthError::InvalidDisplayName);
        }
        let password_hash = hash_password(&self.initial_password)?;
        self.store
            .ensure_bootstrap_admin(&username, display_name, &password_hash)
            .await?;
        Ok(())
    }
}

pub fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AuthError::PasswordHashing)
}

fn verify_password(password: &str, password_hash: &str) -> bool {
    let Ok(parsed_hash) = PasswordHash::new(password_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

fn normalize_username(username: &str) -> Result<String, AuthError> {
    let normalized = username.trim();
    if normalized.is_empty()
        || normalized.chars().count() > 64
        || normalized.chars().any(char::is_whitespace)
    {
        return Err(AuthError::InvalidUsername(
            "must be 1-64 characters without whitespace".to_owned(),
        ));
    }
    Ok(normalized.to_owned())
}

fn require_admin(actor: &UserIdentity) -> Result<(), AuthError> {
    if actor.role == UserRole::Admin && actor.status == AccountStatus::Active {
        Ok(())
    } else {
        Err(AuthError::Forbidden)
    }
}

fn new_session_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    encode_hex(&bytes)
}

pub fn hash_session_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    encode_hex(&digest)
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}
