import { useEffect, useState } from "react";
import { Check, Cloud, Download } from "lucide-react";
import { onSyncReceived, onSyncTransfer } from "@/lib/api";
import type { SyncTransfer } from "@memorafy/shared-types";

type ToastKind = "sent" | "received";

export function SyncToast() {
  const [toast, setToast] = useState<(SyncTransfer & { kind: ToastKind }) | null>(null);

  useEffect(() => {
    let hideTimer: ReturnType<typeof setTimeout> | undefined;

    const show = (kind: ToastKind, transfer: SyncTransfer) => {
      setToast({ ...transfer, kind });
      if (hideTimer) clearTimeout(hideTimer);
      hideTimer = setTimeout(() => setToast(null), 3500);
    };

    void Promise.all([
      onSyncTransfer((transfer) => show("sent", transfer)),
      onSyncReceived((transfer) => show("received", transfer)),
    ]).then(([unlistenSent, unlistenReceived]) => () => {
      if (hideTimer) clearTimeout(hideTimer);
      unlistenSent();
      unlistenReceived();
    });
  }, []);

  if (!toast) return null;

  const isReceived = toast.kind === "received";

  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-[100] animate-in fade-in slide-in-from-bottom-2">
      <div className="w-80 rounded-xl border border-white/10 bg-zinc-950/95 p-4 shadow-2xl backdrop-blur-xl">
        <div className="flex items-start gap-3">
          <div
            className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-lg ${
              isReceived ? "bg-blue-500/20" : "bg-green-500/20"
            }`}
          >
            {isReceived ? (
              <Download className="h-4 w-4 text-blue-400" />
            ) : (
              <Cloud className="h-4 w-4 text-green-400" />
            )}
          </div>
          <div className="min-w-0 flex-1">
            <p className="text-sm font-medium text-zinc-100">
              {isReceived ? `Received from ${toast.sourceDevice}` : "Synced to cloud"}
            </p>
            <p className="truncate text-xs text-zinc-400">{toast.title}</p>
            {!isReceived && toast.onlineDevices.length > 0 && (
              <div className="mt-2 space-y-1">
                <p className="text-[11px] text-zinc-500">Available on:</p>
                {toast.onlineDevices.map((name) => (
                  <p key={name} className="flex items-center gap-1.5 text-xs text-zinc-300">
                    <Check className="h-3 w-3 text-green-400" />
                    {name}
                  </p>
                ))}
              </div>
            )}
            {isReceived && (
              <p className="mt-1 text-[11px] text-zinc-500">Copied to clipboard</p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
