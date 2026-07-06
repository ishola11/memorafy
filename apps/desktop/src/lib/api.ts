import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  ClipItem,
  Collection,
  CreateCollectionInput,
  CreateSnippetInput,
  DeviceInfo,
  PreviewCard,
  SearchFilters,
  SyncState,
  ThemePreference,
  TimelineSection,
  AppTab,
  UpdateCollectionInput,
  UpdateSnippetInput,
} from "@memorafy/shared-types";

export async function searchItems(filters: SearchFilters): Promise<PreviewCard[]> {
  return invoke<PreviewCard[]>("search_items", { filters });
}

export async function getTimeline(): Promise<TimelineSection[]> {
  return invoke<TimelineSection[]>("get_timeline");
}

export async function getTabTimeline(
  tab: AppTab,
  collectionId?: string,
): Promise<TimelineSection[]> {
  return invoke<TimelineSection[]>("get_tab_timeline", {
    filters: { tab, collectionId: collectionId ?? null },
  });
}

export async function getClipboardPaused(): Promise<boolean> {
  return invoke<boolean>("get_clipboard_paused");
}

export async function toggleClipboardPause(): Promise<boolean> {
  return invoke<boolean>("toggle_clipboard_pause");
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

export async function createSnippet(input: CreateSnippetInput): Promise<ClipItem> {
  return invoke<ClipItem>("create_snippet", { input });
}

export async function updateSnippet(input: UpdateSnippetInput): Promise<ClipItem> {
  return invoke<ClipItem>("update_snippet", { input });
}

export async function saveItemAsSnippet(id: string): Promise<ClipItem> {
  return invoke<ClipItem>("save_item_as_snippet", { id });
}

export async function getCollections(): Promise<Collection[]> {
  return invoke<Collection[]>("get_collections");
}

export async function createCollection(input: CreateCollectionInput): Promise<Collection> {
  return invoke<Collection>("create_collection", { input });
}

export async function updateCollection(input: UpdateCollectionInput): Promise<Collection> {
  return invoke<Collection>("update_collection", { input });
}

export async function deleteCollection(id: string): Promise<void> {
  return invoke("delete_collection", { id });
}

export async function addItemToCollection(
  itemId: string,
  collectionId: string,
): Promise<void> {
  return invoke("add_item_to_collection", { itemId, collectionId });
}

export async function removeItemFromCollection(
  itemId: string,
  collectionId: string,
): Promise<void> {
  return invoke("remove_item_from_collection", { itemId, collectionId });
}

export async function getItemCollections(itemId: string): Promise<string[]> {
  return invoke<string[]>("get_item_collections", { itemId });
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

export function onCollectionsUpdated(callback: () => void) {
  return listen("collections-updated", callback);
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

export async function authSignup(
  email: string,
  password: string,
): Promise<import("@memorafy/shared-types").SignUpResult> {
  return invoke("auth_signup", { email, password });
}

export async function authResendConfirmation(email: string): Promise<void> {
  return invoke("auth_resend_confirmation", { email });
}

export async function authRequestPasswordReset(email: string): Promise<void> {
  return invoke("auth_request_password_reset", { email });
}

export async function authVerifySignupOtp(
  email: string,
  token: string,
  password: string,
): Promise<SyncState> {
  return invoke<SyncState>("auth_verify_signup_otp", { email, token, password });
}

export async function authVerifyRecoveryOtp(
  email: string,
  token: string,
  newPassword: string,
): Promise<SyncState> {
  return invoke<SyncState>("auth_verify_recovery_otp", { email, token, newPassword });
}

export async function authChangePassword(newPassword: string): Promise<void> {
  return invoke("auth_change_password", { newPassword });
}

export async function unlockSyncEncryption(password: string): Promise<SyncState> {
  return invoke<SyncState>("unlock_sync_encryption", { password });
}

export async function resetSyncEncryption(password: string): Promise<SyncState> {
  return invoke<SyncState>("reset_sync_encryption", { password });
}

export async function getOnboardingCompleted(): Promise<boolean> {
  return invoke("get_onboarding_completed");
}

export async function setOnboardingCompleted(): Promise<void> {
  return invoke("set_onboarding_completed");
}

/** Erases all local data (history, settings, keys) and restarts the app. */
export async function eraseAllData(): Promise<void> {
  return invoke("erase_all_data");
}

export async function forceSyncNow(): Promise<import("@memorafy/shared-types").SyncActionResult> {
  return invoke("force_sync_now");
}

export async function repairSync(): Promise<import("@memorafy/shared-types").SyncRepairResult> {
  return invoke("repair_sync");
}

export async function openSettings(): Promise<void> {
  return invoke("open_settings");
}

export async function openLogsDir(): Promise<void> {
  return invoke("open_logs_dir");
}

export async function getDiagnostics(
  includeLogs: boolean,
): Promise<import("@memorafy/shared-types").Diagnostics> {
  return invoke("get_diagnostics", { includeLogs });
}

export async function submitFeedback(
  report: import("@memorafy/shared-types").FeedbackReport,
): Promise<import("@memorafy/shared-types").FeedbackOutcome> {
  return invoke("submit_feedback", { report });
}

export async function getAppSettings(): Promise<import("@memorafy/shared-types").AppSettings> {
  return invoke("get_app_settings");
}

export async function setHistoryRetention(
  days: import("@memorafy/shared-types").HistoryRetentionOption,
): Promise<import("@memorafy/shared-types").AppSettings> {
  return invoke("set_history_retention", { days });
}

export async function previewClearHistory(): Promise<
  import("@memorafy/shared-types").ClearHistoryPreview
> {
  return invoke("preview_clear_history");
}

export async function clearHistory(
  scope: import("@memorafy/shared-types").ClearHistoryScope,
  mode: import("@memorafy/shared-types").ClearHistoryMode,
): Promise<import("@memorafy/shared-types").ClearHistoryResult> {
  return invoke("clear_history", { scope, mode });
}

export async function setLaunchAtLogin(enabled: boolean): Promise<boolean> {
  return invoke("set_launch_at_login", { enabled });
}

export async function getThemePreference(): Promise<ThemePreference> {
  return invoke<ThemePreference>("get_theme_preference");
}

export async function setThemePreference(preference: ThemePreference): Promise<ThemePreference> {
  return invoke<ThemePreference>("set_theme_preference", { preference });
}

export function onThemeChanged(callback: (preference: ThemePreference) => void) {
  return listen<ThemePreference>("theme-changed", (event) => {
    callback(event.payload);
  });
}

export function onSyncTransfer(callback: (transfer: import("@memorafy/shared-types").SyncTransfer) => void) {
  return listen<import("@memorafy/shared-types").SyncTransfer>("sync-transfer", (event) => {
    callback(event.payload);
  });
}

export function onSyncReceived(callback: (transfer: import("@memorafy/shared-types").SyncTransfer) => void) {
  return listen<import("@memorafy/shared-types").SyncTransfer>("sync-received", (event) => {
    callback(event.payload);
  });
}

/** Fired when a tray-menu "Sync now" finishes, with a human-readable outcome. */
export function onSyncFinished(callback: (message: string) => void) {
  return listen<string>("sync-finished", (event) => {
    callback(event.payload);
  });
}

export function onAuthCallback(
  callback: (result: import("@memorafy/shared-types").AuthCallbackResult) => void,
) {
  return listen<import("@memorafy/shared-types").AuthCallbackResult>("auth-callback", (event) => {
    callback(event.payload);
  });
}

export function onAuthCallbackError(callback: (message: string) => void) {
  return listen<string>("auth-callback-error", (event) => {
    callback(event.payload);
  });
}
