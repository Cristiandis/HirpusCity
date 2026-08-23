use axum::{
    http::{HeaderMap, header},
    response::{IntoResponse, Redirect, Response},
};

use crate::store::Store;

const COOKIE_NAME: &str = "hcity";

pub fn make_cookie(sub: &str, token: &str) -> String {
    format!("{COOKIE_NAME}={sub}:{token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000")
}

pub fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    let mut found = None;
    for pair in cookies.split(';') {
        let pair = pair.trim();
        if let Some(v) = pair.strip_prefix(&format!("{name}=")) {
            found = Some(v.to_string());
        }
    }
    found
}

/// Extract and verify the current user from request headers.
///
/// A session is valid iff the token matches the one registered for the
/// subdomain: whoever holds the token owns the site.
pub fn current_user(headers: &HeaderMap, store: &Store) -> Option<String> {
    let value = cookie_value(headers, COOKIE_NAME)?;
    let (sub, token) = value.split_once(':')?;
    store.login(sub, token)
}

pub fn set_cookie_response(mut resp: Response, cookie: &str) -> Response {
    resp.headers_mut()
        .append(header::SET_COOKIE, cookie.parse().unwrap());
    resp
}

pub fn expired_cookie(name: &str) -> String {
    format!("{name}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0")
}

pub fn redirect_to_dashboard_with_cookie(sub: &str, token: &str) -> Response {
    set_cookie_response(
        Redirect::to("/dashboard.html").into_response(),
        &make_cookie(sub, token),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn cookie_roundtrip_and_tamper() {
        let dir = std::env::temp_dir().join(format!("hcity-auth-{}", Uuid::new_v4()));
        let store = Store::load(dir.clone(), "pages.hirpus".into(), 1024).unwrap();
        let (_sub, token) = store.signup("alice", "", "").unwrap();

        let mut hm = HeaderMap::new();
        hm.insert(
            header::COOKIE,
            make_cookie("alice", &token).parse().unwrap(),
        );
        assert_eq!(current_user(&hm, &store).as_deref(), Some("alice"));

        let bad = [
            make_cookie("alice", "00000000-0000-0000-0000-000000000000"),
            make_cookie("bob", &token),
            "hcity=garbage-without-colon".to_string(),
        ];
        for value in bad {
            let mut hm2 = HeaderMap::new();
            hm2.insert(header::COOKIE, value.parse().unwrap());
            assert_eq!(current_user(&hm2, &store), None);
        }

        std::fs::remove_dir_all(dir).ok();
    }
}
