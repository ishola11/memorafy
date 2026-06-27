import { useEffect, useState } from "react";
import {
  Cloud,
  Loader2,
  LogOut,
  Monitor,
  Moon,
  Palette,
  Sun,
  Trash2,
  FolderOpen,
} from "lucide-react";
import {
  authLogin,
  authLogout,
  getAppSettings,
  getCollections,
  getDevices,
  getSyncState,
  setHistoryRetention,
  setThemePreference,
} from "@/lib/api";
import { applyTheme } from "@/lib/theme";
import { cn } from "@/lib/utils";
import { CollectionsSettings } from "@/components/settings/CollectionsSettings";
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

type SettingsSection = "account" | "devices" | "retention" | "collections" | "appearance";

const NAV: { id: SettingsSection; label: string; icon: typeof Cloud }[] = [
  { id: "account", label: "Account & Sync", icon: Cloud },
  { id: "devices", label: "Devices", icon: Monitor },
  { id: "retention", label: "History", icon: Trash2 },
  { id: "collections", label: "Collections", icon: FolderOpen },
  { id: "appearance", label: "Appearance", icon: Palette },
];

export function SettingsPanel() {
  const [section, setSection] = useState<SettingsSection>("account");
  const [state, setState] = useState<SyncState | null>(null);
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [collections, setCollections] = useState<Collection[]>([]);
  const [appSettings, setAppSettings] = useState<AppSettings | null>(null);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
    if (sync.userEmail) setEmail(sync.userEmail);
    applyTheme(settings.themePreference);
  };

  useEffect(() => {
    void refresh();
  }, []);

  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);
    try {
      const sync = await authLogin(email, password);
      setState(sync);
      setPassword("");
      await refresh();
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleLogout = async () => {
    setLoading(true);
    try {
      const sync = await authLogout();
      setState(sync);
      setPassword("");
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
                    onClick={() => void handleLogout()}
                    disabled={loading}
                    className="flex w-full items-center justify-center gap-2 rounded-xl border border-border/60 py-2.5 text-sm hover:bg-surface-elevated"
                  >
                    <LogOut className="h-4 w-4" />
                    Sign out
                  </button>
                </div>
              ) : (
                <form onSubmit={(e) => void handleLogin(e)} className="space-y-4">
                  <div>
                    <label className="mb-1.5 block text-xs text-muted">Email</label>
                    <input
                      type="email"
                      value={email}
                      onChange={(e) => setEmail(e.target.value)}
                      required
                      disabled={!state.configured}
                      className="w-full rounded-lg border border-border/60 bg-surface px-3 py-2.5 text-sm outline-none focus:border-accent/50 focus:ring-1 focus:ring-accent/30"
                    />
                  </div>
                  <div>
                    <label className="mb-1.5 block text-xs text-muted">Password</label>
                    <input
                      type="password"
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      required
                      disabled={!state.configured}
                      className="w-full rounded-lg border border-border/60 bg-surface px-3 py-2.5 text-sm outline-none focus:border-accent/50 focus:ring-1 focus:ring-accent/30"
                    />
                  </div>
                  {error && <p className="text-sm text-red-500">{error}</p>}
                  <button
                    type="submit"
                    disabled={loading || !state.configured}
                    className="flex w-full items-center justify-center gap-2 rounded-xl bg-accent py-2.5 text-sm font-medium text-white hover:bg-accent/90 disabled:opacity-50"
                  >
                    {loading && <Loader2 className="h-4 w-4 animate-spin" />}
                    Sign in to sync
                  </button>
                </form>
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
            <div className="space-y-6">
              <div>
                <h2 className="text-base font-semibold">History retention</h2>
                <p className="mt-1 text-sm text-muted">
                  Auto-remove old clipboard history. Pinned, favorited, snippets, and collection
                  items are always kept.
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

          {error && section !== "account" && (
            <p className="mt-6 text-sm text-red-500">{error}</p>
          )}
        </div>
      </main>
    </div>
  );
}
