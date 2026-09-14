/**
 * The theme editor window: the launcher stays a launcher, theming gets its own
 * place with a way back home — and the Themes menu can switch themes behind it.
 */

import { listen } from "@tauri-apps/api/event";

import { MENU_THEME_EVENT } from "./theme/apply";
import { createThemeEditor } from "./theme/editor";
import { applyActiveThemeToShell, THEME_CHANGED_EVENT } from "./theme/shell";

const editor = createThemeEditor();
document.body.append(editor.element);

// The editor wears the theme it is editing, so what you see is what you get.
window.addEventListener(THEME_CHANGED_EVENT, () => {
  void applyActiveThemeToShell();
});

// The launcher page applies what the menu picks; the editor only has to stop
// claiming a different theme is the applied one.
void listen<{ id: string | null }>(MENU_THEME_EVENT, () => {
  void applyActiveThemeToShell();
  void editor.syncApplied();
}).catch(() => {
  // Nothing to report here: the editor still themes instances, it just cannot
  // follow picks made in the menu bar.
});

void applyActiveThemeToShell();
void editor.refresh();
