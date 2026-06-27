import { FolderOpen, Pause, Search, Settings } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { PreviewCard } from "@/components/ui/PreviewCard";
import { TabBar } from "@/components/ui/TabBar";
import { copyItem, getSyncState, openSettings, toggleFavorite, togglePin } from "@/lib/api";
import type { SyncState } from "@memora/shared-types";
import { TIMELINE_LABELS, cn } from "@/lib/utils";
import { useAppStore } from "@/stores/app-store";

export function TrayPanel() {
  const inputRef = useRef<HTMLInputElement>(null);
  const {
    trayOpen,
    query,
    results,
    timeline,
    collections,
    activeTab,
    selectedCollectionId,
    clipboardPaused,
    setQuery,
    setActiveTab,
    setSelectedCollectionId,
    search,
    refresh,
    toggleClipboardPause,
  } = useAppStore();

  const [syncState, setSyncState] = useState<SyncState | null>(null);

  useEffect(() => {
    if (trayOpen) {
      void refresh();
      void getSyncState().then(setSyncState).catch(() => undefined);
    }
  }, [trayOpen, refresh]);

  useEffect(() => {
    const timer = setTimeout(() => {
      void search(query);
    }, 80);
    return () => clearTimeout(timer);
  }, [query, search]);

  if (!trayOpen) return null;

  const showingSearch = query.trim().length > 0;

  return (
    <div className="flex h-screen w-[420px] flex-col bg-zinc-950 text-zinc-100">
      <div className="border-b border-white/10 px-4 py-3">
        <div className="mb-2 flex items-center justify-between">
          <h1 className="text-sm font-semibold tracking-tight">Memora</h1>
          <div className="flex items-center gap-2">
            {syncState?.loggedIn && (
              <span className="flex items-center gap-1 text-[10px] text-green-400">
                <span className="h-1.5 w-1.5 rounded-full bg-green-400" />
                Synced
              </span>
            )}
            <span className="text-[11px] text-zinc-500">Cross-device memory</span>
          </div>
        </div>
        <div className="mb-2 flex items-center gap-2 rounded-xl border border-white/10 bg-zinc-900/80 px-3 py-2">
          <Search className="h-4 w-4 text-zinc-500" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search clips, tags, devices…"
            className="flex-1 bg-transparent text-sm outline-none placeholder:text-zinc-500"
          />
        </div>
        <TabBar activeTab={activeTab} onTabChange={setActiveTab} />
      </div>

      {activeTab === "collections" && collections.length > 0 && (
        <div className="border-b border-white/10 px-3 py-2">
          <div className="flex flex-wrap gap-1">
            <button
              type="button"
              onClick={() => setSelectedCollectionId(null)}
              className={cn(
                "inline-flex items-center gap-1 rounded-lg px-2 py-1 text-xs",
                selectedCollectionId === null
                  ? "bg-indigo-500/20 text-indigo-200"
                  : "bg-zinc-900 text-zinc-300 hover:bg-zinc-800",
              )}
            >
              All
            </button>
            {collections.map((c) => (
              <button
                key={c.id}
                type="button"
                onClick={() => setSelectedCollectionId(c.id)}
                className={cn(
                  "inline-flex items-center gap-1 rounded-lg px-2 py-1 text-xs",
                  selectedCollectionId === c.id
                    ? "bg-indigo-500/20 text-indigo-200"
                    : "bg-zinc-900 text-zinc-300 hover:bg-zinc-800",
                )}
              >
                <span
                  className="h-2 w-2 rounded-full"
                  style={{ backgroundColor: c.color }}
                />
                {c.name}
              </button>
            ))}
          </div>
        </div>
      )}

      <div className="flex-1 overflow-y-auto p-2">
        {showingSearch ? (
          results.length === 0 ? (
            <p className="px-3 py-8 text-center text-sm text-zinc-500">
              No matches
            </p>
          ) : (
            <div className="space-y-1">
              {results.map((card) => (
                <PreviewCard
                  key={card.id}
                  card={card}
                  compact
                  onCopy={() => copyItem(card.id)}
                  onPin={() => togglePin(card.id)}
                  onFavorite={() => toggleFavorite(card.id)}
                />
              ))}
            </div>
          )
        ) : (
          timeline.map((section) =>
            section.items.length > 0 ? (
              <div key={section.bucket} className="mb-3">
                {activeTab === "history" && (
                  <p className="px-2 py-1.5 text-[11px] font-medium uppercase tracking-wide text-zinc-500">
                    {TIMELINE_LABELS[section.bucket] ?? section.label}
                  </p>
                )}
                <div className="space-y-1">
                  {section.items.map((card) => (
                    <PreviewCard
                      key={card.id}
                      card={card}
                      compact
                      onCopy={() => copyItem(card.id)}
                      onPin={() => togglePin(card.id)}
                      onFavorite={() => toggleFavorite(card.id)}
                    />
                  ))}
                </div>
              </div>
            ) : null,
          )
        )}
      </div>

      <div className="flex items-center justify-between border-t border-white/10 px-4 py-2 text-xs text-zinc-500">
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={() => void openSettings()}
            className="inline-flex items-center gap-1 hover:text-zinc-300"
          >
            <Settings className="h-3.5 w-3.5" /> Settings
          </button>
          <button
            type="button"
            onClick={() => setActiveTab("collections")}
            className={cn(
              "inline-flex items-center gap-1 hover:text-zinc-300",
              activeTab === "collections" && "text-indigo-300",
            )}
          >
            <FolderOpen className="h-3.5 w-3.5" /> Collections
          </button>
        </div>
        <button
          type="button"
          onClick={() => void toggleClipboardPause()}
          className={cn(
            "inline-flex items-center gap-1 hover:text-zinc-300",
            clipboardPaused && "text-amber-400",
          )}
        >
          <Pause className="h-3.5 w-3.5" /> {clipboardPaused ? "Paused" : "Pause"}
        </button>
      </div>
    </div>
  );
}

export function TrayShell({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("h-full w-full", "dark", className)}>{children}</div>
  );
}
