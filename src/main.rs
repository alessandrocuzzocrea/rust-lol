use std::{collections::HashMap, sync::LazyLock};

use axum::{extract::Path, extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use sqlx::SqlitePool;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

#[derive(Serialize, ToSchema)]
struct User {
    id: u32,
    name: String,
    email: String,
}

static USERS: LazyLock<HashMap<u32, User>> = LazyLock::new(|| {
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
struct ApiDoc;

#[tokio::main]
async fn main() {
    let db_path = std::env::current_dir().unwrap().join("db.sqlite");
    std::fs::File::create(&db_path).unwrap();
    let db_url = format!("sqlite://{}", db_path.display());
    let pool = SqlitePool::connect(&db_url).await.unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();

    let app = axum::Router::new()
        .route("/", axum::routing::get(hello))
        .route("/user/{id}", axum::routing::get(get_user))
        .with_state(pool)
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    println!("🚀 Server running at http://127.0.0.1:3000");
    println!("📖 Docs at http://127.0.0.1:3000/docs");

    axum::serve(listener, app).await.unwrap();
}

#[utoipa::path(
    get,
    path = "/",
    responses(
        (status = 200, description = "Hello world with hit counter", body = String)
    )
)]
async fn hello(State(pool): State<SqlitePool>) -> String {
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
async fn get_user(Path(id): Path<u32>) -> impl IntoResponse {
    match USERS.get(&id) {
        Some(user) => Json(user).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "user not found"})),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn counter_increments() {
        let pool = test_pool().await;

        let body1 = hello(State(pool.clone())).await;
        assert_eq!(body1, "Hello, world! — you are visitor #1");

        let body2 = hello(State(pool.clone())).await;
        assert_eq!(body2, "Hello, world! — you are visitor #2");

        let body3 = hello(State(pool)).await;
        assert_eq!(body3, "Hello, world! — you are visitor #3");
    }

    #[tokio::test]
    async fn get_user_works() {
        let resp = get_user(Path(1)).await.into_response();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_user_404() {
        let resp = get_user(Path(999)).await.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
