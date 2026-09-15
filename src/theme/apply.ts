/**
 * Turns a theme id from the Themes menu into an applied theme. The menu bar
 * knows theme files, not stylesheets, so this is the one place that looks one up
 * and hands it to the instance windows.
 */

import { themeStylesheet } from "./css";
import {
  applyThemeToWindows,
  listThemeSources,
  parseTheme,
  type ThemeDoc,
} from "./yaml";

/**
 * Event the menu bar fires when a theme is picked there. Mirrors `THEME_EVENT`
 * in `src-tauri/src/launcher.rs`; the payload is `{ id: string | null }`, null
 * meaning "stop theming".
 */
export const MENU_THEME_EVENT = "menu-theme-selected";

/** Applies the theme file with this id, built-in or saved. */
export async function applyThemeFile(id: string): Promise<ThemeDoc> {
  const sources = await listThemeSources();
  const source = sources.find((candidate) => candidate.id === id);
  if (!source) {
    throw new Error(`The theme "${id}" is not on disk any more.`);
  }

  const doc = parseTheme(source);
  await applyThemeToWindows(doc.id, themeStylesheet(doc));

  return doc;
}
