import { Pause, Search, Settings } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { SnippetEditorModal, type SnippetEditorState } from "@/components/snippets/SnippetEditorModal";
import { SnippetsTabHeader } from "@/components/snippets/SnippetsTabHeader";
import { CollectionChips } from "@/components/ui/CollectionChips";
import { PreviewCard } from "@/components/ui/PreviewCard";
import { TabBar } from "@/components/ui/TabBar";
import {
  addItemToCollection,
  copyItem,
  deleteItem,
  getSyncState,
  openSettings,
  removeItemFromCollection,
  saveItemAsSnippet,
  toggleFavorite,
  togglePin,
} from "@/lib/api";
import type { PreviewCard as PreviewCardType, SyncState } from "@memora/shared-types";
import { TIMELINE_LABELS, cn } from "@/lib/utils";
import { useActionToastStore } from "@/stores/action-toast-store";
import { useAppStore } from "@/stores/app-store";

const SNIPPET_KINDS = new Set(["text", "url", "code"]);

/**
 * Reflects actual sync health, not just login state — a green "Synced"
 * badge while 40 changes sit stuck in the queue would be a lie.
 */
function SyncBadge({ syncState }: { syncState: SyncState | null }) {
  if (!syncState?.configured) return null;

  if (!syncState.loggedIn) {
    return (
      <span className="flex items-center gap-1 rounded-full bg-zinc-500/10 px-1.5 py-0.5 text-[10px] font-medium text-zinc-500 dark:text-zinc-400">
        <span className="h-1.5 w-1.5 rounded-full bg-zinc-400" />
        Local only
      </span>
    );
  }

  if (syncState.pendingCount > 0) {
    return (
      <span
        className="flex items-center gap-1 rounded-full bg-amber-500/10 px-1.5 py-0.5 text-[10px] font-medium text-amber-600 dark:text-amber-400"
        title="Changes waiting to upload — they'll sync automatically"
      >
        <span className="h-1.5 w-1.5 rounded-full bg-amber-500" />
        {syncState.pendingCount} pending
      </span>
    );
  }

  return (
    <span className="flex items-center gap-1 rounded-full bg-green-500/10 px-1.5 py-0.5 text-[10px] font-medium text-green-600 dark:text-green-400">
      <span className="h-1.5 w-1.5 rounded-full bg-green-500" />
      Synced
    </span>
  );
}

function isSnippetCard(card: PreviewCardType) {
  return card.kind === "snippet" || card.badges.includes("snippet");
}

function canSaveAsSnippet(card: PreviewCardType) {
  return !isSnippetCard(card) && SNIPPET_KINDS.has(card.kind);
}

export function TrayPanel() {
  const inputRef = useRef<HTMLInputElement>(null);
  const isTrayWindow =
    new URLSearchParams(window.location.search).get("window") === "tray";
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
  const [snippetEditor, setSnippetEditor] = useState<SnippetEditorState | null>(null);
  const showActionToast = useActionToastStore((s) => s.showActionToast);

  useEffect(() => {
    if (!isTrayWindow && !trayOpen) return;
    void refresh();
    void getSyncState().then(setSyncState).catch(() => undefined);
    requestAnimationFrame(() => inputRef.current?.focus());
  }, [trayOpen, isTrayWindow, refresh]);

  useEffect(() => {
    const timer = setTimeout(() => {
      void search(query);
    }, 80);
    return () => clearTimeout(timer);
  }, [query, search]);

  if (!isTrayWindow && !trayOpen) return null;

  const showingSearch = query.trim().length > 0;

  const cardActions = (card: PreviewCardType) => ({
    onCopy: async () => {
      await copyItem(card.id);
      showActionToast("Copied to clipboard");
    },
    onCopyPlain: async () => {
      await copyItem(card.id, true);
      showActionToast("Copied as plain text");
    },
    onPin: async () => {
      await togglePin(card.id);
      await refresh();
      showActionToast(card.isPinned ? "Unpinned" : "Pinned");
    },
    onFavorite: async () => {
      await toggleFavorite(card.id);
      await refresh();
      showActionToast(card.isFavorited ? "Removed from favorites" : "Added to favorites");
    },
    onDelete: async () => {
      await deleteItem(card.id);
      await refresh();
      showActionToast("Deleted");
    },
    collections,
    onAddToCollection: async (collectionId: string) => {
      await addItemToCollection(card.id, collectionId);
      await refresh();
      const name = collections.find((c) => c.id === collectionId)?.name ?? "collection";
      showActionToast(`Added to ${name}`);
    },
    onRemoveFromCollection: async (collectionId: string) => {
      await removeItemFromCollection(card.id, collectionId);
      await refresh();
      const name = collections.find((c) => c.id === collectionId)?.name ?? "collection";
      showActionToast(`Removed from ${name}`);
    },
    onSaveAsSnippet: canSaveAsSnippet(card)
      ? async () => {
          await saveItemAsSnippet(card.id);
          await refresh();
          showActionToast("Saved as snippet");
        }
      : undefined,
    onEditSnippet: isSnippetCard(card)
      ? () => setSnippetEditor({ mode: "edit", snippetId: card.id })
      : undefined,
  });

  const collectionsEmptyMessage =
    collections.length === 0
      ? "Create a collection in Settings, then use the folder icon on any clip to add it."
      : selectedCollectionId
        ? "No items in this collection yet. Hover a clip and click the folder icon to add it."
        : "Select a collection above, or add clips via the folder icon on any card.";

  const snippetsEmptyMessage =
    "Save reusable text — signatures, templates, and boilerplate. Click New snippet to create your first one.";

  return (
    <div
      className={cn(
        "panel-shell shadow-[var(--panel-shadow)]",
        isTrayWindow
          ? "rounded-xl border border-border/50"
          : "w-[400px]",
      )}
    >
      <header className="panel-header px-4 pb-0 pt-3">
        <div className="mb-3 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <h1 className="text-[13px] font-semibold tracking-tight">Memora</h1>
            <SyncBadge syncState={syncState} />
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

      {activeTab === "snippets" && (
        <SnippetsTabHeader onNewSnippet={() => setSnippetEditor({ mode: "create" })} />
      )}

      <div className="panel-content p-2">
        {showingSearch ? (
          results.length === 0 ? (
            <p className="px-3 py-12 text-center text-sm text-muted">No matches</p>
          ) : (
            <div className="space-y-1">
              {results.map((card) => (
                <PreviewCard key={card.id} card={card} compact {...cardActions(card)} />
              ))}
            </div>
          )
        ) : timeline.every((s) => s.items.length === 0) ? (
          <p className="px-3 py-12 text-center text-sm text-muted">
            {activeTab === "collections"
              ? collectionsEmptyMessage
              : activeTab === "snippets"
                ? snippetsEmptyMessage
                : activeTab === "history"
                  ? "Copy something and it'll appear here. Open this list anywhere with the Quick Paste shortcut."
                  : activeTab === "pinned"
                    ? "Pin important clips with the pin icon to keep them at the top, forever."
                    : "Mark clips with the star icon to collect your favorites here."}
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
                    <PreviewCard key={card.id} card={card} compact {...cardActions(card)} />
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

      <SnippetEditorModal
        editor={snippetEditor}
        onClose={() => setSnippetEditor(null)}
        onSaved={async () => {
          await refresh();
          showActionToast(snippetEditor?.mode === "create" ? "Snippet created" : "Snippet updated");
        }}
      />
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
