import { Search } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { SnippetEditorModal, type SnippetEditorState } from "@/components/snippets/SnippetEditorModal";
import { SnippetsTabHeader } from "@/components/snippets/SnippetsTabHeader";
import { PreviewCard } from "@/components/ui/PreviewCard";
import { APP_TABS, TabBar } from "@/components/ui/TabBar";
import {
  addItemToCollection,
  copyItem,
  deleteItem,
  removeItemFromCollection,
  saveItemAsSnippet,
  toggleFavorite,
  togglePin,
} from "@/lib/api";
import { TIMELINE_LABELS, cn } from "@/lib/utils";
import { useActionToastStore } from "@/stores/action-toast-store";
import { useAppStore } from "@/stores/app-store";
import type { AppTab, PreviewCard as PreviewCardType } from "@memora/shared-types";

const SNIPPETS_EMPTY_MESSAGE =
  "Save reusable text: signatures, templates, and boilerplate. Click New snippet to create your first one.";

const SNIPPET_KINDS = new Set(["text", "url", "code"]);

function isSnippetCard(card: PreviewCardType) {
  return card.kind === "snippet" || card.badges.includes("snippet");
}

function canSaveAsSnippet(card: PreviewCardType) {
  return !isSnippetCard(card) && SNIPPET_KINDS.has(card.kind);
}

export function QuickPasteLauncher() {
  const inputRef = useRef<HTMLInputElement>(null);
  const isQuickPasteWindow =
    new URLSearchParams(window.location.search).get("window") === "quick-paste";
  const {
    quickPasteOpen,
    query,
    results,
    timeline,
    selectedIndex,
    activeTab,
    setQuery,
    setActiveTab,
    setSelectedIndex,
    search,
    refresh,
    collections,
    closeQuickPaste,
  } = useAppStore();
  const showActionToast = useActionToastStore((s) => s.showActionToast);
  const [snippetEditor, setSnippetEditor] = useState<SnippetEditorState | null>(null);

  useEffect(() => {
    if (quickPasteOpen) {
      inputRef.current?.focus();
      void refresh();
    }
  }, [quickPasteOpen, refresh]);

  useEffect(() => {
    if (!quickPasteOpen) return;

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        closeQuickPaste();
      }
    };

    document.addEventListener("keydown", onKeyDown, true);
    return () => document.removeEventListener("keydown", onKeyDown, true);
  }, [closeQuickPaste]);

  useEffect(() => {
    const timer = setTimeout(() => {
      void search(query);
    }, 80);
    return () => clearTimeout(timer);
  }, [query, search]);

  const flatItems = query.trim()
    ? results
    : timeline.flatMap((section) => section.items);

  const cycleTab = () => {
    const idx = APP_TABS.findIndex((t) => t.id === activeTab);
    const next = APP_TABS[(idx + 1) % APP_TABS.length]?.id ?? "history";
    setActiveTab(next as AppTab);
  };

  const handleKeyDown = async (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex(Math.min(selectedIndex + 1, flatItems.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex(Math.max(selectedIndex - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const item = flatItems[selectedIndex];
      if (item) {
        await copyItem(item.id);
        closeQuickPaste();
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      closeQuickPaste();
    } else if (e.key === "Tab") {
      e.preventDefault();
      cycleTab();
    }
  };

  if (!quickPasteOpen) return null;

  const cardActions = (card: PreviewCardType) => ({
    onCopy: async () => {
      await copyItem(card.id);
      showActionToast("Copied to clipboard");
      closeQuickPaste();
    },
    onCopyPlain: async () => {
      await copyItem(card.id, true);
      showActionToast("Copied as plain text");
      closeQuickPaste();
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

  const dismiss = () => closeQuickPaste();

  const shellClass = isQuickPasteWindow
    ? "flex h-full w-full flex-col bg-black/40 backdrop-blur-[4px] dark:bg-black/55"
    : "fixed inset-0 z-50 flex flex-col bg-black/35 backdrop-blur-[3px] dark:bg-black/50";

  return (
    <div
      className={shellClass}
      onMouseDown={dismiss}
    >
      <div className="flex flex-1 items-start justify-center px-5 pt-[9vh] pb-6">
        <div
          className={cn(
            "flex w-full max-w-[660px] flex-col overflow-hidden rounded-2xl",
            "border border-white/10 bg-surface shadow-[0_28px_80px_-16px_rgba(0,0,0,0.55)]",
            "ring-1 ring-black/5 dark:ring-white/5",
          )}
          style={{ maxHeight: "min(540px, 78vh)" }}
          onMouseDown={(e) => e.stopPropagation()}
          onKeyDown={handleKeyDown}
        >
          <header className="panel-header px-4 pb-0 pt-3.5">
            <div className="search-input mb-3">
              <Search className="h-4 w-4 shrink-0 text-muted" />
              <input
                ref={inputRef}
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search your memory…  device:mac  type:image  is:pinned"
              />
            </div>
            <TabBar activeTab={activeTab} onTabChange={setActiveTab} compact />
          </header>

          {activeTab === "snippets" && (
            <SnippetsTabHeader onNewSnippet={() => setSnippetEditor({ mode: "create" })} />
          )}

          <div className="panel-content min-h-0 p-2.5">
            {query.trim() ? (
              flatItems.length === 0 ? (
                <p className="px-3 py-12 text-center text-sm text-muted">No matches found</p>
              ) : (
                <div className="space-y-1.5">
                  {flatItems.map((card, index) => (
                    <PreviewCard
                      key={card.id}
                      card={card}
                      selected={index === selectedIndex}
                      onSelect={() => setSelectedIndex(index)}
                      {...cardActions(card)}
                    />
                  ))}
                </div>
              )
            ) : (
              timeline.map((section) =>
                section.items.length > 0 ? (
                  <div key={section.bucket} className="mb-3">
                    {activeTab === "history" && (
                      <p className="sticky top-0 z-10 bg-surface/95 px-2 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted backdrop-blur-sm">
                        {TIMELINE_LABELS[section.bucket] ?? section.label}
                      </p>
                    )}
                    <div className="space-y-1.5">
                      {section.items.map((card, index) => {
                        const globalIndex =
                          timeline
                            .slice(0, timeline.indexOf(section))
                            .reduce((acc, s) => acc + s.items.length, 0) + index;
                        return (
                          <PreviewCard
                            key={card.id}
                            card={card}
                            selected={globalIndex === selectedIndex}
                            onSelect={() => setSelectedIndex(globalIndex)}
                            {...cardActions(card)}
                          />
                        );
                      })}
                    </div>
                  </div>
                ) : null,
              )
            )}
            {!query.trim() && timeline.every((s) => s.items.length === 0) && (
              <p className="px-3 py-12 text-center text-sm text-muted">
                {activeTab === "snippets" ? SNIPPETS_EMPTY_MESSAGE : "Nothing here yet"}
              </p>
            )}
          </div>

          <footer className="panel-footer flex items-center justify-between text-[11px] text-muted">
            <span>↵ paste · ⇥ switch tab · esc close</span>
            <span>{flatItems.length} items</span>
          </footer>
        </div>
      </div>

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
