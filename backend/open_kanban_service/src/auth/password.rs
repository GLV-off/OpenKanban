use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier},
};

use crate::error::AppError;

pub const MIN_PASSWORD_LENGTH: usize = 10;
pub const MAX_PASSWORD_LENGTH: usize = 128;

pub fn hash_password(password: &str) -> Result<String, AppError> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|error| AppError::Internal(anyhow::anyhow!("cannot hash password: {error}")))
}

pub fn verify_password(password: &str, password_hash: &str) -> bool {
    Argon2::default()
        .verify_password(password.as_bytes(), password_hash)
        .is_ok()
}

pub fn validate_password(password: &str) -> Result<(), AppError> {
    let length = password.chars().count();

    if length < MIN_PASSWORD_LENGTH {
        return Err(AppError::validation_field(
            "password",
            format!("password must be at least {MIN_PASSWORD_LENGTH} characters"),
        ));
    }
    if length > MAX_PASSWORD_LENGTH {
        return Err(AppError::validation_field(
            "password",
            format!("password must be at most {MAX_PASSWORD_LENGTH} characters"),
        ));
    }
    if password.trim().is_empty() {
        return Err(AppError::validation_field(
            "password",
            "password must not be blank",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSWORD: &str = "correct horse battery staple";

    #[test]
    fn hashes_and_verifies_passwords() {
        let hash = hash_password(PASSWORD).expect("hash");

        assert_ne!(hash, PASSWORD);
        assert!(verify_password(PASSWORD, &hash));
        assert!(!verify_password("wrong password", &hash));
    }

    #[test]
    fn validates_password_policy() {
        assert!(validate_password("short").is_err());
        assert!(validate_password("          ").is_err());
        assert!(validate_password(&"x".repeat(MAX_PASSWORD_LENGTH + 1)).is_err());
        assert!(validate_password(PASSWORD).is_ok());
    }
}
