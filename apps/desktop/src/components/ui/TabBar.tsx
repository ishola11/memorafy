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
      className={cn(
        "flex gap-0.5 overflow-x-auto",
        compact ? "rounded-lg bg-zinc-900 p-0.5 text-[11px]" : "border-b border-white/10 px-2",
      )}
    >
      {TABS.map((tab) => (
        <button
          key={tab.id}
          type="button"
          onClick={() => onTabChange(tab.id)}
          className={cn(
            "shrink-0 rounded-md px-2 py-1 transition-colors",
            compact
              ? activeTab === tab.id
                ? "bg-zinc-800 text-zinc-100"
                : "text-zinc-500 hover:text-zinc-300"
              : activeTab === tab.id
                ? "border-b-2 border-indigo-400 text-zinc-100"
                : "text-zinc-500 hover:text-zinc-300",
          )}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}

export const APP_TABS = TABS;
