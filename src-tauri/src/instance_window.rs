//! Windows that host a Kaneo instance. Each instance gets its own webview
//! storage, so signing into one never leaks cookies into another.

#[cfg(any(target_os = "windows", target_os = "linux"))]
use std::path::PathBuf;

use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;
use url::Url;
#[cfg(target_os = "macos")]
use uuid::Uuid;

use crate::instance_store::Instance;

pub fn window_label(instance_id: &str) -> String {
    format!("instance-{instance_id}")
}

/// Opens the instance, or focuses the window when it is already open.
pub fn open<R: Runtime>(app: &AppHandle<R>, instance: &Instance) -> Result<(), String> {
    let label = window_label(&instance.id);

    if let Some(window) = app.get_webview_window(&label) {
        window
            .set_focus()
            .map_err(|error| format!("Could not focus {}: {error}", instance.name))?;
        return Ok(());
    }

    let url = Url::parse(&instance.url)
        .map_err(|_| format!("\"{}\" is not a valid URL.", instance.url))?;

    let opener = app.clone();
    let mut builder = WebviewWindowBuilder::new(app, label.as_str(), WebviewUrl::External(url))
        .title(&instance.name)
        .inner_size(1280.0, 860.0)
        .min_inner_size(720.0, 480.0)
        .center()
        // Kaneo drags tasks with the HTML5 drag-and-drop API, which Tauri's
        // native file-drop handler otherwise swallows on Windows.
        .disable_drag_drop_handler()
        // Copying task links and code blocks needs the clipboard API on
        // Linux and Windows.
        .enable_clipboard_access()
        .devtools(cfg!(debug_assertions))
        .on_new_window(move |url, _features| {
            // Kaneo opens pull requests, docs and integration links with
            // `window.open`. Hand those to the system browser rather than
            // spawning a webview that has no instance context.
            if is_web_url(&url) {
                let _ = opener.opener().open_url(url.as_str(), None::<&str>);
            }

            NewWindowResponse::Deny
        });

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        builder = builder.data_directory(storage_dir(app, &instance.id)?);
    }

    #[cfg(target_os = "macos")]
    {
        builder = builder.data_store_identifier(storage_identifier(&instance.id)?);
    }

    builder
        .build()
        .map_err(|error| format!("Could not open {}: {error}", instance.name))?;

    Ok(())
}

fn is_web_url(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

/// Backing directory for a webview's cookies and localStorage.
/// Unsupported on macOS, which uses [`storage_identifier`] instead.
#[cfg(any(target_os = "windows", target_os = "linux"))]
fn storage_dir<R: Runtime>(app: &AppHandle<R>, instance_id: &str) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("webviews").join(instance_id))
        .map_err(|error| format!("Could not resolve the app data directory: {error}"))
}

/// WKWebView isolates storage by data store identifier instead of directory.
/// macOS 14+ only; older releases fall back to the shared default store.
#[cfg(target_os = "macos")]
fn storage_identifier(instance_id: &str) -> Result<[u8; 16], String> {
    Uuid::parse_str(instance_id)
        .map(|id| *id.as_bytes())
        .map_err(|_| format!("\"{instance_id}\" is not a valid instance id."))
}

#[cfg(test)]
mod tests {
    use tauri::test::{mock_builder, mock_context, noop_assets};

    use super::*;

    #[test]
    fn labels_are_namespaced_per_instance() {
        assert_eq!(window_label("abc"), "instance-abc");
    }

    #[test]
    fn only_http_urls_go_to_the_system_browser() {
        assert!(is_web_url(&Url::parse("https://kaneo.app/docs").unwrap()));
        assert!(is_web_url(&Url::parse("http://localhost:5173").unwrap()));
        assert!(!is_web_url(&Url::parse("mailto:team@kaneo.app").unwrap()));
        assert!(!is_web_url(&Url::parse("about:blank").unwrap()));
    }

    #[test]
    fn opening_an_instance_reuses_its_window() {
        let app = mock_builder()
            .build(mock_context(noop_assets()))
            .expect("failed to build the test app");
        let handle = app.handle().clone();
        let instance = Instance {
            id: "6f1c2f9e-0f4b-4b3a-8a9e-1b0e6a5c3d21".to_string(),
            name: "Kaneo Cloud".to_string(),
            url: "https://cloud.kaneo.app".to_string(),
        };

        open(&handle, &instance).expect("the instance window should open");

        let window = handle
            .get_webview_window(&window_label(&instance.id))
            .expect("the instance window should exist");
        assert_eq!(window.url().unwrap().as_str(), "https://cloud.kaneo.app/");

        open(&handle, &instance).expect("reopening should focus the window");
        assert_eq!(app.webview_windows().len(), 1);
    }
}
