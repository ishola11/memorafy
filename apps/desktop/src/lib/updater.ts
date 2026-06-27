import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";

export type UpdateCheckResult =
  | { status: "latest"; message: string }
  | { status: "available"; version: string; message: string }
  | { status: "installed"; version: string; message: string }
  | { status: "unavailable"; message: string }
  | { status: "error"; message: string };

export async function getAppVersion(): Promise<string> {
  return getVersion();
}

export async function checkForUpdates(install = true): Promise<UpdateCheckResult> {
  try {
    const update = await check();
    if (!update) {
      return { status: "latest", message: "You're on the latest version." };
    }

    if (!install) {
      return {
        status: "available",
        version: update.version,
        message: `Version ${update.version} is available.`,
      };
    }

    await update.downloadAndInstall();
    await relaunch();
    return {
      status: "installed",
      version: update.version,
      message: `Updated to ${update.version}. Restarting…`,
    };
  } catch (err) {
    const message = String(err);
    if (message.includes("Could not fetch") || message.includes("pubkey")) {
      return {
        status: "unavailable",
        message: "Updates aren't configured yet. Install a release build to receive updates.",
      };
    }
    return { status: "error", message };
  }
}
