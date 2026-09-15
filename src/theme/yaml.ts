/**
 * Theme files: reading, validating and writing the YAML the picker edits.
 *
 * File access lives in Rust (`list_themes`, `save_theme`, `delete_theme`); this
 * module owns the format, so the schema has exactly one implementation.
 */

import { invoke } from "@tauri-apps/api/core";
import { parse } from "yaml";

import { parseColor } from "./contrast";
import { type Mode, ROLE_KEYS, type ThemeColors } from "./roles";

export type ThemeSource = {
  id: string;
  source: "builtin" | "user";
  contents: string;
  path: string | null;
};

export type ThemeDoc = {
  id: string;
  source: ThemeSource["source"];
  name: string;
  description: string;
  modes: Partial<Record<Mode, ThemeColors>>;
};

export type ActiveThemeFile = { id: string | null; css: string | null };

export const listThemeSources = () => invoke<ThemeSource[]>("list_themes");
export const applyThemeToWindows = (id: string, css: string) =>
  invoke<number>("apply_theme", { id, css });
export const clearThemeFromWindows = () => invoke<number>("clear_theme");
export const readActiveTheme = () => invoke<ActiveThemeFile>("active_theme");
export const saveThemeFile = (id: string, contents: string) =>
  invoke<string>("save_theme", { id, contents });
export const deleteThemeFile = (id: string) =>
  invoke<void>("delete_theme", { id });
export const themesDirectory = () => invoke<string>("themes_dir");

function readColors(
  raw: unknown,
  mode: Mode,
  id: string,
): ThemeColors | undefined {
  if (raw === undefined || raw === null) return undefined;
  if (typeof raw !== "object") {
    throw new Error(`"${id}": ${mode} must be a list of colour roles.`);
  }

  const colors: ThemeColors = {};

  for (const [role, value] of Object.entries(raw as Record<string, unknown>)) {
    if (!ROLE_KEYS.includes(role)) {
      throw new Error(`"${id}": unknown colour role "${role}" in ${mode}.`);
    }
    if (typeof value !== "string" || !parseColor(value)) {
      throw new Error(
        `"${id}": ${mode}.${role} must be a colour like "#52616b".`,
      );
    }

    colors[role] = value;
  }

  return colors;
}

export function parseTheme(source: ThemeSource): ThemeDoc {
  const document = parse(source.contents) as Record<string, unknown> | null;
  if (!document || typeof document !== "object") {
    throw new Error(`"${source.id}" is not a theme document.`);
  }

  const light = readColors(document.light, "light", source.id);
  const dark = readColors(document.dark, "dark", source.id);
  if (!light && !dark) {
    throw new Error(
      `"${source.id}" defines neither a light nor a dark palette.`,
    );
  }

  return {
    id: source.id,
    source: source.source,
    name: typeof document.name === "string" ? document.name : source.id,
    description:
      typeof document.description === "string" ? document.description : "",
    modes: { ...(light ? { light } : {}), ...(dark ? { dark } : {}) },
  };
}

/** Role order matches the template, so saved files stay readable diffs. */
export function serializeTheme(doc: ThemeDoc): string {
  const header = [
    `# ${doc.name}`,
    "#",
    "# Written by the theme editor. Roles left out keep the instance's own value.",
    "",
    `name: ${JSON.stringify(doc.name)}`,
  ];

  if (doc.description)
    header.push(`description: ${JSON.stringify(doc.description)}`);

  const sections = (["light", "dark"] as Mode[])
    .filter((mode) => doc.modes[mode])
    .map((mode) => {
      const colors = doc.modes[mode] as ThemeColors;
      const order = ROLE_KEYS.filter((role) => colors[role]);
      return [
        `${mode}:`,
        ...order.map((role) => `  ${role}: "${colors[role]}"`),
      ].join("\n");
    });

  return `${[...header, "", ...sections].join("\n")}\n`;
}

export function slugify(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64);
}

export type { Mode, ThemeColors };
