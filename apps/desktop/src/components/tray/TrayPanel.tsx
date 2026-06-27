import { Pause, Search, Settings } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { CollectionChips } from "@/components/ui/CollectionChips";
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
      requestAnimationFrame(() => inputRef.current?.focus());
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
    <div className="panel-shell w-[400px] shadow-[var(--panel-shadow)]">
      <header className="panel-header px-4 pb-0 pt-3">
        <div className="mb-3 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <h1 className="text-[13px] font-semibold tracking-tight">Memora</h1>
            {syncState?.loggedIn && (
              <span className="flex items-center gap-1 rounded-full bg-green-500/10 px-1.5 py-0.5 text-[10px] font-medium text-green-600 dark:text-green-400">
                <span className="h-1.5 w-1.5 rounded-full bg-green-500" />
                Synced
              </span>
            )}
          </div>
          <button
            type="button"
            onClick={() => void openSettings()}
            className="rounded-md p-1.5 text-muted transition-colors hover:bg-surface-elevated hover:text-zinc-700 dark:hover:text-zinc-200"
            aria-label="Settings"
          >
            <Settings className="h-3.5 w-3.5" />
          </button>
        </div>

        <div className="search-input mb-3">
          <Search className="h-4 w-4 shrink-0 text-muted" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search clips, tags, devices…"
          />
        </div>

        <TabBar activeTab={activeTab} onTabChange={setActiveTab} />
      </header>

      {activeTab === "collections" && (
        <div className="shrink-0 border-b border-border/60 px-4 py-2.5">
          <CollectionChips
            collections={collections}
            selectedId={selectedCollectionId}
            onSelect={setSelectedCollectionId}
            onCreateClick={() => void openSettings()}
          />
        </div>
      )}

      <div className="panel-content p-2">
        {showingSearch ? (
          results.length === 0 ? (
            <p className="px-3 py-12 text-center text-sm text-muted">No matches</p>
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
        ) : timeline.every((s) => s.items.length === 0) ? (
          <p className="px-3 py-12 text-center text-sm text-muted">
            {activeTab === "collections" ? "No items in this collection" : "Nothing here yet"}
          </p>
        ) : (
          timeline.map((section) =>
            section.items.length > 0 ? (
              <div key={section.bucket} className="mb-3">
                {activeTab === "history" && (
                  <p className="sticky top-0 z-10 bg-surface/95 px-2 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted backdrop-blur-sm">
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

      <footer className="panel-footer flex items-center justify-between text-xs text-muted">
        <span className="text-[11px]">Cross-device memory</span>
        <button
          type="button"
          onClick={() => void toggleClipboardPause()}
          className={cn(
            "inline-flex items-center gap-1 rounded-md px-2 py-1 transition-colors hover:bg-surface-elevated hover:text-zinc-700 dark:hover:text-zinc-200",
            clipboardPaused && "text-amber-600 dark:text-amber-400",
          )}
        >
          <Pause className="h-3.5 w-3.5" />
          {clipboardPaused ? "Paused" : "Pause capture"}
        </button>
      </footer>
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
  return <div className={cn("h-full w-full", className)}>{children}</div>;
}
