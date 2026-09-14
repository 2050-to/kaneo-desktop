//! Persisted list of Kaneo instances the user has added.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

const CLOUD_HOST: &str = "cloud.kaneo.app";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub name: String,
    /// Canonical origin of the instance's web app, e.g. `https://cloud.kaneo.app`.
    pub url: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct StoreFile {
    instances: Vec<Instance>,
}

/// Canonicalizes user input into the URL of a Kaneo web app.
///
/// Accepts bare hosts (`cloud.kaneo.app`), full URLs, subpath deployments and
/// API base URLs copied from the docs (`https://kaneo.example.com/api`).
pub fn normalize_url(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Enter an instance URL.".to_string());
    }

    let candidate = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        let scheme = if is_loopback_input(trimmed) {
            "http"
        } else {
            "https"
        };
        format!("{scheme}://{trimmed}")
    };

    let parsed =
        Url::parse(&candidate).map_err(|_| format!("\"{trimmed}\" is not a valid URL."))?;

    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(format!(
            "Only http and https instances are supported, not \"{}\".",
            parsed.scheme()
        ));
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| format!("\"{trimmed}\" has no host."))?;

    // `/api` is the API base, not the web app; users copy it from the docs.
    let mut path = parsed.path().trim_end_matches('/').to_string();
    if let Some(without_api) = path.strip_suffix("/api") {
        path = without_api.to_string();
    }
    if path == "/" {
        path.clear();
    }

    let mut normalized = format!("{}://{host}", parsed.scheme());
    if let Some(port) = parsed.port() {
        normalized.push_str(&format!(":{port}"));
    }
    normalized.push_str(&path);

    Ok(normalized)
}

/// Bare hosts such as `localhost:5173` are local development instances, which
/// normally run without TLS.
fn is_loopback_input(input: &str) -> bool {
    ["localhost", "127.0.0.1", "[::1]"].iter().any(|prefix| {
        input
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(':') || rest.starts_with('/'))
    })
}

fn host_of(url: &str) -> Option<String> {
    Url::parse(url).ok()?.host_str().map(str::to_string)
}

fn display_name(name: &str, url: &str) -> String {
    let trimmed = name.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }

    match host_of(url) {
        Some(host) if host == CLOUD_HOST => "Kaneo Cloud".to_string(),
        Some(host) => host,
        None => url.to_string(),
    }
}

pub struct InstanceStore {
    path: PathBuf,
    instances: Vec<Instance>,
}

impl InstanceStore {
    /// Reads the store, falling back to an empty list when the file is absent
    /// or unreadable. A broken file must not stop the app from starting.
    pub fn load(path: PathBuf) -> Self {
        let instances = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<StoreFile>(&raw).ok())
            .map(|file| file.instances)
            .unwrap_or_default();

