/**
 * The 41 colour roles Kaneo's stylesheet defines, plus the contrast checks each
 * one takes part in.
 *
 * The catalogue matches `apps/web/src/index.css` (`:root` and `.dark`) — if
 * upstream adds a role, it belongs here and in the built-in themes.
 */

import { composite, ratio } from "./contrast";

export type Mode = "light" | "dark";
export type ThemeColors = Record<string, string>;

export type RoleGroup =
  | "Surfaces"
  | "Text"
  | "Brand"
  | "Borders and focus"
  | "Status"
  | "Charts";

export type RoleSpec = {
  key: string;
  label: string;
  hint: string;
  group: RoleGroup;
};

export const ROLES: RoleSpec[] = [
  {
    key: "background",
    label: "background",
    hint: "app canvas, behind everything",
    group: "Surfaces",
  },
  { key: "card", label: "card", hint: "cards and panels", group: "Surfaces" },
  {
    key: "popover",
    label: "popover",
    hint: "dialogs, dropdowns, menus, tooltips",
    group: "Surfaces",
  },
  {
    key: "sidebar",
    label: "sidebar",
    hint: "project sidebar canvas",
    group: "Surfaces",
  },
  {
    key: "muted",
    label: "muted",
    hint: "subtle fill: avatars, inner card strips, footers",
    group: "Surfaces",
  },
  {
    key: "accent",
    label: "accent",
    hint: "hover and pressed fill for menus and ghost buttons",
    group: "Surfaces",
  },
  {
    key: "secondary",
    label: "secondary",
    hint: "secondary buttons and segmented controls",
    group: "Surfaces",
  },
  { key: "code", label: "code", hint: "code block surface", group: "Surfaces" },
  {
    key: "code-highlight",
    label: "code-highlight",
    hint: "inline code background in the editor",
    group: "Surfaces",
  },

  {
    key: "foreground",
    label: "foreground",
    hint: "default text",
    group: "Text",
  },
  {
    key: "card-foreground",
    label: "card-foreground",
    hint: "text on cards",
    group: "Text",
  },
  {
    key: "popover-foreground",
    label: "popover-foreground",
    hint: "text in dialogs and menus",
    group: "Text",
  },
  {
    key: "muted-foreground",
    label: "muted-foreground",
    hint: "labels, hints, timestamps, meta",
    group: "Text",
  },
  {
    key: "accent-foreground",
    label: "accent-foreground",
    hint: "text on hover and pressed fills",
    group: "Text",
  },
  {
    key: "secondary-foreground",
    label: "secondary-foreground",
    hint: "text on secondary buttons",
    group: "Text",
  },
  {
    key: "code-foreground",
    label: "code-foreground",
    hint: "text inside code blocks",
    group: "Text",
  },
  {
    key: "sidebar-foreground",
    label: "sidebar-foreground",
    hint: "sidebar labels",
    group: "Text",
  },

  {
    key: "primary",
    label: "primary",
    hint: "filled buttons, selected days — the theme's signature",
    group: "Brand",
  },
  {
    key: "primary-foreground",
    label: "primary-foreground",
    hint: "text and icons on primary",
    group: "Brand",
  },

  {
    key: "border",
    label: "border",
    hint: "default borders and dividers (decorative)",
    group: "Borders and focus",
  },
  {
    key: "input",
    label: "input",
    hint: "input borders; dark surfaces also reuse it as a tint",
    group: "Borders and focus",
  },
  {
    key: "ring",
    label: "ring",
    hint: "keyboard focus ring",
    group: "Borders and focus",
  },
  {
    key: "sidebar-border",
    label: "sidebar-border",
    hint: "sidebar dividers (decorative)",
    group: "Borders and focus",
  },
  {
    key: "sidebar-ring",
    label: "sidebar-ring",
    hint: "sidebar focus ring",
    group: "Borders and focus",
  },
  {
    key: "sidebar-primary",
    label: "sidebar-primary",
    hint: "sidebar active item",
    group: "Borders and focus",
  },
  {
    key: "sidebar-primary-foreground",
    label: "sidebar-primary-foreground",
    hint: "text on the sidebar active item",
    group: "Borders and focus",
  },
  {
    key: "sidebar-accent",
    label: "sidebar-accent",
    hint: "sidebar hover fill",
    group: "Borders and focus",
  },
  {
    key: "sidebar-accent-foreground",
    label: "sidebar-accent-foreground",
    hint: "sidebar hover text",
    group: "Borders and focus",
  },

  {
    key: "destructive",
    label: "destructive",
    hint: "delete, error, danger",
    group: "Status",
  },
  {
    key: "destructive-foreground",
    label: "destructive-foreground",
    hint: "destructive text on tints",
    group: "Status",
  },
  {
    key: "success",
    label: "success",
    hint: "completed and connected states",
    group: "Status",
  },
  {
    key: "success-foreground",
    label: "success-foreground",
    hint: "success text on tints",
    group: "Status",
  },
  { key: "warning", label: "warning", hint: "caution states", group: "Status" },
  {
    key: "warning-foreground",
    label: "warning-foreground",
    hint: "warning text on tints",
    group: "Status",
  },
  {
    key: "info",
    label: "info",
    hint: "neutral notices and status dots",
    group: "Status",
  },
  {
    key: "info-foreground",
    label: "info-foreground",
    hint: "info text on tints",
    group: "Status",
  },

  {
    key: "chart-1",
    label: "chart-1",
    hint: "reserved for data visualisation",
    group: "Charts",
  },
  {
    key: "chart-2",
    label: "chart-2",
    hint: "reserved for data visualisation",
    group: "Charts",
  },
  {
    key: "chart-3",
    label: "chart-3",
    hint: "reserved for data visualisation",
    group: "Charts",
  },
  {
    key: "chart-4",
    label: "chart-4",
    hint: "reserved for data visualisation",
    group: "Charts",
  },
  {
    key: "chart-5",
    label: "chart-5",
    hint: "reserved for data visualisation",
    group: "Charts",
  },
];

