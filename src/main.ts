import {
  addInstance,
  type Instance,
  listInstances,
  openInstance,
  type Probe,
  probeInstance,
  removeInstance,
} from "./api";
import { createThemeEditor } from "./theme/editor";
import { applyActiveThemeToShell, THEME_CHANGED_EVENT } from "./theme/shell";

const CLOUD_URL = "https://cloud.kaneo.app";

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

  info.append(
    createElement("span", "instance__name", instance.name),
    createElement("span", "instance__url", instance.url),
  );

  const probe = probes.get(instance.id);
  if (probe) {
    info.append(createElement("span", "instance__meta", describeProbe(probe)));
  }

  const open = createElement("button", "button button--primary", "Open");
  open.type = "button";
  open.addEventListener("click", () => {
    void openSelected(instance, open);
  });

  const remove = createElement("button", "button button--danger", "Remove");
  remove.type = "button";
  remove.addEventListener("click", () => {
    void removeSelected(instance, remove);
  });

  const actions = createElement("div", "instance__actions");
  actions.append(open, remove);
  item.append(info, actions);

  return item;
}

function render(): void {
  if (instances.length === 0) {
    list.replaceChildren(
      createElement(
        "li",
        "empty",
        "No instances yet. Add Kaneo Cloud, or paste the URL of your own server.",
      ),
    );
    return;
  }

  list.replaceChildren(...instances.map(instanceCard));
}

async function refresh(): Promise<void> {
  instances = await listInstances();
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

const themeEditor = createThemeEditor();
document.body.append(themeEditor.element);
requireElement<HTMLButtonElement>("#themes-button").addEventListener(
  "click",
  () => {
    void themeEditor.open();
  },
);

void applyActiveThemeToShell().catch(() => {
  // A missing or unreadable theme leaves the launcher on its own palette.
});

// The picker announces applies and clears so the welcome screen repaints.
window.addEventListener(THEME_CHANGED_EVENT, () => {
  void applyActiveThemeToShell();
});

render();

void refresh().catch((error) => {
  setStatus("error", errorMessage(error));
});
