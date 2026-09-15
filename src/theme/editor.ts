/**
 * The theme editor page: pick a theme, edit any of the 41 colour roles, and
 * watch the preview and the contrast verdicts react. Lives in its own window.
 */

import { goHome } from "../api";
import { meetContrast, parseColor } from "./contrast";
import { themeStylesheet } from "./css";
import { createPreview } from "./preview";
import {
  checksFor,
  evaluate,
  failingCheck,
  type Mode,
  measureFor,
  ROLE_GROUPS,
  type RoleSpec,
  rolesInGroup,
  type ThemeColors,
} from "./roles";
import { THEME_CHANGED_EVENT } from "./shell";
import {
  applyThemeToWindows,
  clearThemeFromWindows,
  deleteThemeFile,
  listThemeSources,
  parseTheme,
  readActiveTheme,
  saveThemeFile,
  serializeTheme,
  slugify,
  type ThemeDoc,
  type ThemeSource,
  themesDirectory,
} from "./yaml";

const MODES: Mode[] = ["light", "dark"];

type Row = {
  role: string;
  element: HTMLElement;
  swatch: HTMLInputElement;
  hex: HTMLInputElement;
  ratio: HTMLElement;
  warning: HTMLElement;
  fix: HTMLButtonElement;
};

type State = {
  doc: ThemeDoc | null;
  mode: Mode;
  sources: ThemeSource[];
  rows: Row[];
  dirty: boolean;
  /** Id of the theme currently injected into instance windows. */
  appliedId: string | null;
};

function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function button(label: string, className = "button"): HTMLButtonElement {
  const node = element("button", className, label);
  node.type = "button";
  return node;
}

function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Something went wrong.";
}

/** `#rrggbb` for the native picker; alpha stays in the hex field. */
const opaquePart = (color: string) => color.slice(0, 7);
const alphaPart = (color: string) => (color.length === 9 ? color.slice(7) : "");

