import { useEffect, useState } from "react";
import {
  Cloud,
  Loader2,
  LogOut,
  MessageSquare,
  Monitor,
  Moon,
  Palette,
  Power,
  RefreshCw,
  Sun,
  Trash2,
  Wrench,
  FolderOpen,
  Download,
  FileText,
  Info,
} from "lucide-react";
import {
  authLogout,
  eraseAllData,
  forceSyncNow,
  getAppSettings,
  getCollections,
  getDevices,
  getSyncState,
  openLogsDir,
  repairSync,
  setHistoryRetention,
  setLaunchAtLogin,
  setThemePreference,
} from "@/lib/api";
import { checkForUpdates, getAppVersion } from "@/lib/updater";
import { applyTheme } from "@/lib/theme";
import { cn } from "@/lib/utils";
import {
  AuthForms,
  ChangePasswordForm,
  EncryptionLockedCard,
} from "@/components/settings/AuthForms";
import { ClearHistorySettings } from "@/components/settings/ClearHistorySettings";
import { CollectionsSettings } from "@/components/settings/CollectionsSettings";
import { FeedbackSettings } from "@/components/settings/FeedbackSettings";
import type {
  AppSettings,
  Collection,
  DeviceInfo,
  HistoryRetentionOption,
  SyncState,
  ThemePreference,
} from "@memora/shared-types";

const RETENTION_OPTIONS: { value: HistoryRetentionOption; label: string }[] = [
  { value: 0, label: "Never delete" },
  { value: 30, label: "30 days" },
  { value: 60, label: "60 days" },
  { value: 90, label: "90 days" },
];

const THEME_OPTIONS: { value: ThemePreference; label: string; icon: typeof Sun }[] = [
  { value: "system", label: "System", icon: Monitor },
  { value: "light", label: "Light", icon: Sun },
  { value: "dark", label: "Dark", icon: Moon },
];

type SettingsSection =
  | "account"
  | "devices"
  | "retention"
  | "collections"
  | "general"
  | "appearance"
  | "feedback"
  | "about";

const NAV: { id: SettingsSection; label: string; icon: typeof Cloud }[] = [
  { id: "account", label: "Account & Sync", icon: Cloud },
  { id: "devices", label: "Devices", icon: Monitor },
  { id: "retention", label: "History", icon: Trash2 },
  { id: "collections", label: "Collections", icon: FolderOpen },
  { id: "general", label: "General", icon: Power },
  { id: "appearance", label: "Appearance", icon: Palette },
  { id: "feedback", label: "Feedback", icon: MessageSquare },
  { id: "about", label: "About", icon: Info },
];

