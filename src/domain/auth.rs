use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Default, Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    Admin,
    #[default]
    Student,
}

impl fmt::Display for UserRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Admin => "admin",
            Self::Student => "student",
        })
    }
}

#[derive(Debug, Error, Clone, Eq, PartialEq)]
#[error("invalid user role: {0}")]
pub struct InvalidUserRole(pub String);

impl FromStr for UserRole {
    type Err = InvalidUserRole;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "admin" => Ok(Self::Admin),
            "student" | "user" => Ok(Self::Student),
            other => Err(InvalidUserRole(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountStatus {
    Active,
    Disabled,
}

impl fmt::Display for AccountStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Active => "active",
            Self::Disabled => "disabled",
        })
    }
}

impl FromStr for AccountStatus {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            other => Err(other.to_owned()),
        }
    }
}

#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum PasswordValidationError {
    #[error("password must not be empty")]
    Empty,
    #[error("password must be at least 8 characters")]
    TooShort,
    #[error("password must contain both letters and numbers")]
    NotComplexEnough,
    #[error("password must not equal the username")]
    SameAsUsername,
    #[error("password must not equal the fixed initial password")]
    SameAsInitialPassword,
}

pub fn validate_new_password(
    username: &str,
    password: &str,
    initial_password: &str,
) -> Result<(), PasswordValidationError> {
    if password.trim().is_empty() {
        return Err(PasswordValidationError::Empty);
    }
    if password.chars().count() < 8 {
        return Err(PasswordValidationError::TooShort);
    }
    let has_letter = password.chars().any(char::is_alphabetic);
    let has_number = password.chars().any(char::is_numeric);
    if !has_letter || !has_number {
        return Err(PasswordValidationError::NotComplexEnough);
    }
    if password == username {
        return Err(PasswordValidationError::SameAsUsername);
    }
    if password == initial_password {
        return Err(PasswordValidationError::SameAsInitialPassword);
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct ClassGroup {
    pub id: i64,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct CreateClassInput {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct UserIdentity {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub role: UserRole,
    pub status: AccountStatus,
    pub must_change_password: bool,
    pub student_number: Option<String>,
    pub class_name: Option<String>,
    pub created_at: String,
    pub last_login_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct CreateUserInput {
    pub username: String,
    pub display_name: String,
    #[serde(default)]
    pub role: UserRole,
    pub student_number: Option<String>,
    pub class_id: Option<i64>,
}
