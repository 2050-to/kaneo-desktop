import { invoke } from "@tauri-apps/api/core";

export type Instance = {
  id: string;
  name: string;
  url: string;
};

export type Probe = {
  url: string;
  hasUsers: boolean;
  hasAdmin: boolean;
  demoMode: boolean;
  registrationDisabled: boolean;
  signInMethods: string[];
};

export const listInstances = () => invoke<Instance[]>("list_instances");

export const addInstance = (name: string, url: string) =>
  invoke<Instance>("add_instance", { name, url });

export const removeInstance = (id: string) =>
  invoke<void>("remove_instance", { id });

export const probeInstance = (url: string) =>
  invoke<Probe>("probe_instance", { url });

export const openThemesWindow = () => invoke<void>("open_themes_window");

export const goHome = () => invoke<void>("go_home");

export const openInstance = (id: string) =>
  invoke<void>("open_instance", { id });
