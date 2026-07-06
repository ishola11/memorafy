import { Plus } from "lucide-react";
import type { Collection } from "@memorafy/shared-types";
import { cn } from "@/lib/utils";

interface CollectionChipsProps {
  collections: Collection[];
  selectedId: string | null;
  onSelect: (id: string | null) => void;
  onCreateClick?: () => void;
  className?: string;
}

export function CollectionChips({
  collections,
  selectedId,
  onSelect,
  onCreateClick,
  className,
}: CollectionChipsProps) {
  return (
    <div className={cn("flex flex-wrap items-center gap-1.5", className)}>
      <button
        type="button"
        onClick={() => onSelect(null)}
        className={cn(
          "inline-flex items-center rounded-md px-2 py-1 text-xs font-medium transition-colors",
          selectedId === null
            ? "bg-accent/15 text-accent ring-1 ring-accent/25"
            : "bg-surface-elevated text-muted hover:text-zinc-700 dark:hover:text-zinc-200",
        )}
      >
        All
      </button>
      {collections.map((c) => (
        <button
          key={c.id}
          type="button"
          onClick={() => onSelect(c.id)}
          className={cn(
            "inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-medium transition-colors",
            selectedId === c.id
              ? "bg-accent/15 text-accent ring-1 ring-accent/25"
              : "bg-surface-elevated text-muted hover:text-zinc-700 dark:hover:text-zinc-200",
          )}
        >
          <span
            className="h-2 w-2 shrink-0 rounded-full ring-1 ring-black/10 dark:ring-white/10"
            style={{ backgroundColor: c.color }}
          />
          {c.name}
          {c.itemCount > 0 && (
            <span className="text-[10px] opacity-60">{c.itemCount}</span>
          )}
        </button>
      ))}
      {onCreateClick && (
        <button
          type="button"
          onClick={onCreateClick}
          className="inline-flex items-center gap-1 rounded-md px-2 py-1 text-xs text-muted transition-colors hover:bg-surface-elevated hover:text-zinc-700 dark:hover:text-zinc-200"
        >
          <Plus className="h-3 w-3" />
          New
        </button>
      )}
    </div>
  );
}
