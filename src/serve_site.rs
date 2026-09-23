use axum::{
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use percent_encoding::percent_decode_str;

use crate::App;

/// Serve files for a user subdomain. Path arrives percent-encoded.
pub async fn serve(app: &App, sub: &str, path: &str) -> Response {
    let site_root = app.store.site_dir(sub);
    if !app.store.exists(sub) || !site_root.is_dir() {
        return crate::not_found_response(app);
    }
    app.store.record_traffic(sub);

    let decoded = percent_decode_str(path).decode_utf8_lossy();
    let name = match flat_target(&decoded) {
        Some(n) => n,
        None => return crate::not_found_response(app),
    };

    let full = site_root.join(&name);
    if !full.is_file() {
        // custom 404.html if the user uploaded one
        let custom = site_root.join("404.html");
        if custom.is_file()
            && let Ok(bytes) = std::fs::read(&custom)
        {
            return (
                StatusCode::NOT_FOUND,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                bytes,
            )
                .into_response();
        }
        return crate::not_found_response(app);
    }

    match std::fs::read(&full) {
        Ok(bytes) => {
            let mime = mime_guess::from_path(&full)
                .first_raw()
                .unwrap_or("application/octet-stream")
                .to_string();
            (StatusCode::OK, [(header::CONTENT_TYPE, mime)], bytes).into_response()
        }
        Err(_) => crate::not_found_response(app),
    }
}

/// Map a URL path to a flat file name. Sites have no subdirectories
fn flat_target(path: &str) -> Option<String> {
    let segs: Vec<&str> = path
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    match segs.as_slice() {
        [] => Some("index.html".into()),
        [only] if *only != ".." => Some((*only).to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use uuid::Uuid;

    #[test]
    fn flat_target_rules() {
        assert_eq!(flat_target("/").as_deref(), Some("index.html"));
        assert_eq!(flat_target("").as_deref(), Some("index.html"));
        assert_eq!(flat_target("/./").as_deref(), Some("index.html"));
        assert_eq!(flat_target("/pagina.html").as_deref(), Some("pagina.html"));
        assert_eq!(flat_target("/../etc/passwd"), None);
        assert_eq!(flat_target("/a/b.html"), None);
        assert_eq!(flat_target("/ok/../nope"), None);
    }

    #[tokio::test]
    async fn traffic_counts_all_site_responses() {
        let dir = std::env::temp_dir().join(format!("hcity-serve-{}", Uuid::new_v4()));
        let app = crate::App {
            store: Store::load(dir.clone(), "pages.hirpus".into(), 1024).unwrap(),
            assets_dir: std::path::PathBuf::from("/nonexistent"),
            admin_key: None,
        };
        app.store.signup("uno", "", "").unwrap();
        let site = app.store.site_dir("uno");
        std::fs::write(site.join("index.html"), "<h1>ciao</h1>").unwrap();
        std::fs::write(site.join("404.html"), "<h1>manca</h1>").unwrap();

        serve(&app, "uno", "/index.html").await;
        serve(&app, "uno", "/manca.html").await;
        serve(&app, "uno", "/altro.html").await;
        assert_eq!(app.store.get_visits("uno"), 3);

        // a nonexistent site is not a visit
        serve(&app, "ghost", "/index.html").await;
        assert_eq!(app.store.get_visits("ghost"), 0);

        std::fs::remove_dir_all(dir).ok();
    }
}
