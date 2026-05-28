fn main() {
    // Release-only sanity: rust_embed silently produces an empty
    // asset table when its target folder is missing or empty, which
    // ships a binary that serves "Not Found" on every non-API URL.
    // Catch the misbuild here.
    //
    // Skipped in debug because `npm run build` doesn't run before
    // `cargo build` in dev; `proxy_to_vite` covers the SPA surface
    // instead.
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        let dist_index = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../ui/dist/index.html");
        if !dist_index.exists() {
            panic!(
                "release build expects the desktop UI bundle at {}, but it doesn't exist. \
                 Run `npm run build --workspace=@ziee/desktop-ui` before `cargo build --release`.",
                dist_index.display()
            );
        }
    }

    tauri_build::build()
}
