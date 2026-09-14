//! Kaneo Desktop: a native window for a Kaneo instance (cloud or self-hosted).

mod instance_store;
mod instance_window;
mod probe;

use std::sync::Mutex;

use tauri::{AppHandle, Manager, State};

use instance_store::{Instance, InstanceStore};
use probe::Probe;

pub struct AppState {
    store: Mutex<InstanceStore>,
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

#[tauri::command]
async fn probe_instance(url: String) -> Result<Probe, String> {
    probe::probe(&url).await
}

#[tauri::command]
fn open_instance(app: AppHandle, id: String, state: State<'_, AppState>) -> Result<(), String> {
    let instance = state.store.lock().map_err(|_| unavailable())?.get(&id)?;

    instance_window::open(&app, &instance)
}

/// Built once so `run` and the tests share a single context expansion.
fn app_context<R: tauri::Runtime>() -> tauri::Context<R> {
    tauri::generate_context!()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let path = app.path().app_config_dir()?.join("instances.json");
            app.manage(AppState {
                store: Mutex::new(InstanceStore::load(path)),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_instances,
            add_instance,
            remove_instance,
            probe_instance,
            open_instance
        ])
        .run(app_context())
        .expect("error while running tauri application");
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

    fn test_app(store_path: PathBuf) -> App<MockRuntime> {
        mock_builder()
            .invoke_handler(tauri::generate_handler![
                list_instances,
                add_instance,
                remove_instance
            ])
            .manage(AppState {
                store: Mutex::new(InstanceStore::load(store_path)),
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
        let app = test_app(store_path.clone());
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
}
