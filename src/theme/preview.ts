/**
 * The miniature Kaneo window the editor shows above the colour rows.
 *
 * Everything is drawn from CSS custom properties (`--kd-<role>`), so updating
 * the preview after an edit is one variable write per role — no re-render, and
 * translucent roles composite in the SVG exactly as they do in the app.
 */

import { composite } from "./contrast";
import { type Mode, type ThemeColors, tintAlpha } from "./roles";

const SVG_NS = "http://www.w3.org/2000/svg";

const MARKUP = `
<rect width="360" height="220" style="fill: var(--kd-background)" />
<rect width="86" height="220" style="fill: var(--kd-sidebar)" />
<line x1="86" y1="0" x2="86" y2="220" style="stroke: var(--kd-sidebar-border)" />
<rect x="10" y="14" width="66" height="16" rx="4" style="fill: var(--kd-sidebar-primary)" />
<rect x="16" y="21" width="38" height="3" rx="1.5" style="fill: var(--kd-sidebar-primary-foreground)" />
<rect x="10" y="36" width="66" height="16" rx="4" style="fill: var(--kd-sidebar-accent)" />
<rect x="16" y="43" width="44" height="3" rx="1.5" style="fill: var(--kd-sidebar-accent-foreground)" />
<rect x="16" y="62" width="34" height="3" rx="1.5" style="fill: var(--kd-sidebar-foreground)" />
<rect x="16" y="74" width="40" height="3" rx="1.5" style="fill: var(--kd-sidebar-foreground)" />

<rect x="98" y="12" width="180" height="18" rx="5" style="fill: var(--kd-card); stroke: var(--kd-input)" />
<rect x="106" y="20" width="60" height="3" rx="1.5" style="fill: var(--kd-muted-foreground)" />
<rect x="288" y="12" width="18" height="18" rx="5" style="fill: var(--kd-card); stroke: var(--kd-ring); stroke-width="2" />

<rect x="98" y="40" width="252" height="124" rx="8" style="fill: var(--kd-card); stroke: var(--kd-border)" />
<rect x="110" y="52" width="90" height="6" rx="3" style="fill: var(--kd-foreground)" />
<rect x="110" y="64" width="150" height="4" rx="2" style="fill: var(--kd-muted-foreground)" />
<rect x="110" y="72" width="120" height="4" rx="2" style="fill: var(--kd-muted-foreground)" />

<rect x="110" y="84" width="64" height="18" rx="5" style="fill: var(--kd-primary)" />
<rect x="120" y="91" width="44" height="4" rx="2" style="fill: var(--kd-primary-foreground)" />
<rect x="180" y="84" width="64" height="18" rx="5" style="fill: var(--kd-secondary)" />
<rect x="190" y="91" width="44" height="4" rx="2" style="fill: var(--kd-secondary-foreground)" />
<rect x="250" y="84" width="88" height="18" rx="5" style="fill: var(--kd-destructive)" />
<rect x="260" y="91" width="56" height="4" rx="2" style="fill: #ffffff" />

<rect x="110" y="112" width="44" height="12" rx="6" style="fill: var(--kd-success-tint)" />
<rect x="118" y="116" width="28" height="3" rx="1.5" style="fill: var(--kd-success-foreground)" />
<rect x="160" y="112" width="44" height="12" rx="6" style="fill: var(--kd-warning-tint)" />
<rect x="168" y="116" width="28" height="3" rx="1.5" style="fill: var(--kd-warning-foreground)" />
<rect x="210" y="112" width="44" height="12" rx="6" style="fill: var(--kd-destructive-tint)" />
<rect x="218" y="116" width="28" height="3" rx="1.5" style="fill: var(--kd-destructive-foreground)" />
<rect x="260" y="112" width="44" height="12" rx="6" style="fill: var(--kd-info-tint)" />
<rect x="268" y="116" width="28" height="3" rx="1.5" style="fill: var(--kd-info-foreground)" />

<rect x="110" y="132" width="216" height="20" rx="4" style="fill: var(--kd-code); stroke: var(--kd-border)" />
<rect x="116" y="138" width="26" height="8" rx="2" style="fill: var(--kd-code-highlight)" />
<rect x="148" y="141" width="80" height="4" rx="2" style="fill: var(--kd-code-foreground)" />

<rect x="98" y="172" width="252" height="16" rx="4" style="fill: var(--kd-accent)" />
<rect x="106" y="178" width="60" height="3" rx="1.5" style="fill: var(--kd-accent-foreground)" />

<rect x="98" y="194" width="120" height="16" rx="4" style="fill: var(--kd-muted)" />
<rect x="106" y="200" width="50" height="3" rx="1.5" style="fill: var(--kd-muted-foreground)" />

<rect x="228" y="188" width="122" height="26" rx="6" style="fill: var(--kd-popover); stroke: var(--kd-border)" />
<rect x="236" y="195" width="52" height="4" rx="2" style="fill: var(--kd-popover-foreground)" />
<rect x="236" y="203" width="76" height="3" rx="1.5" style="fill: var(--kd-muted-foreground)" />
`;

export type Preview = {
  element: SVGSVGElement;
  update(colors: ThemeColors, mode: Mode): void;
};

export function createPreview(): Preview {
  const element = document.createElementNS(SVG_NS, "svg");
  element.setAttribute("viewBox", "0 0 360 220");
  element.setAttribute("class", "preview");
  element.setAttribute("role", "img");
  element.setAttribute("aria-label", "Preview of the theme in a Kaneo window");
  element.innerHTML = MARKUP;

  return {
    element,
    update(colors, mode) {
      for (const [role, value] of Object.entries(colors)) {
        element.style.setProperty(`--kd-${role}`, value);
      }

      // Badge tints are what Kaneo renders for `bg-<status>/8` (16% in dark),
      // so the preview shows the composited colour rather than the raw role.
      for (const role of ["success", "warning", "destructive", "info"]) {
        const fill = colors[role];
        const card = colors.card ?? colors.background ?? "#ffffff";
        element.style.setProperty(
          `--kd-${role}-tint`,
          fill ? composite(fill + tintAlpha(mode), card) : "transparent",
        );
      }
    },
  };
}
