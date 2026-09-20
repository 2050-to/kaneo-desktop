/**
 * Turns a theme document into the stylesheet injected into instance windows.
 */

import { type Mode, ROLES } from "./roles";
import type { ThemeDoc } from "./yaml";

const MODES: Mode[] = ["light", "dark"];

/**
 * `:root:not(.dark)` covers light mode only, `:root.dark` dark mode only.
 *
 * Both selectors are (0,2,0), so they outrank Kaneo's own `:root` and `.dark`
 * rules whatever order the stylesheets land in — and because each mode is
 * scoped, a theme that defines only one leaves the other mode on the
 * instance's own palette.
 */
export function themeStylesheet(doc: ThemeDoc): string {
  const blocks: string[] = [];

  for (const mode of MODES) {
    const colors = doc.modes[mode];
    if (!colors) continue;

    const declarations = ROLES.filter((role) => colors[role.key])
      .map((role) => `  --${role.key}: ${colors[role.key]};`)
      .join("\n");

    const selector = mode === "dark" ? ":root.dark" : ":root:not(.dark)";
    blocks.push(`${selector} {\n${declarations}\n}`);
  }

  return [`/* ${doc.name} — applied by Kaneo Desktop */`, ...blocks].join(
    "\n\n",
  );
}
