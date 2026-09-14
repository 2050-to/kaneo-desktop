/**
 * Wears the active theme in the launcher window, so the welcome screen reflects
 * whatever is applied to the instances.
 *
 * The instance stylesheet is not reused here: it is scoped to Kaneo's light and
 * dark selectors, while the shell wants one palette.
 */

import { shellStylesheet } from "./css";
import { listThemeSources, parseTheme, readActiveTheme } from "./yaml";

const STYLE_ELEMENT_ID = "kaneo-shell-theme";

/** Event the picker fires after applying or clearing a theme. */
export const THEME_CHANGED_EVENT = "kaneo-theme-changed";

function styleElement(): HTMLStyleElement {
  const existing = document.getElementById(STYLE_ELEMENT_ID);
  if (existing instanceof HTMLStyleElement) return existing;

  const element = document.createElement("style");
  element.id = STYLE_ELEMENT_ID;
  document.head.append(element);

  return element;
}

export function clearShellTheme(): void {
  document.getElementById(STYLE_ELEMENT_ID)?.remove();
}

/**
 * Reads what the picker applied and repaints the shell, or falls back to the
 * built-in palette when nothing is applied or the theme file is gone.
 */
export async function applyActiveThemeToShell(): Promise<string | null> {
  const active = await readActiveTheme();
  if (!active.id) {
    clearShellTheme();
    return null;
  }

  const sources = await listThemeSources();
  const source = sources.find((candidate) => candidate.id === active.id);
  if (!source) {
    clearShellTheme();
    return null;
  }

  const doc = parseTheme(source);
  const css = shellStylesheet(doc);
  if (!css) {
    clearShellTheme();
    return null;
  }

  styleElement().textContent = css;
  return doc.name;
}
