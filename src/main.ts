import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import {
  addInstance,
  getDefaultInstance,
  type Instance,
  listInstances,
  openInstance,
  type Probe,
  probeInstance,
  removeInstance,
  setDefaultInstance,
} from "./api";
import { applyThemeFile, MENU_THEME_EVENT } from "./theme/apply";
import { clearThemeFromWindows } from "./theme/yaml";

const CLOUD_URL = "https://cloud.kaneo.app";

/** The built-in demo runs a fake Kaneo server inside the app, offline. */
const DEMO_INSTANCE: Instance = {
  id: "5dd2f7a9-3c1e-4f2b-9a0d-8e6c4b2a1d03",
  name: "Demo — Example Workspace",
  url: "http://127.0.0.1:41337",
};
const DEMO_EMAIL = "demo@kaneo.desktop";
const DEMO_PASSWORD = "demo-password";

let defaultId: string | null = null;

const list = requireElement<HTMLUListElement>("#instance-list");
const form = requireElement<HTMLFormElement>("#add-form");
const urlInput = requireElement<HTMLInputElement>("#url-input");
const nameInput = requireElement<HTMLInputElement>("#name-input");
const addButton = requireElement<HTMLButtonElement>("#add-button");
const cloudButton = requireElement<HTMLButtonElement>("#cloud-button");
const statusLine = requireElement<HTMLParagraphElement>("#status");

/** Probe results for instances added or checked in this session. */
const probes = new Map<string, Probe>();

let instances: Instance[] = [];
let busy = false;

function requireElement<T extends HTMLElement>(selector: string): T {
  const node = document.querySelector<T>(selector);
  if (!node) throw new Error(`Missing element ${selector}`);
  return node;
}

function createElement<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function setStatus(kind: "ok" | "error" | "busy", message: string): void {
  statusLine.dataset.kind = kind;
  statusLine.textContent = message;
}

/** Rust rejects commands with the message itself, so both shapes can arrive. */
function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Something went wrong.";
}

function describeProbe(probe: Probe): string {
  if (probe.demoMode) return "Demo instance — data is reset regularly";
  if (!probe.hasUsers)
    return "Empty instance — the first account becomes the admin";
  if (probe.signInMethods.length === 0)
    return "This instance hides its sign-in methods";

  const methods = probe.signInMethods.join(", ");
  return probe.registrationDisabled
    ? `Sign in with ${methods} — sign-ups are closed`
    : `Sign in with ${methods}`;
}

function instanceCard(instance: Instance): HTMLLIElement {
  const item = createElement("li", "instance");
  const info = createElement("div", "instance__info");

  const isDefault = defaultId === instance.id;
  if (isDefault) {
    // The card itself carries the launch-on-start mark: an outlined border and
    // a chip, so the mark survives a scan of the list without reading buttons.
    item.classList.add("is-default");
  }

  info.append(
    createElement("span", "instance__name", instance.name),
    createElement("span", "instance__url", instance.url),
  );

  if (isDefault) {
    info.append(createElement("span", "instance__flag", "Opens on start"));
  }

  const probe = probes.get(instance.id);
  if (probe) {
    info.append(createElement("span", "instance__meta", describeProbe(probe)));
  }

  const launchOnStart = createElement("button", "button", "Launch on start");
  launchOnStart.type = "button";
  launchOnStart.setAttribute("aria-pressed", String(isDefault));
  launchOnStart.title = isDefault
    ? "Opens automatically on launch — click to turn off"
    : "Open this instance automatically on launch";
  launchOnStart.addEventListener("click", () => {
    void toggleDefault(instance, launchOnStart);
  });

  const open = createElement("button", "button button--primary", "Open");
  open.type = "button";
  open.addEventListener("click", () => {
    void openSelected(instance, open);
  });

  const remove = createElement(
    "button",
    "button button--destructive",
    "Remove",
  );
  remove.type = "button";
  remove.addEventListener("click", () => {
    void removeSelected(instance, remove);
  });

  const actions = createElement("div", "instance__actions");
  actions.append(launchOnStart, open, remove);
  item.append(info, actions);

  return item;
}

function render(): void {
  const cards = [demoCard(), ...instances.map(instanceCard)];
  list.replaceChildren(...cards);
}

