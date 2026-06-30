use axum::{extract::State, routing::get, Router};
use sqlx::SqlitePool;

#[tokio::main]
async fn main() {
    // Connect to SQLite — file must exist first, then SQLx opens it
    let db_path = std::env::current_dir().unwrap().join("db.sqlite");
    std::fs::File::create(&db_path).unwrap(); // touch the file
    let db_url = format!("sqlite://{}", db_path.display());
    let pool = SqlitePool::connect(&db_url).await.unwrap();

    // Run pending migrations
    sqlx::migrate!().run(&pool).await.unwrap();

    // Build the app with shared pool state
    let app = Router::new()
        .route("/", get(handler))
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();

    println!("🚀 Server running at http://127.0.0.1:3000");

    axum::serve(listener, app).await.unwrap();
}

async fn handler(State(pool): State<SqlitePool>) -> String {
    // Atomically increment the visit counter
    sqlx::query("UPDATE counters SET value = value + 1 WHERE name = 'visits'")
        .execute(&pool)
        .await
        .unwrap();

    // Read the current count
    let (count,): (i64,) =
        sqlx::query_as("SELECT value FROM counters WHERE name = 'visits'")
            .fetch_one(&pool)
            .await
            .unwrap();

    format!("Hello, world! — you are visitor #{count}")
}
