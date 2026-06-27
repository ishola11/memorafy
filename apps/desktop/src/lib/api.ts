import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  ClipItem,
  Collection,
  DeviceInfo,
  PreviewCard,
  SearchFilters,
  SyncState,
  TimelineSection,
} from "@memora/shared-types";

export async function searchItems(filters: SearchFilters): Promise<PreviewCard[]> {
  return invoke<PreviewCard[]>("search_items", { filters });
}

export async function getTimeline(): Promise<TimelineSection[]> {
  return invoke<TimelineSection[]>("get_timeline");
}

export async function copyItem(id: string, plainText = false): Promise<void> {
  return invoke("copy_item", { id, plainText });
}

export async function togglePin(id: string): Promise<void> {
  return invoke("toggle_pin", { id });
}

export async function toggleFavorite(id: string): Promise<void> {
  return invoke("toggle_favorite", { id });
}

export async function deleteItem(id: string): Promise<void> {
  return invoke("delete_item", { id });
}

export async function renameItem(id: string, title: string): Promise<void> {
  return invoke("rename_item", { id, title });
}

export async function getCollections(): Promise<Collection[]> {
  return invoke<Collection[]>("get_collections");
}

export async function getDevices(): Promise<DeviceInfo[]> {
  return invoke<DeviceInfo[]>("get_devices");
}

export async function showQuickPaste(): Promise<void> {
  return invoke("show_quick_paste");
}

export async function hideQuickPaste(): Promise<void> {
  return invoke("hide_quick_paste");
}

export async function getItem(id: string): Promise<ClipItem | null> {
  return invoke<ClipItem | null>("get_item", { id });
}

export function onItemsUpdated(callback: () => void) {
  return listen("items-updated", callback);
}

export function onQuickPasteVisibility(callback: (visible: boolean) => void) {
  return listen<boolean>("quick-paste-visibility", (event) => {
    callback(event.payload);
  });
}

export function onTrayVisibility(callback: (visible: boolean) => void) {
  return listen<boolean>("tray-visibility", (event) => {
    callback(event.payload);
  });
}

export async function getSyncState(): Promise<SyncState> {
  return invoke<SyncState>("get_sync_state");
}

export async function authLogin(email: string, password: string): Promise<SyncState> {
  return invoke<SyncState>("auth_login", { email, password });
}

export async function authLogout(): Promise<SyncState> {
  return invoke<SyncState>("auth_logout");
}

export async function openSettings(): Promise<void> {
  return invoke("open_settings");
}

export async function getAppSettings(): Promise<import("@memora/shared-types").AppSettings> {
  return invoke("get_app_settings");
}

export async function setHistoryRetention(
  days: import("@memora/shared-types").HistoryRetentionOption,
): Promise<import("@memora/shared-types").AppSettings> {
  return invoke("set_history_retention", { days });
}

export function onSyncTransfer(callback: (transfer: import("@memora/shared-types").SyncTransfer) => void) {
  return listen<import("@memora/shared-types").SyncTransfer>("sync-transfer", (event) => {
    callback(event.payload);
  });
}

export function onSyncReceived(callback: (transfer: import("@memora/shared-types").SyncTransfer) => void) {
  return listen<import("@memora/shared-types").SyncTransfer>("sync-received", (event) => {
    callback(event.payload);
  });
}
