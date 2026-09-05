use std::sync::Arc;

use axum::{
    Json,
    extract::{FromRequestParts, Multipart, Query, State},
    http::{HeaderMap, StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;

use crate::{App, auth, sanitize};

pub(crate) const ADMIN_COOKIE: &str = "hcity_admin";

fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "error": msg }))).into_response()
}

fn ok_json(value: serde_json::Value) -> Response {
    (StatusCode::OK, Json(value)).into_response()
}

pub struct User(pub String);

impl FromRequestParts<Arc<App>> for User {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        app: &Arc<App>,
    ) -> Result<Self, Self::Rejection> {
        auth::current_user(&parts.headers, &app.store)
            .map(User)
            .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "non autenticato"))
    }
}

pub struct Admin;

impl FromRequestParts<Arc<App>> for Admin {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        app: &Arc<App>,
    ) -> Result<Self, Self::Rejection> {
        let ok = app.admin_key.as_deref().is_some_and(|expected| {
            auth::cookie_value(&parts.headers, ADMIN_COOKIE).is_some_and(|v| v == expected)
        });
        if ok {
            Ok(Self)
        } else {
            Err(err(StatusCode::UNAUTHORIZED, "non autenticato"))
        }
    }
}

pub async fn session(State(app): State<Arc<App>>, headers: HeaderMap) -> Response {
    let sub = auth::current_user(&headers, &app.store);
    ok_json(json!({
        "sub": sub,
        "base_domain": app.store.base_domain,
    }))
}

#[derive(Deserialize)]
pub struct SignupBody {
    #[serde(default)]
    pub subdomain: String,
}

pub async fn signup(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Json(body): Json<SignupBody>,
) -> Response {
    if auth::current_user(&headers, &app.store).is_some() {
        return err(StatusCode::BAD_REQUEST, "sei già connesso");
    }

    // name/description start blank and are set later from the dashboard
    match app.store.signup(&body.subdomain, "", "") {
        Ok((sub, token)) => {
            let starter = app.assets_dir.join("starter_index.html");
            if let Ok(html) = std::fs::read_to_string(&starter) {
                let _ = std::fs::write(app.store.site_dir(&sub).join("index.html"), html);
            }
            let domain = format!("{sub}.{}", app.store.base_domain);
            auth::set_cookie_response(
                ok_json(json!({ "sub": sub, "token": token, "domain": domain })),
                &auth::make_cookie(&sub, &token),
            )
        }
        Err(e) => err(StatusCode::BAD_REQUEST, &e),
    }
}

#[derive(Deserialize)]
pub struct LoginBody {
    #[serde(default)]
    pub subdomain: String,
    #[serde(default)]
    pub token: String,
}

pub async fn login(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Json(body): Json<LoginBody>,
) -> Response {
    if auth::current_user(&headers, &app.store).is_some() {
        return err(StatusCode::BAD_REQUEST, "sei già connesso");
    }
    match app.store.login(&body.subdomain, &body.token) {
        Some(sub) => auth::redirect_to_dashboard_with_cookie(&sub, body.token.trim()),
        None => err(StatusCode::UNAUTHORIZED, "sottodominio o chiave non validi"),
    }
}

pub async fn logout() -> Response {
    auth::set_cookie_response(
        ok_json(json!({ "ok": true })),
        &auth::expired_cookie("hcity"),
    )
}

#[derive(Deserialize)]
pub struct SitesQuery {
    pub q: Option<String>,
    pub limit: Option<usize>,
}

pub async fn sites(State(app): State<Arc<App>>, Query(params): Query<SitesQuery>) -> Response {
    let query = params.q.unwrap_or_default();
    let infos = app.store.search(&query);
    let limit = params.limit.unwrap_or(50).min(500);
    ok_json(json!(infos.into_iter().take(limit).collect::<Vec<_>>()))
}

pub async fn me(State(app): State<Arc<App>>, User(sub): User) -> Response {
    let (name, description) = app.store.get_meta(&sub);
    let visits = app.store.get_visits(&sub);
    let files: Vec<serde_json::Value> = app
        .store
        .site_files(&sub)
        .into_iter()
        .map(|(name, size)| json!({ "name": name, "size_bytes": size }))
        .collect();

    ok_json(json!({
        "sub": sub,
        "domain": format!("{sub}.{}", app.store.base_domain),
        "name": name,
        "description": description,
        "quota_bytes": app.store.quota_bytes,
        "used_bytes": app.store.dir_size(&sub),
        "visits": visits,
        "files": files,
    }))
}

#[derive(Deserialize)]
pub struct MetaBody {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}

