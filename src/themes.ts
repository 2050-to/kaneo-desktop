/**
 * The theme editor window: the launcher stays a launcher, theming gets its own
 * place with a way back home.
 */

import { createThemeEditor } from "./theme/editor";
import { applyActiveThemeToShell, THEME_CHANGED_EVENT } from "./theme/shell";

const editor = createThemeEditor();
document.body.append(editor.element);

// The editor wears the theme it is editing, so what you see is what you get.
window.addEventListener(THEME_CHANGED_EVENT, () => {
  void applyActiveThemeToShell();
});

void applyActiveThemeToShell();
void editor.refresh();
