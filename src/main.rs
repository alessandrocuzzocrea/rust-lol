use axum::{routing::get, Router};

#[tokio::main]
async fn main() {
    // Build our app with a single route
    let app = Router::new().route("/", get(hello_world));

    // Bind to localhost:3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();

    println!("🚀 Server running at http://127.0.0.1:3000");

    axum::serve(listener, app).await.unwrap();
}

async fn hello_world() -> &'static str {
    "Hello, world!"
}
