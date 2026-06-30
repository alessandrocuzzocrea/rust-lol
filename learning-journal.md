## 2026-06-30

### Axum — No Batteries-Included Web Framework

Axum is a minimal Rust web framework. It gives you routing, extractors, and middleware — nothing else. No ORM, no templating, no auth. You compose what you need on top.

Compared to other Rust frameworks:

- **Actix Web** — heavier, its own actor runtime, own middleware system, more built-in
- **Loco.rs** — full Rails-style, batteries-included (SeaORM, auth, workers, mailer, CLI scaffolding)
- **Rocket** — macro-heavy, built-in form handling, templating

### Tower Layers (Middleware)

Tower is Axum's middleware system. A **layer** wraps the app and intercepts every request/response — like Express middleware in Node or Django middleware in Python.

```rust
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tower_http::compression::CompressionLayer;

let app = Router::new()
    .route("/", get(hello_world))
    .layer(CompressionLayer::new())   // gzip responses
    .layer(CorsLayer::permissive())   // allow cross-origin
    .layer(TraceLayer::new());        // log every request/response
```

Layers run **bottom-to-top on request**, **top-to-bottom on response**. It's composable — you only ship what you use, no magic.

Common Tower layers:

| Crate | Layer | Does |
|-------|-------|------|
| `tower-http` | `TraceLayer` | Request/response logging |
| `tower-http` | `CorsLayer` | CORS headers |
| `tower-http` | `CompressionLayer` | gzip/brotli responses |
| `tower-http` | `TimeoutLayer` | Request timeout |
| `tower-http` | `LimitLayer` | Rate limiting |
| `tower` | `ConcurrencyLimitLayer` | Max concurrent requests |
| `tower-http` | `AuthLayer` | Auth guard (via `ValidateRequest`) |
| `tower` | `BufferLayer` | Backpressure handling |
| `tower-http` | `SensitiveHeadersLayer` | Masks cookies/auth headers in logs |
