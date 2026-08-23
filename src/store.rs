use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

const RESERVED_SUBDOMAINS: &[&str] = &[
    "www",
    "admin",
    "api",
    "mail",
    "smtp",
    "ftp",
    "ns1",
    "ns2",
    "root",
    "pages",
    "hirpuscity",
];

/// Single source of truth: one record per site. Listings and search are
/// derived from these on demand.
#[derive(Serialize, Deserialize, Clone)]
pub struct SiteRecord {
    pub token: String,
    pub created_at: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// Derived view of a site used by search / listings.
#[derive(Serialize, Clone)]
pub struct SiteInfo {
    pub domain: String,
    pub name: String,
    pub description: String,
    pub created_at: i64,
}

pub struct Store {
    data_dir: PathBuf,
    pub base_domain: String,
    pub quota_bytes: u64,
    sites: Mutex<BTreeMap<String, SiteRecord>>,
}

fn validate_subdomain(sub: &str) -> Result<(), &'static str> {
    if sub.is_empty() {
        return Err("sottodominio vuoto");
    }
    if sub.len() > 63 {
        return Err("sottodominio troppo lungo (max 63 caratteri)");
    }
    let ok_chars = sub
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !ok_chars {
        return Err("ammesse solo lettere minuscole, cifre e trattini");
    }
    if !sub.as_bytes()[0].is_ascii_alphanumeric()
        || !sub.as_bytes()[sub.len() - 1].is_ascii_alphanumeric()
    {
        return Err("deve iniziare e finire con lettera o cifra");
    }
    if RESERVED_SUBDOMAINS.contains(&sub) {
        return Err("sottodominio riservato");
    }
    Ok(())
}

