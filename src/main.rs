use sqlx::SqlitePool;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[tokio::main]
async fn main() {
    let db_path = std::env::current_dir().unwrap().join("db.sqlite");
    std::fs::File::create(&db_path).unwrap();
    let db_url = format!("sqlite://{}", db_path.display());
    let pool = SqlitePool::connect(&db_url).await.unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();

    let app = axum::Router::new()
        .route("/", axum::routing::get(rust_lol::hello))
        .route("/user/{id}", axum::routing::get(rust_lol::get_user))
        .with_state(pool)
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", rust_lol::ApiDoc::openapi()));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    println!("🚀 Server running at http://127.0.0.1:3000");
    println!("📖 Docs at http://127.0.0.1:3000/docs");

    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{extract::Path, extract::State, http::StatusCode, response::IntoResponse};

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn counter_increments() {
        let pool = test_pool().await;

        let body1 = rust_lol::hello(State(pool.clone())).await;
        assert_eq!(body1, "Hello, world! — you are visitor #1");

        let body2 = rust_lol::hello(State(pool.clone())).await;
        assert_eq!(body2, "Hello, world! — you are visitor #2");

        let body3 = rust_lol::hello(State(pool)).await;
        assert_eq!(body3, "Hello, world! — you are visitor #3");
    }

    #[tokio::test]
    async fn get_user_works() {
        let resp = rust_lol::get_user(Path(1)).await.into_response();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_user_404() {
        let resp = rust_lol::get_user(Path(999)).await.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