        Self { path, instances }
    }

    pub fn all(&self) -> Vec<Instance> {
        self.instances.clone()
    }

    pub fn get(&self, id: &str) -> Result<Instance, String> {
        self.instances
            .iter()
            .find(|instance| instance.id == id)
            .cloned()
            .ok_or_else(|| "That instance is no longer in your list.".to_string())
    }

    pub fn add(&mut self, name: &str, url: &str) -> Result<Instance, String> {
        let url = normalize_url(url)?;

        if let Some(existing) = self.instances.iter().find(|instance| instance.url == url) {
            return Err(format!("\"{}\" is already in your list.", existing.name));
        }

        let instance = Instance {
            id: Uuid::new_v4().to_string(),
            name: display_name(name, &url),
            url,
        };

        self.instances.push(instance.clone());
        self.save()?;

        Ok(instance)
    }

    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        let before = self.instances.len();
        self.instances.retain(|instance| instance.id != id);

        if self.instances.len() == before {
            return Err("That instance is no longer in your list.".to_string());
        }

        self.save()
    }

    /// Writes through a temporary file so a crash mid-write cannot leave a
    /// half-written instance list behind.
    fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
        }

        let json = serde_json::to_string_pretty(&StoreFile {
            instances: self.instances.clone(),
        })
        .map_err(|error| format!("Could not serialize your instances: {error}"))?;

        let temporary = self.path.with_extension("json.tmp");
        fs::write(&temporary, json)
            .map_err(|error| format!("Could not write {}: {error}", temporary.display()))?;
        fs::rename(&temporary, &self.path)
            .map_err(|error| format!("Could not update {}: {error}", self.path.display()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_https_to_bare_hosts() {
        assert_eq!(
            normalize_url("cloud.kaneo.app").unwrap(),
            "https://cloud.kaneo.app"
        );
    }

    #[test]
    fn uses_http_for_loopback_hosts() {
        assert_eq!(
            normalize_url("localhost:5173").unwrap(),
            "http://localhost:5173"
        );
        assert_eq!(
            normalize_url("127.0.0.1:5173/").unwrap(),
            "http://127.0.0.1:5173"
        );
    }

    #[test]
    fn keeps_the_scheme_the_user_typed() {
        assert_eq!(
            normalize_url("http://kaneo.internal:5173").unwrap(),
            "http://kaneo.internal:5173"
        );
    }

    #[test]
    fn strips_trailing_slashes_and_lowercases_the_host() {
        assert_eq!(
            normalize_url("https://Kaneo.Example.COM//").unwrap(),
            "https://kaneo.example.com"
        );
    }

    #[test]
    fn drops_the_api_suffix_and_query_string() {
        assert_eq!(
            normalize_url("https://kaneo.example.com/kaneo/api").unwrap(),
            "https://kaneo.example.com/kaneo"
        );
        assert_eq!(
            normalize_url("https://kaneo.example.com/?utm=1").unwrap(),
            "https://kaneo.example.com"
        );
    }

    #[test]
    fn keeps_subpath_deployments() {
        assert_eq!(
            normalize_url("kaneo.example.com/team/kaneo/").unwrap(),
            "https://kaneo.example.com/team/kaneo"
        );
    }

    #[test]
    fn is_idempotent() {
        let once = normalize_url("https://kaneo.example.com/kaneo/api").unwrap();
        assert_eq!(normalize_url(&once).unwrap(), once);
    }

    #[test]
    fn rejects_empty_and_unsupported_input() {
        assert!(normalize_url("   ").is_err());
        assert!(normalize_url("ftp://kaneo.example.com").is_err());
        assert!(normalize_url("https://").is_err());
    }

    #[test]
    fn names_instances_after_their_host_by_default() {
        assert_eq!(display_name("", "https://cloud.kaneo.app"), "Kaneo Cloud");
        assert_eq!(
            display_name("", "https://kaneo.example.com"),
            "kaneo.example.com"
        );
        assert_eq!(
            display_name("  Work  ", "https://kaneo.example.com"),
            "Work"
        );
    }

    fn store() -> (tempfile::TempDir, InstanceStore) {
        let directory = tempfile::tempdir().unwrap();
        let store = InstanceStore::load(directory.path().join("instances.json"));
        (directory, store)
    }

    #[test]
    fn persists_instances_across_reloads() {
        let (directory, mut store) = store();
        let added = store.add("Work", "kaneo.example.com/").unwrap();

        let reloaded = InstanceStore::load(directory.path().join("instances.json"));
        assert_eq!(reloaded.all(), vec![added]);
    }

    #[test]
    fn refuses_duplicate_instances_ignoring_input_form() {
        let (_directory, mut store) = store();
        store.add("Work", "https://kaneo.example.com").unwrap();

        let error = store.add("Other", "kaneo.example.com/api").unwrap_err();
        assert!(error.contains("already"), "unexpected error: {error}");
        assert_eq!(store.all().len(), 1);
    }

    #[test]
    fn removes_instances_by_id() {
        let (_directory, mut store) = store();
        let first = store.add("", "https://one.example.com").unwrap();
        store.add("", "https://two.example.com").unwrap();

        store.remove(&first.id).unwrap();

        assert_eq!(
            store
                .all()
                .iter()
                .map(|i| i.url.clone())
                .collect::<Vec<_>>(),
            vec!["https://two.example.com".to_string()]
        );
        assert!(store.remove(&first.id).is_err());
    }

    #[test]
    fn recovers_from_a_corrupt_store_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("instances.json");
        fs::write(&path, "{ not json").unwrap();

        assert!(InstanceStore::load(path).all().is_empty());
    }
}
