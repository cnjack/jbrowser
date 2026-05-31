use sqlx::{MySqlPool, Row};
use uuid::Uuid;

use super::models::UserRow;

pub async fn create_user(
    pool: &MySqlPool,
    id: Uuid,
    email: &str,
    password_hash: &str,
    display_name: &str,
) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO users (id, email, password_hash, display_name) VALUES (?, ?, ?, ?)")
        .bind(id.to_string())
        .bind(email)
        .bind(password_hash)
        .bind(display_name)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_user_by_email(pool: &MySqlPool, email: &str) -> sqlx::Result<Option<UserRow>> {
    let row = sqlx::query(
        "SELECT id, email, password_hash, display_name, is_platform_admin FROM users WHERE email = ?",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| UserRow {
        id: r.get("id"),
        email: r.get("email"),
        password_hash: r.get("password_hash"),
        display_name: r.get("display_name"),
        is_platform_admin: r.get::<bool, _>("is_platform_admin"),
    }))
}

pub async fn get_user_by_id(pool: &MySqlPool, id: Uuid) -> sqlx::Result<Option<UserRow>> {
    let row = sqlx::query(
        "SELECT id, email, password_hash, display_name, is_platform_admin FROM users WHERE id = ?",
    )
    .bind(id.to_string())
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| UserRow {
        id: r.get("id"),
        email: r.get("email"),
        password_hash: r.get("password_hash"),
        display_name: r.get("display_name"),
        is_platform_admin: r.get::<bool, _>("is_platform_admin"),
    }))
}

pub async fn update_user_password(
    pool: &MySqlPool,
    id: Uuid,
    password_hash: &str,
) -> sqlx::Result<bool> {
    let result = sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(password_hash)
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
