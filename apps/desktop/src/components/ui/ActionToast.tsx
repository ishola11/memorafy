import { Check, X } from "lucide-react";
import { useActionToastStore } from "@/stores/action-toast-store";

export function ActionToast() {
  const { message, kind } = useActionToastStore();

  if (!message) return null;

  const isError = kind === "error";

  return (
    <div className="pointer-events-none fixed bottom-6 left-6 z-[110] animate-in fade-in slide-in-from-bottom-2">
      <div
        className={`flex items-center gap-2 rounded-lg border px-3 py-2 shadow-lg backdrop-blur-xl ${
          isError
            ? "border-red-500/30 bg-red-950/90 text-red-100"
            : "border-border/60 bg-surface/95 text-zinc-900 dark:text-zinc-100"
        }`}
      >
        {isError ? (
          <X className="h-3.5 w-3.5 shrink-0 text-red-400" />
        ) : (
          <Check className="h-3.5 w-3.5 shrink-0 text-accent" />
        )}
        <p className="text-xs font-medium">{message}</p>
      </div>
    </div>
  );
}
