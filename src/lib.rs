use std::{collections::HashMap, sync::LazyLock};

use axum::{extract::Path, extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use sqlx::SqlitePool;
use utoipa::{OpenApi, ToSchema};

#[derive(Serialize, ToSchema)]
pub struct User {
    pub id: u32,
    pub name: String,
    pub email: String,
}

pub static USERS: LazyLock<HashMap<u32, User>> = LazyLock::new(|| {
    HashMap::from([
        (1, User { id: 1, name: "Alice".into(), email: "alice@example.com".into() }),
        (2, User { id: 2, name: "Bob".into(), email: "bob@example.com".into() }),
        (3, User { id: 3, name: "Carol".into(), email: "carol@example.com".into() }),
    ])
});

#[derive(OpenApi)]
#[openapi(
    paths(hello, get_user),
    components(schemas(User))
)]
pub struct ApiDoc;

#[utoipa::path(
    get,
    path = "/",
    responses(
        (status = 200, description = "Hello world with hit counter", body = String)
    )
)]
pub async fn hello(State(pool): State<SqlitePool>) -> String {
    sqlx::query!("UPDATE counters SET value = value + 1 WHERE name = 'visits'")
        .execute(&pool)
        .await
        .unwrap();

    let row = sqlx::query!("SELECT value FROM counters WHERE name = 'visits'")
        .fetch_one(&pool)
        .await
        .unwrap();

    format!("Hello, world! — you are visitor #{}", row.value)
}

#[utoipa::path(
    get,
    path = "/user/{id}",
    responses(
        (status = 200, description = "User found", body = User),
        (status = 404, description = "User not found")
    ),
    params(
        ("id" = u32, Path, description = "User ID")
    )
)]
pub async fn get_user(Path(id): Path<u32>) -> impl IntoResponse {
    match USERS.get(&id) {
        Some(user) => Json(user).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "user not found"})),
        )
            .into_response(),
    }
}