export const ROLE_KEYS = ROLES.map((role) => role.key);
export const ROLE_GROUPS: RoleGroup[] = [
  "Surfaces",
  "Text",
  "Brand",
  "Borders and focus",
  "Status",
  "Charts",
];

const STATUS_ROLES = ["destructive", "success", "warning", "info"] as const;

/**
 * `owner` is the role the editor adjusts when a check fails. `compute` reads a
 * theme through `get` plus the status tint for the current mode, so one closure
 * serves the readout, the per-role row and the auto-adjust search alike.
 */
type CheckSpec = {
  owner: string;
  label: string;
  min: number;
  compute: (get: (role: string) => string, tint: string) => number;
};

const CHECKS: CheckSpec[] = [
  {
    owner: "foreground",
    label: "body text on the canvas",
    min: 7,
    compute: (get) => ratio(get("foreground"), get("background")),
  },
  {
    owner: "card-foreground",
    label: "body text on cards",
    min: 7,
    compute: (get) => ratio(get("card-foreground"), get("card")),
  },
  {
    owner: "popover-foreground",
    label: "body text in popovers",
    min: 7,
    compute: (get) => ratio(get("popover-foreground"), get("popover")),
  },
  {
    owner: "code-foreground",
    label: "code text",
    min: 7,
    compute: (get) => ratio(get("code-foreground"), get("code")),
  },
  {
    owner: "muted-foreground",
    label: "muted text on the canvas",
    min: 4.5,
    compute: (get) => ratio(get("muted-foreground"), get("background")),
  },
  {
    owner: "muted-foreground",
    label: "muted text on cards",
    min: 4.5,
    compute: (get) => ratio(get("muted-foreground"), get("card")),
  },
  {
    // Kaneo paints non-sidebar surfaces with bg-sidebar too — the settings
    // layout's TabsList is bg-sidebar while inactive tab labels stay
    // text-muted-foreground — so muted text has to survive there as well.
    owner: "muted-foreground",
    label: "muted text on the sidebar",
    min: 4.5,
    compute: (get) => ratio(get("muted-foreground"), get("sidebar")),
  },
  {
    owner: "accent-foreground",
    label: "menu text on the hover fill",
    min: 4.5,
    compute: (get) =>
      ratio(get("accent-foreground"), composite(get("accent"), get("card"))),
  },
  {
    owner: "secondary-foreground",
    label: "secondary button text",
    min: 4.5,
    compute: (get) =>
      ratio(
        get("secondary-foreground"),
        composite(get("secondary"), get("background")),
      ),
  },
  {
    owner: "sidebar-foreground",
    label: "sidebar text",
    min: 4.5,
    compute: (get) => ratio(get("sidebar-foreground"), get("sidebar")),
  },
  {
    owner: "primary-foreground",
    label: "primary button text",
    min: 4.5,
    compute: (get) => ratio(get("primary-foreground"), get("primary")),
  },
  {
    owner: "primary",
    label: "primary used as text on the canvas",
    min: 3,
    compute: (get) => ratio(get("primary"), get("background")),
  },
  {
    owner: "sidebar-primary-foreground",
    label: "sidebar active text",
    min: 4.5,
    compute: (get) =>
      ratio(get("sidebar-primary-foreground"), get("sidebar-primary")),
  },
  {
    owner: "sidebar-accent-foreground",
    label: "sidebar hover text",
    min: 4.5,
    compute: (get) =>
      ratio(
        get("sidebar-accent-foreground"),
        composite(get("sidebar-accent"), get("sidebar")),
      ),
  },
  {
    owner: "ring",
    label: "focus ring on the canvas",
    min: 3,
    compute: (get) => ratio(get("ring"), get("background")),
  },
  {
    owner: "ring",
    label: "focus ring on cards",
    min: 3,
    compute: (get) => ratio(get("ring"), get("card")),
  },
  {
    owner: "sidebar-ring",
    label: "sidebar focus ring",
    min: 3,
    compute: (get) => ratio(get("sidebar-ring"), get("sidebar")),
  },
  {
    owner: "input",
    label: "input border on the canvas",
    min: 3,
    compute: (get) => ratio(get("input"), get("background")),
  },
  {
    owner: "input",
    label: "input border on cards",
    min: 3,
    compute: (get) => ratio(get("input"), get("card")),
  },

  ...STATUS_ROLES.flatMap((role): CheckSpec[] => [
    {
      owner: role,
      label: `${role} as an icon on cards`,
      min: 3,
      compute: (get) => ratio(get(role), get("card")),
    },
    {
      owner: `${role}-foreground`,
      label: `${role} text on its own tint`,
      min: 4.5,
      compute: (get, tint) =>
        ratio(
          get(`${role}-foreground`),
          composite(get(role) + tint, get("background")),
        ),
    },
    {
      owner: `${role}-foreground`,
      label: `${role} text on cards`,
      min: 4.5,
      compute: (get) => ratio(get(`${role}-foreground`), get("card")),
    },
  ]),

  // Kaneo's delete buttons and notification bubble hardcode white on the fill.
  {
    owner: "destructive",
    label: "white text on destructive buttons",
    min: 4.5,
    compute: (get) => ratio("#ffffff", get("destructive")),
  },
];

