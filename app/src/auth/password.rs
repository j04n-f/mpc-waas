use actix_web::{Error, error::ErrorInternalServerError};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};

pub fn hash_password(password: &str) -> Result<String, Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| {
            log::error!("Error hashing password: {}", e);
            ErrorInternalServerError(e)
        })?
        .to_string();

    Ok(password_hash)
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, Error> {
    let parsed_hash = PasswordHash::new(password_hash).map_err(|e| {
        log::error!("Error parsing hash: {}", e);
        ErrorInternalServerError(e)
    })?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

#[cfg(test)]
pub mod tests {
    use super::{hash_password, verify_password};

    #[test]
    fn test_verify_password_correct() {
        let password = "correct_password";
        let hash = hash_password(password).unwrap();

        let verify_result = verify_password(password, &hash);
        assert!(verify_result.is_ok());
        assert!(verify_result.unwrap());
    }

    #[test]
    fn test_verify_password_incorrect() {
        let password = "correct_password";
        let wrong_password = "wrong_password";
        let hash = hash_password(password).unwrap();

        let verify_result = verify_password(wrong_password, &hash);
        assert!(verify_result.is_ok());
        assert!(!verify_result.unwrap());
    }
}
