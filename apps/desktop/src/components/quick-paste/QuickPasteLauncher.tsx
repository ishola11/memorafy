import { Search } from "lucide-react";
import { useEffect, useRef } from "react";
import { PreviewCard } from "@/components/ui/PreviewCard";
import { APP_TABS, TabBar } from "@/components/ui/TabBar";
import { copyItem, toggleFavorite, togglePin } from "@/lib/api";
import { TIMELINE_LABELS } from "@/lib/utils";
import { useAppStore } from "@/stores/app-store";
import type { AppTab } from "@memora/shared-types";

export function QuickPasteLauncher() {
  const inputRef = useRef<HTMLInputElement>(null);
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
    setQuickPasteOpen,
  } = useAppStore();

  useEffect(() => {
    if (quickPasteOpen) {
      inputRef.current?.focus();
      void refresh();
    }
  }, [quickPasteOpen, refresh]);

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
        setQuickPasteOpen(false);
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      setQuickPasteOpen(false);
    } else if (e.key === "Tab") {
      e.preventDefault();
      cycleTab();
    }
  };

  if (!quickPasteOpen) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/40 pt-[12vh] backdrop-blur-sm"
      onMouseDown={() => setQuickPasteOpen(false)}
    >
      <div
        className="w-[680px] overflow-hidden rounded-2xl border border-white/10 bg-zinc-950/95 shadow-launcher backdrop-blur-xl"
        onMouseDown={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        <div className="border-b border-white/10 px-4 py-3">
          <div className="mb-2 flex items-center gap-3">
            <Search className="h-4 w-4 shrink-0 text-zinc-500" />
            <input
              ref={inputRef}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search your memory…  device:mac  type:image  is:pinned"
              className="flex-1 bg-transparent text-sm text-zinc-100 outline-none placeholder:text-zinc-500"
            />
          </div>
          <TabBar activeTab={activeTab} onTabChange={setActiveTab} compact />
        </div>

        <div className="max-h-[360px] overflow-y-auto p-2">
          {query.trim() ? (
            flatItems.length === 0 ? (
              <p className="px-3 py-8 text-center text-sm text-zinc-500">
                No matches found
              </p>
            ) : (
              <div className="space-y-1">
                {flatItems.map((card, index) => (
                  <PreviewCard
                    key={card.id}
                    card={card}
                    selected={index === selectedIndex}
                    onSelect={() => setSelectedIndex(index)}
                    onCopy={async () => {
                      await copyItem(card.id);
                      setQuickPasteOpen(false);
                    }}
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
                    <p className="sticky top-0 z-10 bg-zinc-950/95 px-2 py-1.5 text-[11px] font-medium uppercase tracking-wide text-zinc-500">
                      {TIMELINE_LABELS[section.bucket] ?? section.label}
                    </p>
                  )}
                  <div className="space-y-1">
                    {section.items.map((card, index) => {
                      const globalIndex = timeline
                        .slice(0, timeline.indexOf(section))
                        .reduce((acc, s) => acc + s.items.length, 0) + index;
                      return (
                        <PreviewCard
                          key={card.id}
                          card={card}
                          selected={globalIndex === selectedIndex}
                          onSelect={() => setSelectedIndex(globalIndex)}
                          onCopy={async () => {
                            await copyItem(card.id);
                            setQuickPasteOpen(false);
                          }}
                          onPin={() => togglePin(card.id)}
                          onFavorite={() => toggleFavorite(card.id)}
                        />
                      );
                    })}
                  </div>
                </div>
              ) : null,
            )
          )}
        </div>

        <div className="flex items-center justify-between border-t border-white/10 px-4 py-2 text-[11px] text-zinc-500">
          <span>↵ paste · ⇥ switch tab · esc close</span>
          <span>{flatItems.length} items</span>
        </div>
      </div>
    </div>
  );
}
