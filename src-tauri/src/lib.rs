//! Kaneo Desktop: a native window for a Kaneo instance (cloud or self-hosted).

mod demo;
mod instance_store;
mod instance_window;
mod launcher;
mod probe;
mod themes;

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Manager, Runtime, State};

use instance_store::{Instance, InstanceStore};
use probe::Probe;
use themes::{ActiveTheme, ActiveThemeStore, ThemeSource};

pub struct AppState {
    store: Mutex<InstanceStore>,
    active: Mutex<ActiveThemeStore>,
    config_dir: PathBuf,
}

fn poisoned() -> String {
    "The app state is unavailable.".to_string()
}

fn unavailable() -> String {
    "Your instance list could not be read.".to_string()
}

#[tauri::command]
fn list_instances(state: State<'_, AppState>) -> Result<Vec<Instance>, String> {
    Ok(state.store.lock().map_err(|_| unavailable())?.all())
}

#[tauri::command]
fn add_instance(name: String, url: String, state: State<'_, AppState>) -> Result<Instance, String> {
    state
        .store
        .lock()
        .map_err(|_| unavailable())?
        .add(&name, &url)
}

#[tauri::command]
fn remove_instance(id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.store.lock().map_err(|_| unavailable())?.remove(&id)
}

/// The id of the instance marked to open on launch, if any.
#[tauri::command]
fn default_instance(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let store = state.store.lock().map_err(|_| unavailable())?;
    Ok(store.default().map(|instance| instance.id))
}

/// Marks one instance to open on launch (`Some`) or clears the mark (`None`).
#[tauri::command]
fn set_default_instance(id: Option<String>, state: State<'_, AppState>) -> Result<(), String> {
    state
        .store
        .lock()
        .map_err(|_| unavailable())?
        .set_default(id.as_deref())
}

#[tauri::command]
async fn probe_instance(url: String) -> Result<Probe, String> {
    probe::probe(&url).await
}

#[tauri::command]
fn open_instance(app: AppHandle, id: String, state: State<'_, AppState>) -> Result<(), String> {
    if id == demo::DEMO_INSTANCE_ID {
        demo::start(&app)?;

        let instance = demo::demo_instance();
        let theme = state.active.lock().map_err(|_| poisoned())?.get();

        instance_window::open(&app, &instance, theme.css.as_deref())?;
        launcher::hide(&app);
        return Ok(());
    }

    let instance = state.store.lock().map_err(|_| unavailable())?.get(&id)?;
    let theme = state.active.lock().map_err(|_| poisoned())?.get();

    instance_window::open(&app, &instance, theme.css.as_deref())?;

    // The launcher has done its job once an instance is up; Show Launcher in the
    // menu (or the Dock icon) brings it back.
    launcher::hide(&app);

    Ok(())
}

/// Stores the theme, then pushes it into every instance window that is already
/// open. Windows opened later pick it up through their init script, and the
/// stored copy means a restart keeps the theme.
#[tauri::command]
fn apply_theme<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    css: String,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    state
        .active
        .lock()
        .map_err(|_| poisoned())?
        .set(&id, &css)?;

    launcher::sync_theme_checks(&app, Some(&id));

    let script = themes::style_script(&css);
    let mut applied = 0;

    for (label, window) in app.webview_windows() {
        if !instance_window::is_instance_window(&label) {
            continue;
        }

        // A window that is mid-teardown can refuse the script; the theme is
        // stored, so the next open or restart still picks it up.
        let _ = window.eval(&script);
        applied += 1;
    }

    Ok(applied)
}

#[tauri::command]
fn clear_theme<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>) -> Result<usize, String> {
    state.active.lock().map_err(|_| poisoned())?.clear()?;
    launcher::sync_theme_checks(&app, None);

    let script = themes::clear_script();
    let mut cleared = 0;

    for (label, window) in app.webview_windows() {
        if !instance_window::is_instance_window(&label) {
            continue;
        }

        let _ = window.eval(&script);
        cleared += 1;
    }

    Ok(cleared)
}

#[tauri::command]
fn active_theme(state: State<'_, AppState>) -> Result<ActiveTheme, String> {
    Ok(state.active.lock().map_err(|_| poisoned())?.get())
}

/// Built-in themes plus whatever the user has dropped into the themes
/// directory, so the launcher can parse and apply them.
#[tauri::command]
fn list_themes(state: State<'_, AppState>) -> Vec<ThemeSource> {
    themes::all_themes(&state.config_dir)
}