export function SettingsPanel() {
  const [section, setSection] = useState<SettingsSection>("account");
  const [state, setState] = useState<SyncState | null>(null);
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [collections, setCollections] = useState<Collection[]>([]);
  const [appSettings, setAppSettings] = useState<AppSettings | null>(null);
  const [loading, setLoading] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [repairing, setRepairing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [syncMessage, setSyncMessage] = useState<string | null>(null);
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const [updateMessage, setUpdateMessage] = useState<string | null>(null);
  const [checkingUpdate, setCheckingUpdate] = useState(false);

  useEffect(() => {
    void getAppVersion().then(setAppVersion).catch(() => setAppVersion(null));
  }, []);

  const refresh = async () => {
    const [sync, devs, settings, cols] = await Promise.all([
      getSyncState(),
      getDevices(),
      getAppSettings(),
      getCollections(),
    ]);
    setState(sync);
    setDevices(devs);
    setAppSettings(settings);
    setCollections(cols);
    applyTheme(settings.themePreference);
  };

  useEffect(() => {
    void refresh();
  }, []);

  const handleLogout = async () => {
    setLoading(true);
    try {
      const sync = await authLogout();
      setState(sync);
      setDevices([]);
    } finally {
      setLoading(false);
    }
  };

  const handleRetentionChange = async (days: HistoryRetentionOption) => {
    setLoading(true);
    setError(null);
    try {
      const settings = await setHistoryRetention(days);
      setAppSettings(settings);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleThemeChange = async (preference: ThemePreference) => {
    setLoading(true);
    setError(null);
    try {
      await setThemePreference(preference);
      applyTheme(preference);
      setAppSettings((prev) => (prev ? { ...prev, themePreference: preference } : prev));
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleLaunchAtLoginChange = async (enabled: boolean) => {
    setLoading(true);
    setError(null);
    try {
      await setLaunchAtLogin(enabled);
      setAppSettings((prev) => (prev ? { ...prev, launchAtLogin: enabled } : prev));
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleCheckForUpdates = async () => {
    setCheckingUpdate(true);
    setUpdateMessage(null);
    setError(null);
    try {
      const result = await checkForUpdates(true);
      setUpdateMessage(result.message);
    } catch (err) {
      setError(String(err));
    } finally {
      setCheckingUpdate(false);
    }
  };

  const handleForceSync = async () => {
    setSyncing(true);
    setError(null);
    setSyncMessage(null);
    try {
      const result = await forceSyncNow();
      setState(result);
      setSyncMessage(result.message);
      await refresh();
    } catch (err) {
      setError(String(err));
    } finally {
      setSyncing(false);
    }
  };

  const handleRepairSync = async () => {
    setRepairing(true);
    setError(null);
    setSyncMessage(null);
    try {
      const result = await repairSync();
      setState(result);
      setSyncMessage(result.message);
      await refresh();
    } catch (err) {
      setError(String(err));
    } finally {
      setRepairing(false);
    }
  };

  if (!state) {
    return (
      <div className="flex h-full items-center justify-center bg-surface text-muted">
        <Loader2 className="h-5 w-5 animate-spin" />
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 overflow-hidden bg-surface text-zinc-900 dark:text-zinc-100">
      <aside className="flex w-52 shrink-0 flex-col border-r border-border/60 bg-surface-elevated/30">
        <div className="border-b border-border/60 px-4 py-5">
          <h1 className="text-sm font-semibold tracking-tight">Memora</h1>
          <p className="mt-0.5 text-xs text-muted">Settings</p>
        </div>
        <nav className="flex-1 space-y-0.5 p-2">
          {NAV.map((item) => {
            const Icon = item.icon;
            const active = section === item.id;
            return (
              <button
                key={item.id}
                type="button"
                onClick={() => setSection(item.id)}
                className={cn(
                  "flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-left text-sm transition-colors",
                  active
                    ? "bg-accent/10 font-medium text-accent"
                    : "text-muted hover:bg-surface-elevated hover:text-zinc-800 dark:hover:text-zinc-200",
                )}
              >
                <Icon className="h-4 w-4 shrink-0 opacity-80" />
                {item.label}
              </button>
            );
          })}
        </nav>
      </aside>

      <main className="min-h-0 flex-1 overflow-y-auto overscroll-contain">
        <div className="mx-auto max-w-xl px-8 py-8">
          {section === "account" && (
            <div className="space-y-6">
              <div>
                <h2 className="text-base font-semibold">Account & Sync</h2>
                <p className="mt-1 text-sm text-muted">
                  Sign in to sync clips across your devices.
                </p>
              </div>

              {!state.configured && (
                <div className="rounded-xl border border-amber-500/30 bg-amber-500/10 px-4 py-3 text-sm text-amber-800 dark:text-amber-200">
                  Supabase not configured. Copy <code className="text-xs">.env.example</code> to{" "}
                  <code className="text-xs">apps/desktop/.env</code>.
                </div>
              )}

              {state.loggedIn ? (
                <div className="space-y-4">
                  {state.e2eStatus === "locked" && (
                    <EncryptionLockedCard
                      onResolved={(sync) => {
                        setState(sync);
                        void refresh();
                      }}
                    />
                  )}
                  <div className="rounded-xl border border-border/60 bg-surface-elevated/50 p-4">
                    <div className="flex items-center gap-3">
                      <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-accent/15">
                        <Cloud className="h-5 w-5 text-accent" />
                      </div>
                      <div>
                        <p className="text-xs uppercase tracking-wide text-muted">Signed in</p>
                        <p className="font-medium">{state.userEmail}</p>
                      </div>
                    </div>
                    <div className="mt-3 flex gap-4 text-xs text-muted">
                      <span>{state.pendingCount} pending</span>
                      {state.lastSyncAt && (
                        <span>Last sync {new Date(state.lastSyncAt).toLocaleTimeString()}</span>
                      )}
                    </div>
                  </div>
                  <button
                    type="button"
                    onClick={() => void handleForceSync()}
                    disabled={syncing || repairing || loading}
                    className="flex w-full items-center justify-center gap-2 rounded-xl border border-border/60 bg-surface-elevated/50 py-2.5 text-sm font-medium transition-colors hover:bg-surface-elevated disabled:opacity-50"
                  >
                    {syncing ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <RefreshCw className="h-4 w-4" />
                    )}
                    {syncing ? "Syncing…" : "Sync now"}
                  </button>
                  <button
                    type="button"
                    onClick={() => void handleRepairSync()}
                    disabled={syncing || repairing || loading}
                    className="flex w-full items-center justify-center gap-2 rounded-xl border border-amber-500/30 bg-amber-500/5 py-2.5 text-sm font-medium transition-colors hover:bg-amber-500/10 disabled:opacity-50"
                  >
                    {repairing ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <Wrench className="h-4 w-4" />
                    )}
                    {repairing ? "Repairing…" : "Repair sync"}
                  </button>
                  <p className="text-xs leading-relaxed text-muted">
                    Use Repair sync if sign-in, device registration, or uploads fail. It resets
                    this device in the cloud, clears stuck queue entries, re-downloads your data,
                    and retries uploads.
                  </p>
                  {syncMessage && (
                    <p
                      className={cn(
                        "text-center text-xs",
                        syncMessage.includes("failed") || syncMessage.includes("still pending")
                          ? "text-amber-700 dark:text-amber-300"
                          : "text-green-600 dark:text-green-400",
                      )}
                    >
                      {syncMessage}
                    </p>
                  )}
                  {error && (
                    <p className="text-center text-sm text-red-500">{error}</p>
                  )}
                  <ChangePasswordForm />
                  <button
                    type="button"
                    onClick={() => void handleLogout()}
                    disabled={loading || syncing || repairing}
                    className="flex w-full items-center justify-center gap-2 rounded-xl border border-border/60 py-2.5 text-sm hover:bg-surface-elevated"
                  >
                    <LogOut className="h-4 w-4" />
                    Sign out
                  </button>
                </div>
              ) : (
                <AuthForms
                  configured={state.configured}
                  initialEmail={state.userEmail}
                  onSignedIn={(sync) => {
                    setState(sync);
                    void refresh();
                  }}
                />
              )}
            </div>
          )}

          {section === "devices" && (
            <div className="space-y-6">
              <div>
                <h2 className="text-base font-semibold">Devices</h2>
                <p className="mt-1 text-sm text-muted">Devices linked to your account.</p>
              </div>
              {devices.length === 0 ? (
                <p className="text-sm text-muted">Sign in to see your devices.</p>
              ) : (
                <div className="space-y-2">
                  {devices.map((d) => (
                    <div
                      key={d.id}
                      className="flex items-center justify-between rounded-xl border border-border/60 bg-surface-elevated/40 px-4 py-3 text-sm"
                    >
                      <span>
                        {d.name}
                        {d.isCurrent && (
                          <span className="ml-2 text-xs text-accent">this device</span>
                        )}
                      </span>
                      <span
                        className={cn(
                          "h-2 w-2 rounded-full",
                          d.isOnline ? "bg-green-500" : "bg-zinc-400",
                        )}
                      />
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          {section === "retention" && (
            <div className="space-y-8">
              <div className="space-y-4">
                <div>
                  <h2 className="text-base font-semibold">Auto retention</h2>
                  <p className="mt-1 text-sm text-muted">
                    Automatically remove old clipboard history on a schedule. Pinned, favorited,
                    snippets, and collection items are always kept.
                  </p>
                </div>
                <select
                  value={appSettings?.historyRetentionDays ?? 30}
                  disabled={loading}
                  onChange={(e) =>
                    void handleRetentionChange(Number(e.target.value) as HistoryRetentionOption)
                  }
                  className="w-full rounded-xl border border-border/60 bg-surface px-3 py-2.5 text-sm outline-none focus:border-accent/50"
                >
                  {RETENTION_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>

              <div className="border-t border-border/60 pt-8">
                <ClearHistorySettings
                  loggedIn={state.loggedIn}
                  onCleared={() => void refresh()}
                />
              </div>
            </div>
          )}

          {section === "general" && (
            <div className="space-y-6">
              <div>
                <h2 className="text-base font-semibold">General</h2>
                <p className="mt-1 text-sm text-muted">Startup and system behavior.</p>
              </div>
              <label className="flex cursor-pointer items-center justify-between rounded-xl border border-border/60 bg-surface-elevated/40 px-4 py-3">
                <div>
                  <p className="text-sm font-medium">Launch at login</p>
                  <p className="text-xs text-muted">Start Memora when you sign in to this computer.</p>
                </div>
                <input
                  type="checkbox"
                  checked={appSettings?.launchAtLogin ?? false}
                  disabled={loading}
                  onChange={(e) => void handleLaunchAtLoginChange(e.target.checked)}
                  className="h-4 w-4 rounded border-border accent-accent"
                />
              </label>

              <div className="border-t border-border/60 pt-6">
                <EraseAllDataCard />
              </div>
            </div>
          )}

          {section === "collections" && (
            <CollectionsSettings collections={collections} onChanged={refresh} />
          )}

          {section === "appearance" && (
            <div className="space-y-6">
              <div>
                <h2 className="text-base font-semibold">Appearance</h2>
                <p className="mt-1 text-sm text-muted">
                  Choose how Memora looks. System follows your OS preference.
                </p>
              </div>
              <div className="grid grid-cols-3 gap-2">
                {THEME_OPTIONS.map((opt) => {
                  const Icon = opt.icon;
                  const active = appSettings?.themePreference === opt.value;
                  return (
                    <button
                      key={opt.value}
                      type="button"
                      disabled={loading}
                      onClick={() => void handleThemeChange(opt.value)}
                      className={cn(
                        "flex flex-col items-center gap-2 rounded-xl border px-3 py-4 text-sm transition-colors",
                        active
                          ? "border-accent/50 bg-accent/10 text-accent ring-1 ring-accent/25"
                          : "border-border/60 bg-surface-elevated/40 text-muted hover:border-border hover:text-zinc-800 dark:hover:text-zinc-200",
                      )}
                    >
                      <Icon className="h-5 w-5" />
                      {opt.label}
                    </button>
                  );
                })}
              </div>
            </div>
          )}

          {section === "feedback" && <FeedbackSettings userEmail={state.userEmail} />}

          {section === "about" && (
            <div className="space-y-6">
              <div>
                <h2 className="text-base font-semibold">About Memora</h2>
                <p className="mt-1 text-sm text-muted">
                  Your personal cross-device memory for clipboard history and snippets.
                </p>
              </div>
              <div className="rounded-xl border border-border/60 bg-surface-elevated/50 p-4">
                <p className="text-xs font-medium uppercase tracking-wide text-muted">Version</p>
                <p className="mt-1 text-sm font-medium">{appVersion ?? "…"}</p>
              </div>
              <button
                type="button"
                onClick={() => void handleCheckForUpdates()}
                disabled={checkingUpdate}
                className="flex w-full items-center justify-center gap-2 rounded-xl border border-border/60 bg-surface-elevated/50 py-2.5 text-sm font-medium transition-colors hover:bg-surface-elevated disabled:opacity-50"
              >
                {checkingUpdate ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Download className="h-4 w-4" />
                )}
                {checkingUpdate ? "Checking…" : "Check for updates"}
              </button>
              {updateMessage && (
                <p
                  className={cn(
                    "text-center text-xs",
                    updateMessage.includes("latest") || updateMessage.includes("Restarting")
                      ? "text-green-600 dark:text-green-400"
                      : "text-muted",
                  )}
                >
                  {updateMessage}
                </p>
              )}
              <button
                type="button"
                onClick={() => void openLogsDir().catch(() => setError("Could not open the logs folder."))}
                className="flex w-full items-center justify-center gap-2 rounded-xl border border-border/60 py-2.5 text-sm transition-colors hover:bg-surface-elevated"
              >
                <FileText className="h-4 w-4" />
                Open logs folder
              </button>
              <p className="text-xs text-muted">
                Logs help diagnose sync or capture issues. They stay on this device unless you
                share them.
              </p>
            </div>
          )}

          {error && section !== "account" && (
            <p className="mt-6 text-sm text-red-500">{error}</p>
          )}
        </div>
      </main>
    </div>
  );
}

/** Two-step destructive action: wipe every trace of local data and restart. */
function EraseAllDataCard() {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleErase = async () => {
    setBusy(true);
    setError(null);
    try {
      await eraseAllData(); // restarts the app; only reached on failure
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  };

  return (
    <div className="space-y-3">
      <div>
        <h3 className="text-sm font-semibold text-red-600 dark:text-red-400">Danger zone</h3>
        <p className="mt-1 text-xs leading-relaxed text-muted">
          Erase all local data — clipboard history, snippets, settings, and sign-in — and
          restart Memora as a fresh install. Cloud data is not touched (use History → Clear →
          Everywhere for that). This also prepares the app for a clean uninstall.
        </p>
      </div>
      {!confirming ? (
        <button
          type="button"
          onClick={() => setConfirming(true)}
          className="w-full rounded-xl border border-red-500/40 py-2.5 text-sm text-red-600 transition-colors hover:bg-red-500/10 dark:text-red-400"
        >
          Erase all local data…
        </button>
      ) : (
        <div className="space-y-2 rounded-xl border border-red-500/40 bg-red-500/10 p-4">
          <p className="text-xs font-medium text-red-700 dark:text-red-300">
            This permanently deletes everything Memora stores on this device. Are you sure?
          </p>
          {error && <p className="text-xs text-red-500">{error}</p>}
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => setConfirming(false)}
              disabled={busy}
              className="flex-1 rounded-lg border border-border/60 py-2 text-xs hover:bg-surface-elevated"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={() => void handleErase()}
              disabled={busy}
              className="flex-1 rounded-lg bg-red-600 py-2 text-xs font-medium text-white hover:bg-red-700 disabled:opacity-50"
            >
              {busy ? 'Erasing…' : 'Erase and restart'}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