/** The demo is always first: it works with no account and no server. */
function demoCard(): HTMLLIElement {
  const item = createElement("li", "instance is-demo");
  const info = createElement("div", "instance__info");

  info.append(
    createElement("span", "instance__name", DEMO_INSTANCE.name),
    createElement("span", "instance__url", DEMO_INSTANCE.url),
    createElement("span", "instance__flag", "Built-in · offline"),
    createElement(
      "span",
      "instance__meta",
      `Login: ${DEMO_EMAIL} / ${DEMO_PASSWORD}`,
    ),
  );

  const open = createElement("button", "button button--primary", "Open");
  open.type = "button";
  open.addEventListener("click", () => {
    void openDemo(open);
  });

  const actions = createElement("div", "instance__actions");
  actions.append(open);
  item.append(info, actions);

  return item;
}

async function openDemo(button: HTMLButtonElement): Promise<void> {
  button.disabled = true;
  try {
    await openInstance(DEMO_INSTANCE.id);
    setStatus("ok", "Opened the demo workspace.");
  } catch (error) {
    setStatus("error", errorMessage(error));
  } finally {
    button.disabled = false;
  }
}

async function refresh(): Promise<void> {
  instances = await listInstances();
  defaultId = await getDefaultInstance();
  render();
}

async function openSelected(
  instance: Instance,
  button: HTMLButtonElement,
): Promise<void> {
  button.disabled = true;
  try {
    await openInstance(instance.id);
    setStatus("ok", `Opened ${instance.name}.`);
  } catch (error) {
    setStatus("error", errorMessage(error));
  } finally {
    button.disabled = false;
  }
}

/** Flips the launch-on-start mark for one instance. */
async function toggleDefault(
  instance: Instance,
  button: HTMLButtonElement,
): Promise<void> {
  button.disabled = true;
  try {
    const next = defaultId === instance.id ? null : instance.id;
    await setDefaultInstance(next);
    defaultId = next;
    render();
    setStatus(
      "ok",
      next
        ? `${instance.name} will open on launch.`
        : `${instance.name} will not open on launch.`,
    );
  } catch (error) {
    setStatus("error", errorMessage(error));
    button.disabled = false;
  }
}

async function removeSelected(
  instance: Instance,
  button: HTMLButtonElement,
): Promise<void> {
  button.disabled = true;
  try {
    await removeInstance(instance.id);
    probes.delete(instance.id);
    await refresh();
    setStatus("ok", `Removed ${instance.name}.`);
  } catch (error) {
    setStatus("error", errorMessage(error));
    button.disabled = false;
  }
}

/** Verifies the URL against the instance, then stores it. */
async function addFromForm(name: string, url: string): Promise<boolean> {
  setStatus("busy", `Checking ${url}…`);
  try {
    const probe = await probeInstance(url);
    const instance = await addInstance(name, url);
    probes.set(instance.id, probe);
    await refresh();
    setStatus("ok", `${instance.name} added — ${describeProbe(probe)}.`);
    return true;
  } catch (error) {
    setStatus("error", errorMessage(error));
    return false;
  }
}

form.addEventListener("submit", (event) => {
  event.preventDefault();
  if (busy) return;

  const url = urlInput.value.trim();
  if (!url) {
    setStatus("error", "Enter an instance URL.");
    urlInput.focus();
    return;
  }

  busy = true;
  addButton.disabled = true;

  void addFromForm(nameInput.value.trim(), url).then((added) => {
    busy = false;
    addButton.disabled = false;

    if (added) {
      urlInput.value = "";
      nameInput.value = "";
      urlInput.focus();
    }
  });
});

cloudButton.addEventListener("click", () => {
  if (busy) return;

  urlInput.value = CLOUD_URL;
  nameInput.value = "";
  form.requestSubmit();
});

/**
 * The menu bar picks a theme by id; the page owns the rest of the pipeline. A
 * failure comes back with the launcher, which is hidden whenever this runs.
 */
async function applyThemeFromMenu(id: string | null): Promise<void> {
  try {
    if (id === null) {
      await clearThemeFromWindows();
      setStatus("ok", "Theming off — instances use their own palette.");
      return;
    }

    const doc = await applyThemeFile(id);
    setStatus("ok", `${doc.name} applied to your instances.`);
  } catch (error) {
    setStatus("error", errorMessage(error));

    try {
      await getCurrentWindow().show();
    } catch {
      // Coming back into view is a courtesy; the status line already says what
      // went wrong, and a window that refuses to show must not hide that.
    }
  }
}

void listen<{ id: string | null }>(MENU_THEME_EVENT, (event) => {
  void applyThemeFromMenu(event.payload.id);
}).catch((error) => {
  setStatus("error", `Themes menu unavailable: ${errorMessage(error)}`);
});

// The launcher wears Catppuccin — Latte in light system appearance, Frappe in
// dark — on purpose: theming is for instances, and a fixed look here is what
// makes the app recognisable whatever is applied.

render();

void refresh().catch((error) => {
  setStatus("error", errorMessage(error));
});
