//! Theme files: the built-ins compiled into the binary and the user themes the
//! picker writes next to the instance list.
//!
//! Parsing and validation live in the frontend (`src/theme/`), which owns the
//! role catalogue; this module only moves file contents around.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Compiled in so the app ships with themes regardless of the working directory.
const BUILTIN_THEMES: &[(&str, &str)] = &[
    ("default", include_str!("../../themes/builtin/default.yaml")),
    ("grey", include_str!("../../themes/builtin/grey.yaml")),
    (
        "dark-lilac",
        include_str!("../../themes/builtin/dark-lilac.yaml"),
    ),
    (
        "synthwave",
        include_str!("../../themes/builtin/synthwave.yaml"),
    ),
    (
        "aquamarine",
        include_str!("../../themes/builtin/aquamarine.yaml"),
    ),
    ("sunset", include_str!("../../themes/builtin/sunset.yaml")),
    ("summer", include_str!("../../themes/builtin/summer.yaml")),
    ("pastels", include_str!("../../themes/builtin/pastels.yaml")),
    (
        "metallic-sky",
        include_str!("../../themes/builtin/metallic-sky.yaml"),
    ),
    (
        "oldschool",
        include_str!("../../themes/builtin/oldschool.yaml"),
    ),
    (
        "80s-colors",
        include_str!("../../themes/builtin/80s-colors.yaml"),
    ),
    (
        "kanagawa-wave",
        include_str!("../../themes/builtin/kanagawa-wave.yaml"),
    ),
    (
        "kanagawa-dragon",
        include_str!("../../themes/builtin/kanagawa-dragon.yaml"),
    ),
    (
        "kanagawa-lotus",
        include_str!("../../themes/builtin/kanagawa-lotus.yaml"),
    ),
    (
        "solarized",
        include_str!("../../themes/builtin/solarized.yaml"),
    ),
    (
        "calm-seas",
        include_str!("../../themes/builtin/calm-seas.yaml"),
    ),
    (
        "catppuccin-latte",
        include_str!("../../themes/builtin/catppuccin-latte.yaml"),
    ),
    (
        "catppuccin-frappe",
        include_str!("../../themes/builtin/catppuccin-frappe.yaml"),
    ),
    (
        "catppuccin-macchiato",
        include_str!("../../themes/builtin/catppuccin-macchiato.yaml"),
    ),
    (
        "catppuccin-mocha",
        include_str!("../../themes/builtin/catppuccin-mocha.yaml"),
    ),
    (
        "winter-metallic",
        include_str!("../../themes/builtin/winter-metallic.yaml"),
    ),
    (
        "midnight-snow",
        include_str!("../../themes/builtin/midnight-snow.yaml"),
    ),
    (
        "vermillion-daydream",
        include_str!("../../themes/builtin/vermillion-daydream.yaml"),
    ),
];

const MAX_ID_LENGTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSource {
    pub id: String,
    /// `builtin` or `user`.
    pub source: String,
    pub contents: String,
    /// Absolute path for user themes; `None` for built-ins.
    pub path: Option<String>,
}

pub fn user_themes_dir(config_dir: &Path) -> PathBuf {
    config_dir.join("themes")
}

pub fn builtin_themes() -> Vec<ThemeSource> {
    BUILTIN_THEMES
        .iter()
        .map(|(id, contents)| ThemeSource {
            id: (*id).to_string(),
            source: "builtin".to_string(),
            contents: (*contents).to_string(),
            path: None,
        })
        .collect()
}

/// Built-ins first, then whatever the editor has saved — the order both the
/// Themes menu and the picker show them in.
pub fn all_themes(config_dir: &Path) -> Vec<ThemeSource> {
    let mut themes = builtin_themes();
    themes.extend(user_themes(&user_themes_dir(config_dir)));
    themes
}

/// User themes on disk, cheapest first: a directory that cannot be read simply
/// yields no themes instead of failing the picker.
pub fn user_themes(directory: &Path) -> Vec<ThemeSource> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };

    let mut themes: Vec<ThemeSource> = entries
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "yaml"))
        .filter_map(|entry| {
            let path = entry.path();
            let id = path.file_stem()?.to_str()?.to_string();
            let contents = fs::read_to_string(&path).ok()?;

            Some(ThemeSource {
                id,
                source: "user".to_string(),
                contents,
                path: Some(path.to_string_lossy().into_owned()),
            })
        })
        .collect();

    themes.sort_by(|a, b| a.id.cmp(&b.id));
    themes
}

