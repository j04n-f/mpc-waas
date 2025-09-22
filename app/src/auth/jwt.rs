use actix_web::Error;
use actix_web::error::ErrorInternalServerError;
use actix_web::error::ErrorUnauthorized;
use anyhow::Result;
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::models::UserModel;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub jti: String,
    pub user_id: i32,
    pub username: String,
}

pub static JWT_SECRET: Lazy<String> = Lazy::new(|| {
    std::env::var("JWT_SECRET").unwrap_or_else(|_| "default_jwt_secret_for_development_only".into())
});

pub fn generate_claims(user: &UserModel) -> Claims {
    let expiration = Utc::now()
        .checked_add_signed(Duration::days(1))
        .unwrap()
        .timestamp() as usize;

    let iat = Utc::now().timestamp() as usize;
    let jti = Uuid::new_v4().to_string();

    Claims {
        sub: user.id.to_string(),
        exp: expiration,
        iat,
        jti,
        user_id: user.id,
        username: user.username.clone(),
    }
}

pub fn generate_token(claims: &Claims) -> Result<String, Error> {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(JWT_SECRET.as_bytes()),
    )
    .map_err(|e| {
        log::error!("Error generating token: {}", e);
        ErrorInternalServerError(e)
    })
}

pub async fn validate_token(token: &str) -> Result<TokenData<Claims>, Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| {
        log::debug!("JWT validation error: {}", e);
        ErrorUnauthorized::<String>("Invalid token".to_string())
    })?;

    Ok(token_data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_roundtrip() {
        let user = UserModel {
            id: 123,
            username: "testuser".to_string(),
            password: "hashed_password".to_string(),
            email: "test@example.com".to_string(),
            created_on: Some(chrono::DateTime::from_timestamp(1640995200, 0).unwrap()),
            updated_on: Some(chrono::DateTime::from_timestamp(1640995200, 0).unwrap()),
        };

        let original_claims = generate_claims(&user);

        let token = generate_token(&original_claims).unwrap();
        let validation_result = validate_token(&token).await.unwrap();

        let recovered_claims = validation_result.claims;

        assert_eq!(original_claims.sub, recovered_claims.sub);
        assert_eq!(original_claims.user_id, recovered_claims.user_id);
        assert_eq!(original_claims.username, recovered_claims.username);
        assert_eq!(original_claims.jti, recovered_claims.jti);
    }
}
