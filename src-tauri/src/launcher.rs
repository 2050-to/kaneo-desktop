//! The launcher window's menu: hide it out of the way, bring it back from the
//! menu bar or the Dock icon, and treat closing it as hiding it so it always
//! has a way home.

use tauri::menu::{Menu, MenuItem, Submenu};
use tauri::{AppHandle, Manager, Runtime, WindowEvent};

pub const LAUNCHER_LABEL: &str = "main";
pub const SHOW_LAUNCHER: &str = "launcher-show";
pub const HIDE_LAUNCHER: &str = "launcher-hide";

/// The standard menu plus a Launcher submenu. `Menu::default` is kept so the
/// Edit and Window menus (and their copy/paste shortcuts) survive.
pub fn menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::default(app)?;

    let show = MenuItem::with_id(
        app,
        SHOW_LAUNCHER,
        "Show Launcher",
        true,
        Some("CmdOrCtrl+Shift+L"),
    )?;
    let hide = MenuItem::with_id(
        app,
        HIDE_LAUNCHER,
        "Hide Launcher",
        true,
        Some("CmdOrCtrl+Shift+H"),
    )?;

    let submenu = Submenu::with_items(app, "Launcher", true, &[&show, &hide])?;
    menu.insert(&submenu, 1)?;

    Ok(menu)
}

pub fn show<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(LAUNCHER_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn hide<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(LAUNCHER_LABEL) {
        let _ = window.hide();
    }
}

/// Closing the launcher hides it instead of destroying it — a destroyed window
/// could not be shown again, and the menu item would go dead.
pub fn keep_alive_on_close<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(LAUNCHER_LABEL) else {
        return;
    };

    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            hide(&handle);
        }
    });
}

/// Handles the menu items; returns whether the event belonged to the launcher.
pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, id: &str) -> bool {
    match id {
        SHOW_LAUNCHER => show(app),
        HIDE_LAUNCHER => hide(app),
        _ => return false,
    }

    true
}

#[cfg(test)]
mod tests {
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::WebviewWindowBuilder;

    use super::*;

    fn app_with_launcher() -> tauri::App<tauri::test::MockRuntime> {
        let app = mock_builder()
            .build(mock_context(noop_assets()))
            .expect("failed to build the test app");
        WebviewWindowBuilder::new(&app, LAUNCHER_LABEL, Default::default())
            .build()
            .expect("failed to build the launcher window");

        app
    }

    // The mock runtime does not model window visibility, so what is covered here
    // is the routing contract; the hiding itself is checked by hand in the app.
    #[test]
    fn routes_only_launcher_menu_ids() {
        let app = app_with_launcher();
        let handle = app.handle().clone();

        assert!(handle_menu_event(&handle, HIDE_LAUNCHER));
        assert!(handle_menu_event(&handle, SHOW_LAUNCHER));
        assert!(!handle_menu_event(&handle, "quit"));
        assert!(!handle_menu_event(&handle, "some-other-item"));
    }

    #[test]
    fn survives_a_missing_launcher_window() {
        let app = mock_builder()
            .build(mock_context(noop_assets()))
            .expect("failed to build the test app");
        let handle = app.handle().clone();

        hide(&handle);
        show(&handle);
        assert!(handle_menu_event(&handle, HIDE_LAUNCHER));
        keep_alive_on_close(&handle);
    }
}
