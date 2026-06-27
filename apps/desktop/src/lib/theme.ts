import type { ThemePreference } from "@memora/shared-types";
import { getThemePreference } from "@/lib/api";

function systemPrefersDark(): boolean {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function resolveDarkMode(preference: ThemePreference): boolean {
  if (preference === "dark") return true;
  if (preference === "light") return false;
  return systemPrefersDark();
}

export function applyTheme(preference: ThemePreference): void {
  const root = document.documentElement;
  root.classList.toggle("dark", resolveDarkMode(preference));
  root.dataset.theme = preference;
}

export async function initTheme(): Promise<ThemePreference> {
  const preference = (await getThemePreference()) as ThemePreference;
  applyTheme(preference);
  return preference;
}

export function watchSystemTheme(onChange: () => void): () => void {
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  const handler = () => onChange();
  mq.addEventListener("change", handler);
  return () => mq.removeEventListener("change", handler);
}
