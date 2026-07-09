import { useEffect, useState } from "react";
import { Download, Loader2, X } from "lucide-react";
import { onUpdateAvailable } from "@/lib/api";
import { checkForUpdates } from "@/lib/updater";

const DISMISSED_VERSION_KEY = "memorafy:dismissedUpdateVersion";

function isDismissed(version: string): boolean {
  try {
    return localStorage.getItem(DISMISSED_VERSION_KEY) === version;
  } catch {
    return false;
  }
}

function dismissVersion(version: string) {
  try {
    localStorage.setItem(DISMISSED_VERSION_KEY, version);
  } catch {
    // ignore quota / private-mode errors
  }
}

export function UpdateToast() {
  const [version, setVersion] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void onUpdateAvailable((payload) => {
      if (isDismissed(payload.version)) return;
      setVersion(payload.version);
      setError(null);
    }).then((unlisten) => unlisten);
  }, []);

  if (!version) return null;

  const handleLater = () => {
    dismissVersion(version);
    setVersion(null);
    setError(null);
  };

  const handleInstall = async () => {
    setInstalling(true);
    setError(null);
    try {
      const result = await checkForUpdates(true);
      if (result.status === "error") {
        setError(result.message);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setInstalling(false);
    }
  };

  return (
    <div className="pointer-events-auto fixed bottom-20 left-6 z-[120] animate-in fade-in slide-in-from-bottom-2">
      <div className="w-80 rounded-xl border border-accent/30 bg-zinc-950/95 p-4 shadow-2xl backdrop-blur-xl">
        <div className="flex items-start gap-3">
          <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-accent/20">
            <Download className="h-4 w-4 text-accent" />
          </div>
          <div className="min-w-0 flex-1">
            <p className="text-sm font-medium text-zinc-100">Update available</p>
            <p className="mt-0.5 text-xs text-zinc-400">
              Memorafy v{version} is ready to install.
            </p>
            {error && <p className="mt-2 text-xs text-red-400">{error}</p>}
            <div className="mt-3 flex items-center gap-2">
              <button
                type="button"
                disabled={installing}
                onClick={() => void handleInstall()}
                className="rounded-md bg-accent px-3 py-1.5 text-xs font-medium text-white transition hover:bg-accent/90 disabled:opacity-60"
              >
                {installing ? (
                  <span className="flex items-center gap-1.5">
                    <Loader2 className="h-3 w-3 animate-spin" />
                    Installing…
                  </span>
                ) : (
                  "Update now"
                )}
              </button>
              <button
                type="button"
                disabled={installing}
                onClick={handleLater}
                className="rounded-md px-3 py-1.5 text-xs font-medium text-zinc-400 transition hover:text-zinc-200 disabled:opacity-60"
              >
                Later
              </button>
            </div>
          </div>
          <button
            type="button"
            disabled={installing}
            onClick={handleLater}
            className="shrink-0 rounded p-0.5 text-zinc-500 transition hover:text-zinc-300 disabled:opacity-60"
            aria-label="Dismiss"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
      </div>
    </div>
  );
}
