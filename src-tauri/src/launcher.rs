//! The launcher window's menu: hide it out of the way, bring it back from the
//! menu bar or the Dock icon, treat closing it as hiding it so it always has a
//! way home, and switch themes without a window in the way.

use serde::Serialize;
use tauri::menu::{
    CheckMenuItem, IsMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu,
};
use tauri::{AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::themes::ThemeSource;

pub const LAUNCHER_LABEL: &str = "main";
pub const THEMES_LABEL: &str = "themes";
pub const SHOW_LAUNCHER: &str = "launcher-show";
pub const HIDE_LAUNCHER: &str = "launcher-hide";
pub const OPEN_THEMES: &str = "themes-open";

/// A theme's menu item is this prefix and the theme id, so one id space carries
/// both the file and the item that picks it.
const APPLY_THEME: &str = "themes-apply:";

/// Emitted when the menu picks a theme. The menu bar knows theme files, not
/// stylesheets, so it names one and lets the windows parse and apply it.
pub const THEME_EVENT: &str = "menu-theme-selected";

/// Payload of [`THEME_EVENT`]; a missing id means "stop theming".
#[derive(Clone, Serialize)]
pub struct ThemeRequest {
    pub id: Option<String>,
}

/// The standard menu plus a Launcher submenu and a Themes submenu listing every
/// theme on disk. `Menu::default` is kept so the Edit and Window menus (and
/// their copy/paste shortcuts) survive.
pub fn menu<R: Runtime>(
    app: &AppHandle<R>,
    themes: &[ThemeSource],
    active: Option<&str>,
) -> tauri::Result<Menu<R>> {
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
    menu.insert(&themes_menu(app, themes, active)?, 2)?;

    Ok(menu)
}

/// The theme list is fixed at launch: rebuilding a native menu is not worth it
/// when `sync_theme_checks` can keep the tick honest instead.
fn themes_menu<R: Runtime>(
    app: &AppHandle<R>,
    themes: &[ThemeSource],
    active: Option<&str>,
) -> tauri::Result<Submenu<R>> {
    let choices = themes
        .iter()
        .map(|theme| {
            CheckMenuItem::with_id(
                app,
                theme_menu_id(&theme.id),
                theme_label(&theme.id),
                true,
                active == Some(theme.id.as_str()),
                None::<&str>,
            )
        })
        .collect::<tauri::Result<Vec<_>>>()?;

    let editor = MenuItem::with_id(app, OPEN_THEMES, "Theme Editor…", true, Some("CmdOrCtrl+,"))?;
    let before_editor = PredefinedMenuItem::separator(app)?;

    let mut items: Vec<&dyn IsMenuItem<R>> = choices
        .iter()
        .map(|item| item as &dyn IsMenuItem<R>)
        .collect();
    items.extend([&before_editor as &dyn IsMenuItem<R>, &editor]);

    Submenu::with_items(app, "Themes", true, &items)
}

fn theme_menu_id(id: &str) -> String {
    format!("{APPLY_THEME}{id}")
}

/// "metallic-sky" reads as "Metallic Sky" in a menu. Ids are slugs of a theme's
/// own name and Rust owns no YAML parser, so the id is all there is to go on.
fn theme_label(id: &str) -> String {
    id.split('-')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut letters = word.chars();
            match letters.next() {
                Some(first) => first.to_uppercase().chain(letters).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

/// Moves the tick to the theme that is applied. Called after any change, so the
/// menu never claims a theme that is not the one in use.
pub fn sync_theme_checks<R: Runtime>(app: &AppHandle<R>, active: Option<&str>) {
    let Some(menu) = app.menu() else {
        return;
    };
    let Ok(items) = menu.items() else {
        return;
    };

    for item in items {
        let MenuItemKind::Submenu(submenu) = item else {
            continue;
        };
        let Ok(children) = submenu.items() else {
            continue;
        };

        for child in children {
            let MenuItemKind::Check(item) = child else {
                continue;
            };

            if let Some(theme) = item.id().as_ref().strip_prefix(APPLY_THEME) {
                let _ = item.set_checked(Some(theme) == active);
            }
        }
    }
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

/// The theme editor lives in its own window, so theming is not something the
/// launcher has to stay open for.
pub fn open_themes<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(THEMES_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    WebviewWindowBuilder::new(app, THEMES_LABEL, WebviewUrl::App("themes.html".into()))
        .title("Themes")
        .inner_size(1180.0, 800.0)
        .min_inner_size(900.0, 600.0)
        .center()
        .devtools(cfg!(debug_assertions))
        .build()
        .map_err(|error| format!("Could not open the theme editor: {error}"))?;

    Ok(())
}

/// Back to the launcher: show it, then close the editor window.
pub fn go_home<R: Runtime>(app: &AppHandle<R>) {
    show(app);

    if let Some(window) = app.get_webview_window(THEMES_LABEL) {
        let _ = window.close();
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
        OPEN_THEMES => {
            let _ = open_themes(app);
        }
        _ => match id.strip_prefix(APPLY_THEME) {
            Some(theme) => select_theme(app, Some(theme)),
            None => return false,
        },
    }

    true
}

/// Names a theme to every window that cares: the launcher page applies it, and
/// the editor follows along. A window that is mid-teardown can refuse the
/// event, so nothing here waits on the outcome.
fn select_theme<R: Runtime>(app: &AppHandle<R>, id: Option<&str>) {
    let request = ThemeRequest {
        id: id.map(str::to_string),
    };
    let _ = app.emit(THEME_EVENT, request);
}

#[cfg(test)]
mod tests {
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::{Listener, WebviewWindowBuilder};

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
    fn routes_menu_ids_it_owns() {
        let app = app_with_launcher();
        let handle = app.handle().clone();

        assert!(handle_menu_event(&handle, HIDE_LAUNCHER));
        assert!(handle_menu_event(&handle, SHOW_LAUNCHER));
        assert!(handle_menu_event(&handle, OPEN_THEMES));
        assert!(!handle_menu_event(&handle, "quit"));
        assert!(!handle_menu_event(&handle, "some-other-item"));
    }

    /// The launcher page is the half that turns a theme id into CSS, so what the
    /// menu hands it — event name and payload shape — is the contract to pin.
    #[test]
    fn menu_theme_picks_reach_the_windows() {
        let app = app_with_launcher();
        let handle = app.handle().clone();
        let (sender, receiver) = std::sync::mpsc::channel();

        handle.listen(THEME_EVENT, move |event| {
            let _ = sender.send(event.payload().to_string());
        });

        assert!(handle_menu_event(&handle, &theme_menu_id("synthwave")));
        assert_eq!(
            receiver
                .recv_timeout(std::time::Duration::from_secs(1))
                .expect("picking a theme should emit"),
            r#"{"id":"synthwave"}"#
        );

        // A theme id is anything after the prefix, hyphens and all.
        assert!(handle_menu_event(&handle, &theme_menu_id("metallic-sky")));
        assert_eq!(
            receiver
                .recv_timeout(std::time::Duration::from_secs(1))
                .expect("picking a hyphenated theme should emit"),
            r#"{"id":"metallic-sky"}"#
        );

        assert!(!handle_menu_event(&handle, "themes-apply"));
        assert!(!handle_menu_event(&handle, "other-apply:synthwave"));
        assert!(!handle_menu_event(&handle, "themes-stop"));
    }

    #[test]
    fn menu_labels_read_as_names() {
        assert_eq!(theme_label("metallic-sky"), "Metallic Sky");
        assert_eq!(theme_label("grey"), "Grey");
        assert_eq!(theme_label("default"), "Default");
        assert_eq!(theme_label("my--theme"), "My Theme");
        assert_eq!(theme_label(""), "");
    }

    /// The tick has to be movable in the app; without a menu there is nothing to
    /// move and it must not panic.
    #[test]
    fn syncing_checks_without_a_menu_is_a_no_op() {
        let app = app_with_launcher();
        let handle = app.handle().clone();

        sync_theme_checks(&handle, Some("synthwave"));
        sync_theme_checks(&handle, None);
    }

    #[test]
    fn opens_one_themes_window() {
        let app = app_with_launcher();
        let handle = app.handle().clone();

        open_themes(&handle).expect("the editor window should open");
        let window = handle
            .get_webview_window(THEMES_LABEL)
            .expect("the editor window should exist");
        assert_eq!(window.url().unwrap().path(), "/themes.html");

        open_themes(&handle).expect("reopening should focus it");
        assert_eq!(app.webview_windows().len(), 2, "launcher plus editor");
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
