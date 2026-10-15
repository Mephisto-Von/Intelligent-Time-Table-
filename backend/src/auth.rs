use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn hash_password(password: &str) -> ApiResult<String> {
    use argon2::password_hash::PasswordHasher;
    use argon2::Argon2;
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| ApiError::Internal(format!("password hash error: {e}")))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    use argon2::password_hash::{phc::PasswordHash, PasswordVerifier};
    use argon2::Argon2;
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub acc: String,
    pub sch: String,
    pub role: String,
    pub exp: usize,
}

pub fn make_token(state: &AppState, user_id: &str, acc: &str, sch: &str, role: &str) -> ApiResult<String> {
    let exp = (chrono::Utc::now() + chrono::Duration::days(30)).timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        acc: acc.to_string(),
        sch: sch.to_string(),
        role: role.to_string(),
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    )
    .map_err(|e| ApiError::Internal(format!("token error: {e}")))
}

fn decode_token(state: &AppState, token: &str) -> Option<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .ok()
    .map(|d| d.claims)
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: String,
    pub account_id: String,
    pub school_id: String,
    pub role: String,
    pub name: String,
    pub email: String,
    pub teacher_id: Option<String>,
    pub class_id: Option<String>,
}

impl AuthUser {
    pub fn is_staff(&self) -> bool {
        self.role == "admin" || self.role == "scheduler"
    }
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
    pub fn require_staff(&self) -> ApiResult<()> {
        if self.is_staff() {
            Ok(())
        } else {
            Err(ApiError::Forbidden)
        }
    }
    pub fn require_admin(&self) -> ApiResult<()> {
        if self.is_admin() {
            Ok(())
        } else {
            Err(ApiError::Forbidden)
        }
    }
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "account_id": self.account_id,
            "school_id": self.school_id,
            "role": self.role,
            "name": self.name,
            "email": self.email,
            "teacher_id": self.teacher_id,
            "class_id": self.class_id,
        })
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let token = header.strip_prefix("Bearer ").ok_or(ApiError::Unauthorized)?;
        let claims = decode_token(state, token).ok_or(ApiError::Unauthorized)?;
        let row = sqlx::query(
            "SELECT id, account_id, school_id, email, name, role, teacher_id, class_id FROM users WHERE id = ?",
        )
        .bind(&claims.sub)
        .fetch_optional(&state.pool)
        .await?;
        let r = row.ok_or(ApiError::Unauthorized)?;
        Ok(AuthUser {
            id: r.get("id"),
            account_id: r.get("account_id"),
            school_id: r.get("school_id"),
            role: r.get("role"),
            name: r.get("name"),
            email: r.get("email"),
            teacher_id: r.get::<Option<String>, _>("teacher_id"),
            class_id: r.get::<Option<String>, _>("class_id"),
        })
    }
}
