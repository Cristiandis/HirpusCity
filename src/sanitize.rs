use std::path::Path;

const ALLOWED_EXTENSIONS: &[&str] = &[
    "html", "htm", "css", "png", "jpg", "jpeg", "gif", "webp", "svg", "ico",
];

fn name_ok(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'(' | b')' | b'+' | b' ')
        })
}

fn is_allowed_extension(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .is_some_and(|e| ALLOWED_EXTENSIONS.contains(&e.as_str()))
}

pub fn clean_upload_name(raw: &str) -> Option<String> {
    let normalized = raw.replace('\\', "/");
    let base = normalized.rsplit('/').next().unwrap_or("");
    if base.starts_with('.') || !name_ok(base) || base == ".." {
        return None;
    }
    if !is_allowed_extension(base) {
        return None;
    }
    Some(base.to_string())
}

pub fn safe_file_name(raw: &str) -> Option<String> {
    if raw.is_empty() || raw.starts_with('.') || !name_ok(raw) {
        return None;
    }
    Some(raw.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_names() {
        assert_eq!(
            clean_upload_name("index.html").as_deref(),
            Some("index.html")
        );
        assert_eq!(
            clean_upload_name("../../etc/passwd.css").as_deref(),
            Some("passwd.css")
        );
        assert_eq!(
            clean_upload_name("..\\..\\evil.html").as_deref(),
            Some("evil.html")
        );
        assert_eq!(clean_upload_name("script.js"), None);
        assert_eq!(clean_upload_name(".hidden.html"), None);
        assert_eq!(clean_upload_name("noext"), None);
        assert_eq!(clean_upload_name(""), None);
        assert_eq!(clean_upload_name("bad<name>.html"), None);
        assert!(clean_upload_name("my (1) pic.PNG").is_some());
    }

    #[test]
    fn file_names() {
        assert_eq!(safe_file_name("index.html").as_deref(), Some("index.html"));
        assert_eq!(
            safe_file_name("a b (1).png").as_deref(),
            Some("a b (1).png")
        );
        assert_eq!(safe_file_name("../secret.txt"), None);
        assert_eq!(safe_file_name("/abs.html"), None);
        assert_eq!(safe_file_name(""), None);
        assert_eq!(safe_file_name("."), None);
        assert_eq!(safe_file_name(".hidden.html"), None);
        assert_eq!(safe_file_name("bad\\name.html"), None);
    }
}
