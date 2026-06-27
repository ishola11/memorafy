import type { AppTab } from "@memora/shared-types";
import { cn } from "@/lib/utils";

const TABS: { id: AppTab; label: string }[] = [
  { id: "history", label: "History" },
  { id: "pinned", label: "Pinned" },
  { id: "favorites", label: "Favorites" },
  { id: "collections", label: "Collections" },
  { id: "snippets", label: "Snippets" },
];

interface TabBarProps {
  activeTab: AppTab;
  onTabChange: (tab: AppTab) => void;
  compact?: boolean;
}

export function TabBar({ activeTab, onTabChange, compact }: TabBarProps) {
  return (
    <div
      role="tablist"
      className={cn(
        "flex shrink-0 gap-1 border-b border-border/60",
        compact ? "px-1" : "px-0.5",
      )}
    >
      {TABS.map((tab) => {
        const active = activeTab === tab.id;
        return (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={active}
            onClick={() => onTabChange(tab.id)}
            className={cn(
              "relative shrink-0 px-2.5 pb-2 pt-1 font-medium transition-colors",
              compact ? "text-[11px]" : "text-xs",
              active
                ? "text-zinc-900 dark:text-zinc-100"
                : "text-muted hover:text-zinc-700 dark:hover:text-zinc-300",
            )}
          >
            {tab.label}
            {active && (
              <span className="absolute inset-x-1.5 -bottom-px h-0.5 rounded-full bg-accent" />
            )}
          </button>
        );
      })}
    </div>
  );
}

export const APP_TABS = TABS;
