use utoipa::OpenApi;

fn main() {
    let spec = rust_lol::ApiDoc::openapi();
    let json = spec.to_pretty_json().unwrap();
    let out = std::env::current_dir().unwrap().join("openapi.json");
    std::fs::write(&out, json).unwrap();
    eprintln!("Wrote {}", out.display());
}