export function createThemeEditor(): {
  element: HTMLElement;
  refresh(): Promise<void>;
  syncApplied(): Promise<void>;
} {
  const state: State = {
    doc: null,
    mode: "light",
    sources: [],
    rows: [],
    dirty: false,
    appliedId: null,
  };

  const preview = createPreview();
  const list = element("aside", "editor__list");
  const roleList = element("div", "editor__roles");
  const modeTabs = element("div", "editor__modes");
  const status = element("p", "editor__status");
  const meta = element("span", "editor__meta");
  const nameInput = element("input", "editor__name-input") as HTMLInputElement;
  nameInput.type = "text";
  nameInput.id = "theme-name";
  nameInput.spellcheck = false;
  nameInput.placeholder = "Theme name";
  nameInput.setAttribute("aria-label", "Theme name");

  const saveButton = button("Save", "button button--primary");
  const autoButton = button("Auto-adjust failing");
  const applyButton = button("Apply to instances", "button button--primary");
  const stopButton = button("Stop theming");
  const deleteButton = button("Delete", "button button--danger");

  const backButton = button("← Launcher", "button editor__back");
  backButton.title = "Back to the launcher";

  const root = element("div", "editor");
  const bar = element("header", "editor__bar");
  bar.append(backButton, element("h2", "editor__title", "Themes"), modeTabs);

  // Actions live in their own column so every button is the same size.
  const actions = element("aside", "editor__actions");
  actions.append(autoButton, applyButton, saveButton, stopButton, deleteButton);

  const nameRow = element("div", "editor__name");
  nameRow.append(
    element("span", "editor__name-label", "Name"),
    nameInput,
    meta,
  );

  const previewWrap = element("div", "editor__preview");
  previewWrap.append(preview.element);

  const pane = element("section", "editor__pane");
  pane.append(previewWrap, nameRow, roleList);

  const body = element("div", "editor__body");
  body.append(list, pane, actions);

  root.append(bar, body, status);

  function setStatus(
    message: string,
    kind: "info" | "error" | "ok" = "info",
  ): void {
    status.textContent = message;
    status.dataset.kind = kind;
  }

  function colors(): ThemeColors {
    return state.doc?.modes[state.mode] ?? {};
  }

  function refreshVerdicts(): void {
    const palette = colors();
    const failingRoles: string[] = [];

    for (const row of state.rows) {
      const checks = checksFor(palette, row.role, state.mode);

      if (checks.length === 0) {
        row.ratio.textContent = "—";
        row.ratio.className = "role__ratio";
        row.ratio.removeAttribute("title");
        row.warning.textContent = "";
        row.fix.hidden = true;
        continue;
      }

      const worst = checks.reduce((a, b) =>
        a.value / a.min <= b.value / b.min ? a : b,
      );
      const ok = worst.value >= worst.min;

      row.ratio.textContent = `${worst.value.toFixed(2)}:1 / ${worst.min}`;
      row.ratio.className = `role__ratio ${ok ? "is-pass" : "is-fail"}`;
      row.ratio.title = worst.label;
      row.warning.textContent = ok ? "" : "may need more contrast";
      row.fix.hidden = ok;

      if (!ok) failingRoles.push(row.role);

      const current = palette[row.role];
      if (current && document.activeElement !== row.hex) {
        row.hex.value = current;
        row.swatch.value = opaquePart(current);
      }
    }

    const failingChecks = evaluate(palette, state.mode).filter(
      (check) => check.value < check.min,
    ).length;

    if (failingRoles.length > 0) {
      const shown = failingRoles.slice(0, 6).join(", ");
      setStatus(
        `${failingRoles.length} of ${state.rows.length} roles may need more contrast — ${shown}${failingRoles.length > 6 ? ", …" : ""} (${failingChecks} failing checks)`,
        "error",
      );
      return;
    }

    setStatus(
      state.dirty
        ? "All contrast checks pass. Unsaved changes."
        : "All contrast checks pass.",
      state.dirty ? "info" : "ok",
    );
  }

  function applyRole(role: string, value: string): void {
    const palette = colors();
    palette[role] = value;
    state.dirty = true;
    preview.update(palette, state.mode);
    refreshVerdicts();
  }

  function fixRole(row: Row): boolean {
    const palette = colors();
    const current = palette[row.role];
    if (!current) return false;

    const checks = checksFor(palette, row.role, state.mode);
    if (checks.length === 0) return false;

    const target = Math.max(...checks.map((check) => check.min));
    const adjusted = meetContrast(
      current,
      target,
      measureFor(palette, row.role, state.mode),
    );

    if (adjusted === current) return false;
    applyRole(row.role, adjusted);
    return true;
  }

  function buildRow(spec: RoleSpec, palette: ThemeColors): Row {
    const container = element("div", "role");
    container.dataset.role = spec.key;
    const info = element("div", "role__info");
    info.append(
      element("span", "role__label", spec.label),
      element("span", "role__hint", spec.hint),
    );

    const swatch = element("input", "role__swatch") as HTMLInputElement;
    swatch.type = "color";
    swatch.value = opaquePart(palette[spec.key] ?? "#000000");
    swatch.title = "Pick a colour";

    const hex = element("input", "role__hex") as HTMLInputElement;
    hex.type = "text";
    hex.spellcheck = false;
    hex.value = palette[spec.key] ?? "";

    const ratio = element("span", "role__ratio");
    const warning = element("span", "role__warning");
    const fix = button("Fix", "role__fix");
    fix.title = "Adjust this colour until its contrast checks pass";

    const verdict = element("div", "role__verdict");
    verdict.append(ratio, warning, fix);

    swatch.addEventListener("input", () => {
      applyRole(spec.key, swatch.value + alphaPart(hex.value || ""));
    });

    hex.addEventListener("input", () => {
      const value = hex.value.trim();
      if (!parseColor(value)) {
        hex.classList.add("is-invalid");
        return;
      }

      hex.classList.remove("is-invalid");
      applyRole(spec.key, value);
      swatch.value = opaquePart(value);
    });

    const row: Row = {
      role: spec.key,
      element: container,
      swatch,
      hex,
      ratio,
      warning,
      fix,
    };

    fix.addEventListener("click", () => {
      if (!fixRole(row)) {
        setStatus(`No adjustment improves ${spec.label} further.`, "error");
      }
    });

    container.append(info, swatch, hex, verdict);

    return row;
  }

  function renderRoles(): void {
    const palette = colors();
    roleList.replaceChildren();
    state.rows = [];

    for (const group of ROLE_GROUPS) {
      roleList.append(element("h3", "role__group", group));
      for (const spec of rolesInGroup(group)) {
        const row = buildRow(spec, palette);
        state.rows.push(row);
        roleList.append(row.element);
      }
    }

    preview.update(palette, state.mode);
  }

  function renderModes(): void {
    modeTabs.replaceChildren();

    for (const mode of MODES) {
      const available = Boolean(state.doc?.modes[mode]);
      const tab = button(
        mode === "light" ? "Light" : "Dark",
        "button editor__mode",
      );
      tab.disabled = !available;
      tab.classList.toggle("is-active", state.mode === mode);

      tab.addEventListener("click", () => {
        state.mode = mode;
        renderModes();
        renderRoles();
        refreshVerdicts();
      });

      modeTabs.append(tab);
    }
  }

  function renderList(): void {
    list.replaceChildren();

    for (const source of state.sources) {
      const entry = button(source.id, "editor__theme");
      entry.classList.toggle("is-active", state.doc?.id === source.id);

      const applied = source.id === state.appliedId;
      entry.append(
        element(
          "span",
          `editor__theme-source${applied ? " is-applied" : ""}`,
          applied
            ? "applied"
            : source.source === "builtin"
              ? "built-in"
              : "saved",
        ),
      );
      entry.addEventListener("click", () => selectSource(source));
      list.append(entry);
    }
  }

  function selectSource(source: ThemeSource): void {
    let doc: ThemeDoc;
    try {
      doc = parseTheme(source);
    } catch (error) {
      setStatus(errorMessage(error), "error");
      return;
    }

    state.doc = doc;
    state.mode = MODES.find((mode) => doc.modes[mode]) ?? "light";
    state.dirty = false;

    nameInput.value = doc.name;
    deleteButton.disabled = doc.source !== "user";
    deleteButton.textContent = "Delete";
    meta.textContent =
      doc.source === "builtin"
        ? "Built-in theme"
        : `Saved theme · ${source.path ?? ""}`;

    renderList();
    renderModes();
    renderRoles();
    refreshVerdicts();
  }

  async function refreshSources(): Promise<void> {
    state.sources = await listThemeSources();
    renderList();
  }

  async function save(): Promise<void> {
    if (!state.doc) return;

    const name = nameInput.value.trim() || state.doc.name;
    const id = state.doc.source === "user" ? state.doc.id : slugify(name);

    if (!id) {
      setStatus("Give the theme a name with letters or digits.", "error");
      return;
    }

    try {
      const path = await saveThemeFile(
        id,
        serializeTheme({ ...state.doc, id, name }),
      );
      state.dirty = false;
      await refreshSources();
      const saved = state.sources.find((source) => source.id === id);
      if (saved) selectSource(saved);
      setStatus(`Saved to ${path}`, "ok");
    } catch (error) {
      setStatus(errorMessage(error), "error");
    }
  }

  async function remove(): Promise<void> {
    const doc = state.doc;
    if (doc?.source !== "user") return;

    if (deleteButton.textContent !== "Confirm delete") {
      deleteButton.textContent = "Confirm delete";
      setStatus(`Click again to delete "${doc.name}" from disk.`);
      return;
    }

    try {
      await deleteThemeFile(doc.id);
      state.doc = null;
      await refreshSources();
      if (state.sources.length > 0) selectSource(state.sources[0]);
      setStatus(`Deleted "${doc.name}".`, "ok");
    } catch (error) {
      setStatus(errorMessage(error), "error");
    }
  }

  autoButton.addEventListener("click", () => {
    let changed = 0;

    // A fix can change another role's verdict (e.g. text against a surface), so
    // sweep until the palette settles.
    for (let pass = 0; pass < 3; pass += 1) {
      const failing = state.rows.filter((row) =>
        failingCheck(colors(), row.role, state.mode),
      );
      if (failing.length === 0) break;
      for (const row of failing) {
        if (fixRole(row)) changed += 1;
      }
    }

    refreshVerdicts();
    if (changed === 0)
      setStatus("Nothing could be improved automatically.", "error");
  });

  async function apply(): Promise<void> {
    const doc = state.doc;
    if (!doc) return;

    const name = nameInput.value.trim() || doc.name;
    try {
      const css = themeStylesheet({ ...doc, name });
      const windows = await applyThemeToWindows(doc.id, css);
      await syncApplied();
      window.dispatchEvent(new Event(THEME_CHANGED_EVENT));
      setStatus(
        windows > 0
          ? `Applied "${name}" to ${windows} open window${windows === 1 ? "" : "s"}.`
          : `Applied "${name}". Instances you open from now on will use it.`,
        "ok",
      );
    } catch (error) {
      setStatus(errorMessage(error), "error");
    }
  }

  async function stop(): Promise<void> {
    try {
      const windows = await clearThemeFromWindows();
      await syncApplied();
      window.dispatchEvent(new Event(THEME_CHANGED_EVENT));
      setStatus(
        windows > 0
          ? `Theme removed from ${windows} open window${windows === 1 ? "" : "s"}.`
          : "Theming is off.",
        "ok",
      );
    } catch (error) {
      setStatus(errorMessage(error), "error");
    }
  }

  applyButton.addEventListener("click", () => void apply());
  stopButton.addEventListener("click", () => void stop());
  saveButton.addEventListener("click", () => void save());
  deleteButton.addEventListener("click", () => void remove());
  backButton.addEventListener("click", () => {
    void goHome();
  });
  nameInput.addEventListener("input", () => {
    state.dirty = true;
  });

  /** Reads what is applied; a theme file that cannot be read is not a failure. */
  async function readApplied(): Promise<string | null> {
    try {
      const active = await readActiveTheme();
      return active.id;
    } catch {
      return null;
    }
  }

  /**
   * Re-reads the applied theme without touching the selection, so unsaved edits
   * survive. Used when the Themes menu switched themes behind the editor.
   */
  async function syncApplied(): Promise<void> {
    state.appliedId = await readApplied();
    stopButton.disabled = state.appliedId === null;
    renderList();
  }

  return {
    element: root,
    syncApplied,
    async refresh() {
      try {
        await refreshSources();
        await syncApplied();

        const current = state.sources.find(
          (source) => source.id === state.doc?.id,
        );
        if (current) selectSource(current);
        else if (state.sources.length > 0) selectSource(state.sources[0]);

        const directory = await themesDirectory();
        meta.title = `Themes are saved to ${directory}`;
      } catch (error) {
        setStatus(errorMessage(error), "error");
      }
    },
  };
}
