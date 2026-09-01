use crate::auth::{hash_password, issue_token, verify_password, verify_token};
use crate::error::AppError;
use crate::models::{AuthResponse, LoginRequest, RegisterRequest, User};
use crate::AppState;
use axum::extract::State;
use axum::http::{header, HeaderMap};
use axum::Json;

const MIN_PASSWORD_LEN: usize = 8;
/// Postgres SQLSTATE for unique_violation.
const UNIQUE_VIOLATION: &str = "23505";

fn normalize_email(email: &str) -> Result<String, AppError> {
    let email = email.trim().to_lowercase();
    let valid = email
        .split_once('@')
        .is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.'));
    if !valid {
        return Err(AppError::BadRequest("invalid email address".into()));
    }
    Ok(email)
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let email = normalize_email(&req.email)?;
    if req.password.chars().count() < MIN_PASSWORD_LEN {
        return Err(AppError::BadRequest(format!(
            "password must be at least {MIN_PASSWORD_LEN} characters"
        )));
    }

    let password_hash = hash_password(&req.password)?;

    let user: User = sqlx::query_as(
        "INSERT INTO users (email, password_hash) VALUES ($1, $2) \
         RETURNING id, email, password_hash, created_at",
    )
    .bind(&email)
    .bind(&password_hash)
    .fetch_one(&state.db)
    .await
    .map_err(|err| match &err {
        sqlx::Error::Database(db) if db.code().as_deref() == Some(UNIQUE_VIOLATION) => {
            AppError::EmailTaken
        }
        _ => err.into(),
    })?;

    let token = issue_token(user.id, &state.config.jwt_secret)?;
    Ok(Json(AuthResponse { token }))
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let email = req.email.trim().to_lowercase();

    let user: Option<User> =
        sqlx::query_as("SELECT id, email, password_hash, created_at FROM users WHERE email = $1")
            .bind(&email)
            .fetch_optional(&state.db)
            .await?;

    let user = user.ok_or(AppError::Unauthorized)?;
    if !verify_password(&req.password, &user.password_hash)? {
        return Err(AppError::Unauthorized);
    }

    let token = issue_token(user.id, &state.config.jwt_secret)?;
    Ok(Json(AuthResponse { token }))
}

/// Returns the user identified by the `Authorization: Bearer <token>` header.
pub async fn me(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<User>, AppError> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;
    let claims =
        verify_token(token, &state.config.jwt_secret).map_err(|_| AppError::Unauthorized)?;

    let user: Option<User> =
        sqlx::query_as("SELECT id, email, password_hash, created_at FROM users WHERE id = $1")
            .bind(claims.sub)
            .fetch_optional(&state.db)
            .await?;

    user.map(Json).ok_or(AppError::Unauthorized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_email() {
        assert_eq!(
            normalize_email("  Foo@Example.COM ").unwrap(),
            "foo@example.com"
        );
    }

    #[test]
    fn rejects_malformed_email() {
        for bad in ["", "foo", "@example.com", "foo@bar"] {
            assert!(normalize_email(bad).is_err(), "{bad:?} should be rejected");
        }
    }
}