/// Case-insensitive word-substring match against domain/name/description.
fn matches_query(info: &SiteInfo, words: &[String]) -> bool {
    if words.is_empty() {
        return true;
    }
    let name = info.name.to_lowercase();
    let desc = info.description.to_lowercase();
    let domain = info.domain.to_lowercase();
    words.iter().any(|w| {
        let w = w.to_lowercase();
        name.contains(&w) || desc.contains(&w) || domain.contains(&w)
    })
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Store {
    pub fn load(data_dir: PathBuf, base_domain: String, quota_bytes: u64) -> io::Result<Self> {
        fs::create_dir_all(data_dir.join("sites"))?;

        let sites_path = data_dir.join("sites.json");
        let sites: BTreeMap<String, SiteRecord> = if sites_path.exists() {
            let raw = fs::read_to_string(&sites_path)?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            BTreeMap::new()
        };

        let store = Store {
            data_dir,
            base_domain,
            quota_bytes,
            sites: Mutex::new(sites),
        };
        store.persist()?;
        Ok(store)
    }

    fn all_infos(sites: &BTreeMap<String, SiteRecord>, base_domain: &str) -> Vec<SiteInfo> {
        let mut infos: Vec<SiteInfo> = sites
            .iter()
            .map(|(sub, r)| SiteInfo {
                domain: format!("{sub}.{base_domain}"),
                name: r.name.clone(),
                description: r.description.clone(),
                created_at: r.created_at,
            })
            .collect();
        infos.sort_by_key(|i| std::cmp::Reverse(i.created_at));
        infos
    }

    fn persist(&self) -> io::Result<()> {
        let sites = self.sites.lock().unwrap();
        let sites_json = serde_json::to_string_pretty(&*sites).expect("serialize sites.json");
        atomic_write(&self.data_dir.join("sites.json"), sites_json.as_bytes())?;
        Ok(())
    }

    pub fn site_dir(&self, sub: &str) -> PathBuf {
        self.data_dir.join("sites").join(sub)
    }

    pub fn subdomain_from_host(&self, host: &str) -> Option<String> {
        let host = host.split(':').next()?.to_ascii_lowercase();
        let suffix = format!(".{}", self.base_domain);
        let sub = host.strip_suffix(&suffix)?;
        if sub.contains('.') || validate_subdomain(sub).is_err() {
            return None;
        }
        Some(sub.to_string())
    }

    /// Create a new site (registry entry + empty directory).
    /// Returns the generated UUID token.
    pub fn signup(
        &self,
        raw_sub: &str,
        name: &str,
        description: &str,
    ) -> Result<(String, String), String> {
        let sub = raw_sub.trim().to_ascii_lowercase();
        let token = Uuid::new_v4().to_string();

        {
            let sites = self.sites.lock().unwrap();
            if let Err(reason) = validate_subdomain(&sub) {
                return Err(reason.to_string());
            }
            if sites.contains_key(&sub) {
                return Err("sottodominio già occupato".into());
            }
        }

        fs::create_dir_all(self.site_dir(&sub)).map_err(|e| format!("storage error: {e}"))?;

        // Commit the registry entry, re-checking after the I/O window above.
        {
            let mut sites = self.sites.lock().unwrap();
            if sites.contains_key(&sub) {
                return Err("sottodominio già occupato".into());
            }
            sites.insert(
                sub.clone(),
                SiteRecord {
                    token: token.clone(),
                    created_at: now_unix(),
                    name: clean_meta(name, &sub),
                    description: clean_meta(description, ""),
                },
            );
        }
        self.persist().map_err(|e| format!("storage error: {e}"))?;

        Ok((sub, token))
    }

    /// Check sub + token; on success returns the normalized subdomain.
    pub fn login(&self, sub: &str, token: &str) -> Option<String> {
        let sub = sub.trim().to_ascii_lowercase();
        let sites = self.sites.lock().unwrap();
        sites
            .get(&sub)
            .is_some_and(|r| r.token == token.trim())
            .then_some(sub)
    }

    pub fn reset_token(&self, sub: &str) -> Result<String, String> {
        let new_token = Uuid::new_v4().to_string();
        {
            let mut sites = self.sites.lock().unwrap();
            match sites.get_mut(sub) {
                Some(r) => r.token = new_token.clone(),
                None => return Err("sito inesistente".into()),
            }
        }
        self.persist().map_err(|e| format!("storage error: {e}"))?;
        Ok(new_token)
    }

    pub fn delete_site(&self, sub: &str) -> Result<(), String> {
        if !self.exists(sub) {
            return Err("sito inesistente".into());
        }
        let dir = self.site_dir(sub);
        if dir.exists() {
            fs::remove_dir_all(dir).map_err(|e| format!("storage error: {e}"))?;
        }
        {
            let mut sites = self.sites.lock().unwrap();
            if sites.remove(sub).is_none() {
                return Err("sito inesistente".into());
            }
        }
        self.persist().map_err(|e| format!("storage error: {e}"))
    }

    /// (name, description) for a subdomain.
    pub fn get_meta(&self, sub: &str) -> (String, String) {
        let sites = self.sites.lock().unwrap();
        sites
            .get(sub)
            .map(|r| (r.name.clone(), r.description.clone()))
            .unwrap_or_default()
    }

    pub fn exists(&self, sub: &str) -> bool {
        self.sites.lock().unwrap().contains_key(sub)
    }

    pub fn update_meta(&self, sub: &str, name: &str, description: &str) -> io::Result<()> {
        {
            let mut sites = self.sites.lock().unwrap();
            if let Some(r) = sites.get_mut(sub) {
                r.name = clean_meta(name, sub);
                r.description = clean_meta(description, "");
            }
        }
        self.persist()
    }

    pub fn search(&self, query: &str) -> Vec<SiteInfo> {
        let words: Vec<String> = query.split_whitespace().map(str::to_string).collect();
        let sites = self.sites.lock().unwrap();
        let hits: Vec<SiteInfo> = Self::all_infos(&sites, &self.base_domain)
            .into_iter()
            .filter(|i| matches_query(i, &words))
            .take(50)
            .collect();
        hits
    }

    pub fn recent_sites(&self, n: usize) -> Vec<SiteInfo> {
        let sites = self.sites.lock().unwrap();
        Self::all_infos(&sites, &self.base_domain)
            .into_iter()
            .take(n)
            .collect()
    }

    /// Total size in bytes of a site's files (sites are flat).
    pub fn dir_size(&self, sub: &str) -> u64 {
        let mut total = 0;
        if let Ok(rd) = fs::read_dir(self.site_dir(sub)) {
            for entry in rd.flatten() {
                if let Ok(md) = entry.metadata() {
                    total += md.len();
                }
            }
        }
        total
    }

    pub fn site_files(&self, sub: &str) -> Vec<(String, u64)> {
        let mut files = Vec::new();
        if let Ok(rd) = fs::read_dir(self.site_dir(sub)) {
            for entry in rd.flatten() {
                if let Ok(md) = entry.metadata()
                    && md.is_file()
                {
                    files.push((entry.file_name().to_string_lossy().to_string(), md.len()));
                }
            }
        }
        files.sort();
        files
    }
}

fn clean_meta(input: &str, fallback: &str) -> String {
    let trimmed = input.split_whitespace().collect::<Vec<_>>().join(" ");
    let cleaned: String = trimmed.chars().filter(|c| !c.is_control()).collect();
    let s: String = cleaned.chars().take(120).collect();
    if s.trim().is_empty() {
        fallback.to_string()
    } else {
        s.trim().to_string()
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!(
        ".{}.tmp",
        path.file_name().and_then(|f| f.to_str()).unwrap_or("file"),
    ));
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subdomain_rules() {
        assert!(validate_subdomain("miosito").is_ok());
        assert!(validate_subdomain("a-1").is_ok());
        assert!(validate_subdomain("").is_err());
        assert!(validate_subdomain("-bad").is_err());
        assert!(validate_subdomain("bad-").is_err());
        assert!(validate_subdomain("Has Upper").is_err());
        assert!(validate_subdomain("ciao!").is_err());
        assert!(validate_subdomain("www").is_err());
        assert!(validate_subdomain("admin").is_err());
        assert!(validate_subdomain(&"a".repeat(64)).is_err());
    }

    #[test]
    fn query_matching() {
        let e = SiteInfo {
            domain: "gatto.pages.hirpus".into(),
            name: "Il Gatto Nero".into(),
            description: "Un sito sui gatti e sulla pizza".into(),
            created_at: 0,
        };
        assert!(matches_query(&e, &["gatto".into()]));
        assert!(matches_query(&e, &["PIZZA".into()]));
        assert!(matches_query(&e, &["nero".into(), "cane".into()]));
        assert!(!matches_query(&e, &["cane".into()]));
        assert!(matches_query(&e, &[])); // empty query matches everything
    }

    #[test]
    fn signup_login_flow() {
        let dir = std::env::temp_dir().join(format!("hcity-test-{}", Uuid::new_v4()));
        let store = Store::load(dir.clone(), "pages.hirpus".into(), 1024).unwrap();
        let (sub, token) = store.signup("TestSite", "Il Test", "sito di test").unwrap();
        assert_eq!(sub, "testsite");
        assert_eq!(store.login("testsite", &token).as_deref(), Some("testsite"));
        assert_eq!(store.login("testsite", "wrong"), None);
        assert_eq!(store.login("other", &token), None);
        assert!(store.site_dir("testsite").is_dir());

        assert!(
            store
                .recent_sites(10)
                .iter()
                .any(|i| i.domain == "testsite.pages.hirpus" && i.name == "Il Test")
        );

        store.update_meta("testsite", "Nuovo Nome", "").unwrap();
        assert!(
            store
                .recent_sites(10)
                .iter()
                .any(|i| i.name == "Nuovo Nome")
        );
        assert_eq!(store.get_meta("testsite").0, "Nuovo Nome");

        assert!(dir.join("sites.json").exists());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn reset_and_delete_site() {
        let dir = std::env::temp_dir().join(format!("hcity-test-{}", Uuid::new_v4()));
        let store = Store::load(dir.clone(), "pages.hirpus".into(), 1024).unwrap();
        let (_sub, token) = store.signup("dave", "", "").unwrap();

        let new_token = store.reset_token("dave").unwrap();
        assert_ne!(token, new_token);
        assert_eq!(store.login("dave", &token), None);
        assert_eq!(store.login("dave", &new_token).as_deref(), Some("dave"));
        assert!(store.reset_token("ghost").is_err());

        assert!(store.delete_site("dave").is_ok());
        assert!(!store.exists("dave"));
        assert!(!store.site_dir("dave").exists());
        assert!(store.delete_site("dave").is_err());

        let store2 = Store::load(dir.clone(), "pages.hirpus".into(), 1024).unwrap();
        assert!(!store2.exists("dave"));
        std::fs::remove_dir_all(dir).ok();
    }
}
