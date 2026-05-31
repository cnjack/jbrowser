use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{Duration as ChronoDuration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};

use crate::server::error::AppError;
use crate::server::types::{JwtClaims, User};

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|err| anyhow::anyhow!(err.to_string()))?
        .to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<(), AppError> {
    let parsed =
        PasswordHash::new(hash).map_err(|_| AppError::unauthorized("invalid credentials"))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| AppError::unauthorized("invalid credentials"))
}

pub fn hash_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

pub fn generate_token(prefix: &str) -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("{}_{}", prefix, URL_SAFE_NO_PAD.encode(bytes))
}

pub fn issue_jwt(secret: &str, user: &User) -> Result<String, AppError> {
    let now = Utc::now();
    let claims = JwtClaims {
        sub: user.id.to_string(),
        email: user.email.clone(),
        tenants: user.tenants.clone(),
        iat: now.timestamp() as usize,
        exp: (now + ChronoDuration::hours(12)).timestamp() as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|err| AppError::internal(format!("failed to issue jwt: {err}")))
}

pub fn decode_jwt(secret: &str, token: &str) -> Result<JwtClaims, AppError> {
    decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| AppError::unauthorized("invalid bearer token"))
}