pub async fn update_meta(
    State(app): State<Arc<App>>,
    User(sub): User,
    Json(body): Json<MetaBody>,
) -> Response {
    match app.store.update_meta(&sub, &body.name, &body.description) {
        Ok(_) => ok_json(json!({ "ok": true })),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// Self-service account deletion.
pub async fn delete_me(State(app): State<Arc<App>>, User(sub): User) -> Response {
    match app.store.delete_site(&sub) {
        Ok(_) => auth::set_cookie_response(
            ok_json(json!({ "ok": true })),
            &auth::expired_cookie("hcity"),
        ),
        Err(e) => err(StatusCode::NOT_FOUND, &e),
    }
}

pub async fn upload(
    State(app): State<Arc<App>>,
    User(sub): User,
    mut multipart: Multipart,
) -> Response {
    let site_root = app.store.site_dir(&sub);
    let mut saved: usize = 0;
    let mut errors: Vec<String> = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let raw_name = match field.file_name() {
            Some(n) => n.to_string(),
            None => continue,
        };
        let clean = match sanitize::clean_upload_name(&raw_name) {
            Some(c) => c,
            None => {
                errors.push(format!("{raw_name}: nome o tipo non ammesso (solo html/htm/css/png/jpg/jpeg/gif/webp/svg/ico)"));
                continue;
            }
        };
        let data = match field.bytes().await {
            Ok(b) => b,
            Err(e) => {
                errors.push(format!("{clean}: errore di lettura ({e})"));
                continue;
            }
        };
        if data.is_empty() {
            errors.push(format!("{clean}: file vuoto"));
            continue;
        }
        let used = app.store.dir_size(&sub);
        if used + data.len() as u64 > app.store.quota_bytes {
            errors.push(format!(
                "{clean}: quota superata (limite {} byte)",
                app.store.quota_bytes
            ));
            continue;
        }
        match std::fs::write(site_root.join(&clean), &data[..]) {
            Ok(_) => saved += 1,
            Err(e) => errors.push(format!("{clean}: scrittura fallita ({e})")),
        }
    }

    ok_json(json!({ "saved": saved, "errors": errors }))
}

#[derive(Deserialize)]
pub struct DeleteFileBody {
    #[serde(default)]
    pub name: String,
}

pub async fn delete_file(
    State(app): State<Arc<App>>,
    User(sub): User,
    Json(body): Json<DeleteFileBody>,
) -> Response {
    let Some(name) = sanitize::safe_file_name(&body.name) else {
        return err(StatusCode::BAD_REQUEST, "percorso non valido");
    };
    let full = app.store.site_dir(&sub).join(&name);
    match full.symlink_metadata() {
        Ok(md) if md.is_file() => {}
        _ => return err(StatusCode::NOT_FOUND, "file non trovato"),
    }
    match std::fs::remove_file(&full) {
        Ok(_) => ok_json(json!({ "ok": true })),
        Err(e) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cancellazione fallita ({e})"),
        ),
    }
}

pub async fn admin_login(State(app): State<Arc<App>>, Json(body): Json<LoginBody>) -> Response {
    match &app.admin_key {
        Some(expected) if body.token == *expected => auth::set_cookie_response(
            ok_json(json!({ "ok": true })),
            &format!("{ADMIN_COOKIE}={expected}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000"),
        ),
        _ => err(StatusCode::UNAUTHORIZED, "chiave sbagliata"),
    }
}

pub async fn admin_logout() -> Response {
    auth::set_cookie_response(
        ok_json(json!({ "ok": true })),
        &auth::expired_cookie(ADMIN_COOKIE),
    )
}

pub async fn admin_sites(State(app): State<Arc<App>>, _: Admin) -> Response {
    let base_suffix = format!(".{}", app.store.base_domain);
    let mut rows: Vec<serde_json::Value> = Vec::new();
    for info in app.store.recent_sites(usize::MAX) {
        if let Some(sub) = info.domain.strip_suffix(&base_suffix) {
            let visits = app.store.get_visits(sub);
            rows.push(json!({
                "sub": sub,
                "domain": info.domain,
                "name": info.name,
                "description": info.description,
                "size_bytes": app.store.dir_size(sub),
                "visits": visits,
            }));
        }
    }
    rows.sort_by(|a, b| a["sub"].as_str().cmp(&b["sub"].as_str()));
    ok_json(json!({ "sites": rows }))
}

#[derive(Deserialize)]
pub struct AdminSubBody {
    #[serde(default)]
    pub sub: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}

pub async fn admin_meta(
    State(app): State<Arc<App>>,
    _: Admin,
    Json(body): Json<AdminSubBody>,
) -> Response {
    let sub = body.sub.trim();
    if sub.is_empty() {
        return err(StatusCode::BAD_REQUEST, "sottodominio mancante");
    }
    match app.store.update_meta(sub, &body.name, &body.description) {
        Ok(_) => ok_json(json!({ "ok": true })),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn admin_reset_key(
    State(app): State<Arc<App>>,
    _: Admin,
    Json(body): Json<AdminSubBody>,
) -> Response {
    let sub = body.sub.trim();
    if sub.is_empty() {
        return err(StatusCode::BAD_REQUEST, "sottodominio mancante");
    }
    match app.store.reset_token(sub) {
        Ok(token) => ok_json(json!({ "sub": sub, "token": token })),
        Err(e) => err(StatusCode::NOT_FOUND, &e),
    }
}

pub async fn admin_delete(
    State(app): State<Arc<App>>,
    _: Admin,
    Json(body): Json<AdminSubBody>,
) -> Response {
    let sub = body.sub.trim();
    if sub.is_empty() {
        return err(StatusCode::BAD_REQUEST, "sottodominio mancante");
    }
    match app.store.delete_site(sub) {
        Ok(_) => ok_json(json!({ "ok": true })),
        Err(e) => err(StatusCode::NOT_FOUND, &e),
    }
}
