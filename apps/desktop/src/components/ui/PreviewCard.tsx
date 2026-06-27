import {
  Code2,
  Globe,
  Image as ImageIcon,
  Pin,
  Star,
  Type,
  Zap,
} from "lucide-react";
import type { PreviewCard as PreviewCardType } from "@memora/shared-types";
import { cn } from "@/lib/utils";

const kindIcons = {
  text: Type,
  url: Globe,
  code: Code2,
  image: ImageIcon,
  richtext: Type,
  snippet: Zap,
};

interface PreviewCardProps {
  card: PreviewCardType;
  selected?: boolean;
  onSelect?: () => void;
  onCopy?: () => void;
  onPin?: () => void;
  onFavorite?: () => void;
  compact?: boolean;
}

export function PreviewCard({
  card,
  selected = false,
  onSelect,
  onCopy,
  onPin,
  onFavorite,
  compact = false,
}: PreviewCardProps) {
  const Icon = kindIcons[card.kind as keyof typeof kindIcons] ?? Type;

  return (
    <div
      role="option"
      aria-selected={selected}
      onClick={onSelect}
      onDoubleClick={onCopy}
      className={cn(
        "group relative flex cursor-pointer gap-3 rounded-xl border px-3 py-2.5 transition-all",
        selected
          ? "border-accent/60 bg-accent/10 ring-1 ring-accent/30"
          : "border-border/60 bg-surface-elevated/80 hover:border-border hover:bg-surface-elevated",
        compact && "py-2",
      )}
    >
      <div className="flex h-10 w-10 shrink-0 items-center justify-center overflow-hidden rounded-lg bg-surface border border-border/50">
        {card.thumbnail ? (
          <img
            src={card.thumbnail}
            alt=""
            className="h-full w-full object-cover"
          />
        ) : (
          <Icon className="h-4 w-4 text-muted" />
        )}
      </div>

      <div className="min-w-0 flex-1">
        <div className="flex items-start gap-2">
          <p className="truncate text-sm font-medium text-zinc-900 dark:text-zinc-100">
            {card.title}
          </p>
          <div className="ml-auto flex shrink-0 items-center gap-1 opacity-70">
            {card.badges.includes("pinned") && (
              <Pin className="h-3 w-3 text-accent" />
            )}
            {card.badges.includes("favorite") && (
              <Star className="h-3 w-3 text-amber-400" />
            )}
            {card.badges.includes("snippet") && (
              <span className="rounded bg-accent/20 px-1.5 py-0.5 text-[10px] font-medium text-accent">
                snippet
              </span>
            )}
          </div>
        </div>
        {card.subtitle && (
          <p className="truncate text-xs text-muted">{card.subtitle}</p>
        )}
        <p className="mt-0.5 truncate text-[11px] text-muted">{card.meta}</p>
      </div>

      <div className="absolute right-2 top-1/2 hidden -translate-y-1/2 items-center gap-1 group-hover:flex">
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onCopy?.();
          }}
          className="rounded-md bg-surface-elevated px-2 py-1 text-[11px] text-zinc-700 hover:bg-border/40 dark:text-zinc-200 dark:hover:bg-zinc-800"
        >
          Copy
        </button>
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onPin?.();
          }}
          className="rounded-md p-1 text-muted hover:bg-surface-elevated hover:text-zinc-700 dark:hover:text-zinc-200"
        >
          <Pin className="h-3.5 w-3.5" />
        </button>
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onFavorite?.();
          }}
          className="rounded-md p-1 text-muted hover:bg-surface-elevated hover:text-zinc-700 dark:hover:text-zinc-200"
        >
          <Star className="h-3.5 w-3.5" />
        </button>
      </div>
    </div>
  );
}
