mod api;
mod auth;
mod sanitize;
mod serve_site;
mod store;

use std::{path::PathBuf, sync::Arc};

use axum::{
    Router,
    extract::{Request, State},
    http::{StatusCode, Uri, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};

use store::Store;

pub struct App {
    pub store: Store,
    pub assets_dir: PathBuf,
    pub admin_key: Option<String>,
}

#[tokio::main]
async fn main() {
    let listen = std::env::var("HIRPUS_LISTEN").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let data_dir =
        PathBuf::from(std::env::var("HIRPUS_DATA_DIR").unwrap_or_else(|_| "./data".into()));
    let assets_dir =
        PathBuf::from(std::env::var("HIRPUS_ASSETS_DIR").unwrap_or_else(|_| "./web".into()));
    let base_domain = std::env::var("HIRPUS_BASE_DOMAIN").unwrap_or_else(|_| "pages.hirpus".into());
    let quota_bytes: u64 = std::env::var("HIRPUS_SITE_QUOTA_MB")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1024)
        * 1024
        * 1024;

    // Admin panel: enabled only by a well-formed UUID in the environment.
    let admin_key = match std::env::var("HIRPUS_ADMIN_UUID").ok() {
        Some(v) if !v.trim().is_empty() && uuid::Uuid::parse_str(v.trim()).is_ok() => Some(v),
        Some(v) if !v.trim().is_empty() => {
            eprintln!("HIRPUS_ADMIN_UUID non valido: pannello admin disattivato");
            None
        }
        _ => None,
    };

    let store = Store::load(data_dir.clone(), base_domain.clone(), quota_bytes)
        .expect("impossibile inizializzare lo storage");
    let app = Arc::new(App {
        store,
        assets_dir,
        admin_key,
    });

    println!("HIRPUSPAGES in ascolto su http://{listen}");
    println!("  dominio:      {base_domain} + *.{base_domain}");
    println!("  dati:         {}", data_dir.display());
    println!("  frontend:     {}", app.assets_dir.display());
    println!("  quota / sito: {}", human_bytes(quota_bytes));
    match &app.admin_key {
        Some(_) => println!("  pannello admin: attivo su /admin.html"),
        None => println!("  pannello admin: disattivo (imposta HIRPUS_ADMIN_UUID)"),
    }

    let base_router = Router::new()
        .route("/", get(asset))
        .route("/{*path}", get(asset))
        .route("/api/session", get(api::session))
        .route("/api/signup", post(api::signup))
        .route("/api/login", post(api::login))
        .route("/api/logout", post(api::logout))
        .route("/api/me/delete", post(api::delete_me))
        .route("/api/sites", get(api::sites))
        .route("/api/me", get(api::me))
        .route("/api/me/meta", post(api::update_meta))
        .route("/api/files", post(api::upload))
        .route("/api/files/delete", post(api::delete_file));

    let router = if app.admin_key.is_some() {
        base_router
            .route("/api/admin/login", post(api::admin_login))
            .route("/api/admin/logout", post(api::admin_logout))
            .route("/api/admin/sites", get(api::admin_sites))
            .route("/api/admin/meta", post(api::admin_meta))
            .route("/api/admin/reset-key", post(api::admin_reset_key))
            .route("/api/admin/delete", post(api::admin_delete))
    } else {
        base_router
    }
    .fallback(main_fallback);

    let app = router
        .layer(middleware::from_fn_with_state(app.clone(), host_dispatch))
        .layer(axum::extract::DefaultBodyLimit::max(
            quota_bytes as usize + 1024 * 1024,
        ))
        .with_state(app);

    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .expect("porta non disponibile");
    axum::serve(listener, app).await.expect("server error");
}

/// Route user-subdomain requests straight to static file serving.
async fn host_dispatch(State(app): State<Arc<App>>, req: Request, next: Next) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    if let Some(sub) = app.store.subdomain_from_host(host) {
        let method = req.method().clone();
        let path = req.uri().path().to_string();
        if method == axum::http::Method::GET || method == axum::http::Method::HEAD {
            return serve_site::serve(&app, &sub, &path).await;
        }
        return (
            StatusCode::METHOD_NOT_ALLOWED,
            axum::body::Body::from("method not allowed"),
        )
            .into_response();
    }
    next.run(req).await
}

async fn asset(State(app): State<Arc<App>>, uri: Uri) -> Response {
    let name = uri.path().trim_start_matches('/');
    let name = if name.is_empty() { "index.html" } else { name };
    if name.split('/').any(|s| s.is_empty() || s.starts_with('.')) {
        return not_found_response(&app);
    }
    if name == "admin.html" && app.admin_key.is_none() {
        return not_found_response(&app);
    }
    match std::fs::read(app.assets_dir.join(name)) {
        Ok(bytes) => {
            let mime = mime_guess::from_path(name)
                .first_raw()
                .unwrap_or("application/octet-stream")
                .to_string();
            (StatusCode::OK, [(header::CONTENT_TYPE, mime)], bytes).into_response()
        }
        Err(_) => not_found_response(&app),
    }
}

/// Platform-wide 404.
pub(crate) fn not_found_response(app: &App) -> Response {
    let bytes = std::fs::read(app.assets_dir.join("404.html"))
        .unwrap_or_else(|_| b"<h1>404 - pagina non trovata</h1>".to_vec());
    (
        StatusCode::NOT_FOUND,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        bytes,
    )
        .into_response()
}

async fn main_fallback(State(app): State<Arc<App>>) -> Response {
    not_found_response(&app)
}

fn human_bytes(bytes: u64) -> String {
    let mb = bytes / (1024 * 1024);
    format!("{mb} MB")
}
