import { useCallback, useEffect, useState } from "react";
import { Cloud, HardDrive, Loader2, Trash2 } from "lucide-react";
import { clearHistory, previewClearHistory } from "@/lib/api";
import { cn } from "@/lib/utils";
import type { ClearHistoryMode, ClearHistoryPreview, ClearHistoryScope } from "@memora/shared-types";

interface ClearHistorySettingsProps {
  loggedIn: boolean;
  onCleared?: () => void;
}

type PendingAction = {
  scope: ClearHistoryScope;
  mode: ClearHistoryMode;
  count: number;
};

function actionLabel(scope: ClearHistoryScope, mode: ClearHistoryMode, count: number): string {
  const modeLabel = mode === "expired" ? "expired" : "all";
  const scopeLabel = scope === "local" ? "on this device" : "everywhere";
  return `Clear ${count} ${modeLabel} clip${count === 1 ? "" : "s"} ${scopeLabel}`;
}

export function ClearHistorySettings({ loggedIn, onCleared }: ClearHistorySettingsProps) {
  const [preview, setPreview] = useState<ClearHistoryPreview | null>(null);
  const [loading, setLoading] = useState(true);
  const [clearing, setClearing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [pending, setPending] = useState<PendingAction | null>(null);

  const refreshPreview = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const next = await previewClearHistory();
      setPreview(next);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refreshPreview();
  }, [refreshPreview]);

  const requestClear = (scope: ClearHistoryScope, mode: ClearHistoryMode) => {
    if (!preview) return;
    const count = mode === "expired" ? preview.expiredCount : preview.allCount;
    if (count === 0) return;
    setPending({ scope, mode, count });
    setMessage(null);
    setError(null);
  };

  const confirmClear = async () => {
    if (!pending) return;
    setClearing(true);
    setError(null);
    try {
      const result = await clearHistory(pending.scope, pending.mode);
      setPending(null);
      setMessage(
        result.cleared === 0
          ? "Nothing to clear."
          : `Removed ${result.cleared} clip${result.cleared === 1 ? "" : "s"}.`,
      );
      await refreshPreview();
      onCleared?.();
    } catch (err) {
      setError(String(err));
    } finally {
      setClearing(false);
    }
  };

  const expiredDisabled =
    !preview || preview.retentionDays <= 0 || preview.expiredCount === 0 || loading || clearing;
  const allDisabled = !preview || preview.allCount === 0 || loading || clearing;

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-sm font-semibold">Clear history</h3>
        <p className="mt-1 text-sm text-muted">
          Pinned, favorited, snippets, and items in collections are always kept.
        </p>
      </div>

      {loading && !preview ? (
        <div className="flex items-center gap-2 text-sm text-muted">
          <Loader2 className="h-4 w-4 animate-spin" />
          Counting clips…
        </div>
      ) : (
        <div className="space-y-3">
          <section className="rounded-xl border border-border/60 bg-surface-elevated/40 p-4">
            <div className="flex items-start gap-3">
              <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-surface-elevated">
                <HardDrive className="h-4 w-4 text-muted" />
              </div>
              <div className="min-w-0 flex-1">
                <p className="text-sm font-medium">On this device only</p>
                <p className="mt-1 text-xs leading-relaxed text-muted">
                  Removes clips from this Mac or PC. Your cloud account and other devices are not
                  affected.
                </p>
                <div className="mt-3 flex flex-wrap gap-2">
                  <ClearButton
                    label={
                      preview && preview.retentionDays > 0
                        ? `Clear expired locally (${preview.expiredCount})`
                        : "Clear expired locally"
                    }
                    disabled={expiredDisabled}
                    onClick={() => requestClear("local", "expired")}
                  />
                  <ClearButton
                    label={`Clear all locally (${preview?.allCount ?? 0})`}
                    disabled={allDisabled}
                    variant="strong"
                    onClick={() => requestClear("local", "all")}
                  />
                </div>
                {preview && preview.retentionDays <= 0 && (
                  <p className="mt-2 text-xs text-muted">
                    Set a retention period above to enable clearing expired clips.
                  </p>
                )}
              </div>
            </div>
          </section>

          <section
            className={cn(
              "rounded-xl border p-4",
              loggedIn
                ? "border-border/60 bg-surface-elevated/40"
                : "border-border/40 bg-surface-elevated/20 opacity-80",
            )}
          >
            <div className="flex items-start gap-3">
              <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-surface-elevated">
                <Cloud className="h-4 w-4 text-muted" />
              </div>
              <div className="min-w-0 flex-1">
                <p className="text-sm font-medium">Everywhere</p>
                <p className="mt-1 text-xs leading-relaxed text-muted">
                  Deletes from your Memora cloud account. Other signed-in devices remove these clips
                  on their next sync. This cannot be undone.
                </p>
                {!loggedIn ? (
                  <p className="mt-2 text-xs text-amber-700 dark:text-amber-300">
                    Sign in under Account & Sync to use cloud delete.
                  </p>
                ) : (
                  <div className="mt-3 flex flex-wrap gap-2">
                    <ClearButton
                      label={
                        preview && preview.retentionDays > 0
                          ? `Clear expired everywhere (${preview.expiredCount})`
                          : "Clear expired everywhere"
                      }
                      disabled={expiredDisabled || !loggedIn}
                      onClick={() => requestClear("everywhere", "expired")}
                    />
                    <ClearButton
                      label={`Clear all everywhere (${preview?.allCount ?? 0})`}
                      disabled={allDisabled || !loggedIn}
                      variant="danger"
                      onClick={() => requestClear("everywhere", "all")}
                    />
                  </div>
                )}
              </div>
            </div>
          </section>
        </div>
      )}

      {message && (
        <p className="text-sm text-green-600 dark:text-green-400">{message}</p>
      )}
      {error && <p className="text-sm text-red-500">{error}</p>}

      {pending && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4">
          <div
            role="dialog"
            aria-modal="true"
            className="w-full max-w-md rounded-2xl border border-border/60 bg-surface p-6 shadow-xl"
          >
            <div className="flex items-center gap-3">
              <div
                className={cn(
                  "flex h-10 w-10 items-center justify-center rounded-xl",
                  pending.scope === "everywhere"
                    ? "bg-red-500/10 text-red-600 dark:text-red-400"
                    : "bg-amber-500/10 text-amber-700 dark:text-amber-300",
                )}
              >
                <Trash2 className="h-5 w-5" />
              </div>
              <div>
                <h4 className="font-semibold">Confirm clear</h4>
                <p className="text-sm text-muted">
                  {actionLabel(pending.scope, pending.mode, pending.count)}
                </p>
              </div>
            </div>
            <p className="mt-4 text-sm leading-relaxed text-muted">
              {pending.scope === "local" ? (
                <>
                  Clips disappear from this device only. If you are signed in, cloud copies stay
                  available on other devices and will not be re-downloaded here.
                </>
              ) : (
                <>
                  Clips are deleted from your account and will be removed on all devices. This action
                  is permanent.
                </>
              )}
            </p>
            <div className="mt-6 flex justify-end gap-2">
              <button
                type="button"
                disabled={clearing}
                onClick={() => setPending(null)}
                className="rounded-lg border border-border/60 px-4 py-2 text-sm hover:bg-surface-elevated disabled:opacity-50"
              >
                Cancel
              </button>
              <button
                type="button"
                disabled={clearing}
                onClick={() => void confirmClear()}
                className={cn(
                  "flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-medium text-white disabled:opacity-50",
                  pending.scope === "everywhere"
                    ? "bg-red-600 hover:bg-red-700"
                    : "bg-amber-600 hover:bg-amber-700",
                )}
              >
                {clearing && <Loader2 className="h-4 w-4 animate-spin" />}
                {pending.scope === "everywhere" ? "Delete everywhere" : "Clear on this device"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function ClearButton({
  label,
  disabled,
  onClick,
  variant = "default",
}: {
  label: string;
  disabled?: boolean;
  onClick: () => void;
  variant?: "default" | "strong" | "danger";
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "rounded-lg border px-3 py-1.5 text-xs font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-40",
        variant === "danger" &&
          "border-red-500/30 text-red-700 hover:bg-red-500/10 dark:text-red-300",
        variant === "strong" &&
          "border-border/60 hover:bg-surface-elevated",
        variant === "default" &&
          "border-border/60 text-muted hover:bg-surface-elevated hover:text-zinc-800 dark:hover:text-zinc-200",
      )}
    >
      {label}
    </button>
  );
}