export type CheckResult = {
  owner: string;
  label: string;
  min: number;
  value: number;
};

/** Kaneo tints status badges at 8% in light mode and 16% in dark. */
export function tintAlpha(mode: Mode): string {
  return mode === "dark" ? "29" : "14";
}

function read(colors: ThemeColors, role: string): string {
  return colors[role] ?? "#000000";
}

function run(
  colors: ThemeColors,
  tint: string,
  candidate?: { role: string; value: string },
): CheckResult[] {
  const get = (role: string) =>
    candidate && candidate.role === role ? candidate.value : read(colors, role);

  return CHECKS.map((check) => ({
    owner: check.owner,
    label: check.label,
    min: check.min,
    value: check.compute(get, tint),
  }));
}

export function evaluate(colors: ThemeColors, mode: Mode): CheckResult[] {
  return run(colors, tintAlpha(mode));
}

/** The checks a single role is responsible for — the row's own verdicts. */
export function checksFor(
  colors: ThemeColors,
  role: string,
  mode: Mode,
): CheckResult[] {
  return run(colors, tintAlpha(mode)).filter((check) => check.owner === role);
}

/**
 * Worst ratio a role would produce for any candidate value. `meetContrast`
 * maximises this when auto-adjusting, so the search never trades one failing
 * pair for another.
 */
export function measureFor(
  colors: ThemeColors,
  role: string,
  mode: Mode,
): (candidate: string) => number {
  const tint = tintAlpha(mode);

  return (candidate: string) => {
    const owned = run(colors, tint, { role, value: candidate }).filter(
      (check) => check.owner === role,
    );

    return owned.length === 0
      ? 21
      : Math.min(...owned.map((check) => check.value));
  };
}

/** The tightest failing check for a role, if it has one. */
export function failingCheck(
  colors: ThemeColors,
  role: string,
  mode: Mode,
): CheckResult | undefined {
  const failing = checksFor(colors, role, mode).filter(
    (check) => check.value < check.min,
  );
  if (failing.length === 0) return undefined;

  return failing.reduce((a, b) => (a.value / a.min <= b.value / b.min ? a : b));
}

/** Roles in catalogue order, ready to render. */
export function rolesInGroup(group: RoleGroup): RoleSpec[] {
  return ROLES.filter((role) => role.group === group);
}