/// Filenames become ids verbatim, so anything that is not a plain slug is
/// rejected rather than sanitised into a surprising file name.
fn valid_id(id: &str) -> Result<(), String> {
    let slug = !id.is_empty()
        && id.len() <= MAX_ID_LENGTH
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && id.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && !id.ends_with('-');

    if slug {
        return Ok(());
    }

    Err(
        "Theme ids must be lowercase letters, digits and dashes (not starting or ending with a dash)."
            .to_string(),
    )
}

pub fn save_user_theme(directory: &Path, id: &str, contents: &str) -> Result<PathBuf, String> {
    valid_id(id)?;

    if BUILTIN_THEMES.iter().any(|(builtin, _)| *builtin == id) {
        return Err(format!(
            "\"{id}\" is a built-in theme. Save yours under a different name."
        ));
    }

    if contents.trim().is_empty() {
        return Err("Refusing to save an empty theme.".to_string());
    }

    fs::create_dir_all(directory)
        .map_err(|error| format!("Could not create {}: {error}", directory.display()))?;

    let path = directory.join(format!("{id}.yaml"));
    let temporary = directory.join(format!("{id}.yaml.tmp"));

    fs::write(&temporary, contents)
        .map_err(|error| format!("Could not write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, &path)
        .map_err(|error| format!("Could not update {}: {error}", path.display()))?;

    Ok(path)
}

pub fn delete_user_theme(directory: &Path, id: &str) -> Result<(), String> {
    valid_id(id)?;

    let path = directory.join(format!("{id}.yaml"));
    if !path.exists() {
        return Err(format!("No saved theme called \"{id}\"."));
    }

    fs::remove_file(&path).map_err(|error| format!("Could not delete {}: {error}", path.display()))
}

/// The theme currently applied to instance windows, persisted so a restart
/// keeps it. The CSS is a snapshot generated by the frontend when the user
/// applies a theme — this module never parses theme files.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveTheme {
    pub id: Option<String>,
    pub css: Option<String>,
}

/// Every injected stylesheet hangs off this element id, so applying a theme
/// twice replaces it instead of stacking.
pub const STYLE_ELEMENT_ID: &str = "kaneo-desktop-theme";

/// Installs or replaces the theme's `<style>` element. Sent to running windows
/// through `eval` and registered as an init script on new ones.
pub fn style_script(css: &str) -> String {
    let css = serde_json::to_string(css).unwrap_or_else(|_| "\"\"".to_string());

    format!(
        "(() => {{\n  const id = {STYLE_ELEMENT_ID:?};\n  let element = document.getElementById(id);\n  if (!element) {{\n    element = document.createElement(\"style\");\n    element.id = id;\n    (document.head || document.documentElement).appendChild(element);\n  }}\n  element.textContent = {css};\n}})();"
    )
}

pub fn clear_script() -> String {
    format!("document.getElementById({STYLE_ELEMENT_ID:?})?.remove();")
}

pub struct ActiveThemeStore {
    path: PathBuf,
    value: ActiveTheme,
}

impl ActiveThemeStore {
    /// A missing or unreadable file means "no theme", never a failed start.
    pub fn load(path: PathBuf) -> Self {
        let value = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<ActiveTheme>(&raw).ok())
            .unwrap_or_default();

        Self { path, value }
    }

    pub fn get(&self) -> ActiveTheme {
        self.value.clone()
    }

    pub fn set(&mut self, id: &str, css: &str) -> Result<(), String> {
        self.value = ActiveTheme {
            id: Some(id.to_string()),
            css: Some(css.to_string()),
        };

        self.save()
    }

    pub fn clear(&mut self) -> Result<(), String> {
        self.value = ActiveTheme::default();
        self.save()
    }

    fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
        }

        let json = serde_json::to_string_pretty(&self.value)
            .map_err(|error| format!("Could not serialize the active theme: {error}"))?;

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
    fn ships_the_builtin_themes() {
        let themes = builtin_themes();

        assert_eq!(themes.len(), BUILTIN_THEMES.len());
        for theme in &themes {
            assert_eq!(theme.source, "builtin");
            assert!(theme.path.is_none());
            assert!(theme.contents.contains("name:"), "{} has no name", theme.id);
            assert!(
                theme.contents.contains("light:") || theme.contents.contains("dark:"),
                "{} defines no mode",
                theme.id
            );
        }
    }

    #[test]
    fn lists_built_ins_before_saved_themes() {
        let directory = tempfile::tempdir().unwrap();
        save_user_theme(&user_themes_dir(directory.path()), "my-theme", "name: x\n").unwrap();

        let themes = all_themes(directory.path());

        assert_eq!(themes.len(), BUILTIN_THEMES.len() + 1);
        assert_eq!(
            themes
                .iter()
                .position(|theme| theme.id == "my-theme")
                .expect("the saved theme should be listed"),
            BUILTIN_THEMES.len(),
        );
    }

    #[test]
    fn saves_reads_and_deletes_user_themes() {
        let directory = tempfile::tempdir().unwrap();
        let themes_dir = user_themes_dir(directory.path());

        assert!(user_themes(&themes_dir).is_empty());

        let path = save_user_theme(&themes_dir, "my-theme", "name: \"My Theme\"\n").unwrap();
        assert!(path.exists());
        assert!(!themes_dir.join("my-theme.yaml.tmp").exists());

        let listed = user_themes(&themes_dir);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "my-theme");
        assert_eq!(listed[0].source, "user");
        assert_eq!(listed[0].contents, "name: \"My Theme\"\n");
        assert_eq!(
            listed[0].path.as_deref(),
            Some(path.to_string_lossy().as_ref())
        );

        delete_user_theme(&themes_dir, "my-theme").unwrap();
        assert!(user_themes(&themes_dir).is_empty());
        assert!(delete_user_theme(&themes_dir, "my-theme").is_err());
    }

    #[test]
    fn refuses_ids_that_are_not_slugs() {
        let directory = tempfile::tempdir().unwrap();
        let themes_dir = user_themes_dir(directory.path());

        for id in [
            "",
            "My Theme",
            "../escape",
            "nested/theme",
            "-leading",
            "trailing-",
            "UPPER",
            &"a".repeat(MAX_ID_LENGTH + 1),
        ] {
            assert!(
                save_user_theme(&themes_dir, id, "name: x\n").is_err(),
                "expected {id:?} to be rejected"
            );
        }

        assert!(!themes_dir.exists(), "nothing should have been written");
    }

    #[test]
    fn refuses_to_shadow_a_builtin() {
        let directory = tempfile::tempdir().unwrap();

        let error = save_user_theme(&user_themes_dir(directory.path()), "default", "name: x\n")
            .unwrap_err();

        assert!(error.contains("built-in"), "unexpected error: {error}");
    }

    #[test]
    fn persists_the_active_theme_across_reloads() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("active-theme.json");

        let mut store = ActiveThemeStore::load(path.clone());
        assert_eq!(store.get(), ActiveTheme::default());

        store
            .set("summer", ":root { --background: #fefae0; }")
            .unwrap();

        let reloaded = ActiveThemeStore::load(path);
        assert_eq!(reloaded.get().id.as_deref(), Some("summer"));
        assert_eq!(
            reloaded.get().css.as_deref(),
            Some(":root { --background: #fefae0; }")
        );

        let mut reloaded = reloaded;
        reloaded.clear().unwrap();
        assert_eq!(
            ActiveThemeStore::load(reloaded.path.clone()).get(),
            ActiveTheme::default()
        );
    }

    #[test]
    fn ignores_a_corrupt_active_theme_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("active-theme.json");
        fs::write(&path, "{ not json").unwrap();

        assert_eq!(ActiveThemeStore::load(path).get(), ActiveTheme::default());
    }

    #[test]
    fn style_script_embeds_the_css_as_a_json_string() {
        let css = ":root {\n  --background: \"quoted\";\n}";
        let script = style_script(css);

        assert!(script.contains(STYLE_ELEMENT_ID));
        assert!(script.contains("document.createElement(\"style\")"));
        assert!(
            script.contains(&serde_json::to_string(css).unwrap()),
            "the CSS must be embedded as a JSON string: {script}"
        );
        // textContent, never innerHTML: a stylesheet cannot inject markup.
        assert!(!script.contains("innerHTML"));

        let clearer = clear_script();
        assert!(clearer.contains(STYLE_ELEMENT_ID));
        assert!(clearer.contains("remove()"));
    }

    #[test]
    fn refuses_empty_contents() {
        let directory = tempfile::tempdir().unwrap();

        assert!(save_user_theme(&user_themes_dir(directory.path()), "empty", "   \n").is_err());
    }
}