/// Built once so `run` and the tests share a single context expansion.
fn app_context<R: tauri::Runtime>() -> tauri::Context<R> {
    tauri::generate_context!()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let active = ActiveThemeStore::load(config_dir.join("active-theme.json"));

            app.set_menu(launcher::menu(
                app.handle(),
                &themes::all_themes(&config_dir),
                active.get().id.as_deref(),
            )?)?;

            let store = InstanceStore::load(config_dir.join("instances.json"));
            launcher::keep_alive_on_close(app.handle());

            // Launch straight into the default instance when one is marked, and
            // keep the launcher out of the way; the Launcher menu (⌘⇧L) and the
            // Dock icon bring it back. Anything that fails falls back to the
            // launcher — never to an invisible app.
            let mut launched = false;
            if let Some(instance) = store.default() {
                let theme_css = active.get().css.clone();
                launched =
                    instance_window::open(app.handle(), &instance, theme_css.as_deref()).is_ok();
            }
            if launched {
                launcher::hide(app.handle());
            } else {
                launcher::show(app.handle());
            }

            app.manage(AppState {
                store: Mutex::new(store),
                active: Mutex::new(active),
                config_dir,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_instances,
            add_instance,
            remove_instance,
            default_instance,
            set_default_instance,
            probe_instance,
            open_instance,
            list_themes,
            apply_theme,
            clear_theme,
            active_theme
        ])
        .build(app_context())
        .expect("error while running tauri application");

    app.run(|handle, event| {
        match event {
            // Clicking the Dock icon shows the workspace the user was in, and
            // only asks for the launcher when no instance window exists.
            tauri::RunEvent::Reopen {
                has_visible_windows,
                ..
            } => launcher::reopen(handle, has_visible_windows),
            tauri::RunEvent::MenuEvent(event) => {
                let id = event.id().as_ref().to_string();
                launcher::handle_menu_event(handle, &id);
            }
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde::de::DeserializeOwned;
    use serde_json::{json, Value};
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{mock_builder, MockRuntime, INVOKE_KEY};
    use tauri::webview::InvokeRequest;
    use tauri::{App, WebviewWindow, WebviewWindowBuilder};

    use super::*;

    fn test_app(config_dir: PathBuf) -> App<MockRuntime> {
        mock_builder()
            .invoke_handler(tauri::generate_handler![
                list_instances,
                add_instance,
                remove_instance,
                list_themes,
                apply_theme,
                clear_theme,
                active_theme
            ])
            .manage(AppState {
                store: Mutex::new(InstanceStore::load(config_dir.join("instances.json"))),
                active: Mutex::new(ActiveThemeStore::load(config_dir.join("active-theme.json"))),
                config_dir,
            })
            // The real context, so the capability that allows these commands
            // for the shell window is part of the picture.
            .build(app_context())
            .expect("failed to build the test app")
    }

    /// The shell window's origin; anything else counts as remote and needs an
    /// explicit ACL capability.
    #[cfg(any(target_os = "windows", target_os = "android"))]
    const SHELL_ORIGIN: &str = "http://tauri.localhost";
    #[cfg(not(any(target_os = "windows", target_os = "android")))]
    const SHELL_ORIGIN: &str = "tauri://localhost";

    /// Calls a command through Tauri's IPC layer — the same path the frontend
    /// uses, so command names and argument names are covered too.
    fn invoke<T: DeserializeOwned>(
        webview: &WebviewWindow<MockRuntime>,
        cmd: &str,
        body: Value,
    ) -> Result<T, Value> {
        tauri::test::get_ipc_response(
            webview,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: SHELL_ORIGIN.parse().unwrap(),
                body: InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|response| {
            response
                .deserialize::<T>()
                .expect("failed to deserialize the command response")
        })
    }

    #[test]
    fn the_frontend_contract_manages_instances() {
        let directory = tempfile::tempdir().unwrap();
        let store_path = directory.path().join("instances.json");
        let app = test_app(directory.path().to_path_buf());
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();

        let added: Instance = invoke(
            &webview,
            "add_instance",
            json!({ "name": "Work", "url": "kaneo.example.com" }),
        )
        .expect("add_instance should succeed");

        assert_eq!(added.name, "Work");
        assert_eq!(added.url, "https://kaneo.example.com");
        assert!(store_path.exists(), "the instance list should persist");

        let listed: Vec<Instance> =
            invoke(&webview, "list_instances", json!({})).expect("list_instances should succeed");
        assert_eq!(listed, vec![added.clone()]);

        let duplicate: Result<Instance, Value> = invoke(
            &webview,
            "add_instance",
            json!({ "name": "", "url": "https://kaneo.example.com/" }),
        );
        let message = duplicate.expect_err("a duplicate instance should be rejected");
        assert!(
            message.as_str().unwrap_or_default().contains("already"),
            "unexpected error: {message}"
        );

        let removed: Value = invoke(&webview, "remove_instance", json!({ "id": added.id }))
            .expect("remove_instance should succeed");
        assert!(removed.is_null());

        let listed: Vec<Instance> =
            invoke(&webview, "list_instances", json!({})).expect("list_instances should succeed");
        assert!(listed.is_empty());
    }

    #[test]
    fn the_frontend_contract_applies_and_clears_themes() {
        let directory = tempfile::tempdir().unwrap();
        let app = test_app(directory.path().to_path_buf());
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();

        let initial: ActiveTheme =
            invoke(&webview, "active_theme", json!({})).expect("active_theme should succeed");
        assert_eq!(initial.id, None);

        let css = ":root { --background: #fefae0; }";
        let applied: usize = invoke(
            &webview,
            "apply_theme",
            json!({ "id": "summer", "css": css }),
        )
        .expect("apply_theme should succeed");
        assert_eq!(applied, 0, "the shell window must not be themed");

        let active: ActiveTheme =
            invoke(&webview, "active_theme", json!({})).expect("active_theme should succeed");
        assert_eq!(active.id.as_deref(), Some("summer"));
        assert_eq!(active.css.as_deref(), Some(css));

        // The stored copy is what a restart reads back.
        let stored = ActiveThemeStore::load(directory.path().join("active-theme.json"));
        assert_eq!(stored.get().id.as_deref(), Some("summer"));

        // An open instance window counts as themed; the shell window still does not.
        let instance = Instance {
            id: "6f1c2f9e-0f4b-4b3a-8a9e-1b0e6a5c3d21".to_string(),
            name: "Kaneo Cloud".to_string(),
            url: "https://cloud.kaneo.app".to_string(),
        };
        instance_window::open(&app.handle().clone(), &instance, Some(css)).unwrap();

        let applied: usize = invoke(
            &webview,
            "apply_theme",
            json!({ "id": "summer", "css": css }),
        )
        .expect("apply_theme should succeed");
        assert_eq!(applied, 1);

        let cleared: usize =
            invoke(&webview, "clear_theme", json!({})).expect("clear_theme should succeed");
        assert_eq!(cleared, 1);

        let active: ActiveTheme =
            invoke(&webview, "active_theme", json!({})).expect("active_theme should succeed");
        assert_eq!(active.id, None);
    }

    /// The picker no longer writes theme files; users drop YAML files into the
    /// themes directory. The contract to pin: `list_themes` reads them and
    /// marks their source, and a user file cannot shadow a built-in id.
    #[test]
    fn the_frontend_contract_lists_themes_from_disk() {
        let directory = tempfile::tempdir().unwrap();
        let app = test_app(directory.path().to_path_buf());
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();

        let themes: Vec<ThemeSource> =
            invoke(&webview, "list_themes", json!({})).expect("list_themes should succeed");
        assert!(
            themes
                .iter()
                .any(|theme| theme.id == "default" && theme.source == "builtin"),
            "the built-in themes should ship with the app"
        );

        let themes_dir = themes::user_themes_dir(directory.path());
        std::fs::create_dir_all(&themes_dir).unwrap();
        std::fs::write(themes_dir.join("my-theme.yaml"), "name: \"My Theme\"\n").unwrap();

        let themes: Vec<ThemeSource> =
            invoke(&webview, "list_themes", json!({})).expect("list_themes should succeed");
        let saved = themes
            .iter()
            .find(|theme| theme.id == "my-theme")
            .expect("the dropped theme should be listed");
        assert_eq!(saved.source, "user");
        assert_eq!(saved.contents, "name: \"My Theme\"\n");

        // Built-ins come first, so a user file named like one is ignored —
        // the built-in wins and the menu never shows two ticks for one id.
        std::fs::write(themes_dir.join("default.yaml"), "name: \"Fake Default\"\n").unwrap();
        let themes: Vec<ThemeSource> =
            invoke(&webview, "list_themes", json!({})).expect("list_themes should succeed");
        assert_eq!(
            themes.iter().filter(|theme| theme.id == "default").count(),
            1,
            "a user file must not shadow a built-in id"
        );
    }
}
