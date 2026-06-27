import { useEffect, useRef, useState } from "react";
import {
  Check,
  Clipboard,
  ClipboardCopy,
  Code2,
  FolderPlus,
  Globe,
  Image as ImageIcon,
  Loader2,
  Pin,
  Star,
  Trash2,
  Type,
  Zap,
} from "lucide-react";
import type { Collection, PreviewCard as PreviewCardType } from "@memora/shared-types";
import { getItemCollections } from "@/lib/api";
import { cn } from "@/lib/utils";

const kindIcons = {
  text: Type,
  url: Globe,
  code: Code2,
  image: ImageIcon,
  richtext: Type,
  snippet: Zap,
};

type BusyAction =
  | "copy"
  | "copyPlain"
  | "pin"
  | "favorite"
  | "delete"
  | `collection:${string}`
  | null;

interface PreviewCardProps {
  card: PreviewCardType;
  selected?: boolean;
  onSelect?: () => void;
  onCopy?: () => void | Promise<void>;
  onCopyPlain?: () => void | Promise<void>;
  onPin?: () => void | Promise<void>;
  onFavorite?: () => void | Promise<void>;
  onDelete?: () => void | Promise<void>;
  collections?: Collection[];
  itemCollectionIds?: string[];
  onAddToCollection?: (collectionId: string) => void | Promise<void>;
  onRemoveFromCollection?: (collectionId: string) => void | Promise<void>;
  compact?: boolean;
}

function ActionButton({
  label,
  onClick,
  children,
  className,
  danger,
  disabled,
  busy,
}: {
  label: string;
  onClick: () => void;
  children: React.ReactNode;
  className?: string;
  danger?: boolean;
  disabled?: boolean;
  busy?: boolean;
}) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      disabled={disabled || busy}
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      className={cn(
        "rounded p-0.5 text-muted transition-colors hover:bg-surface-elevated disabled:cursor-not-allowed disabled:opacity-50",
        danger
          ? "hover:text-red-500"
          : "hover:text-zinc-700 dark:hover:text-zinc-200",
        className,
      )}
    >
      {busy ? <Loader2 className="h-3 w-3 animate-spin" /> : children}
    </button>
  );
}

async function runAction(action: () => void | Promise<void>, setBusy: (v: BusyAction) => void, key: BusyAction) {
  setBusy(key);
  try {
    await action();
  } finally {
    setBusy(null);
  }
}

export function PreviewCard({
  card,
  selected = false,
  onSelect,
  onCopy,
  onCopyPlain,
  onPin,
  onFavorite,
  onDelete,
  collections = [],
  itemCollectionIds: itemCollectionIdsProp,
  onAddToCollection,
  onRemoveFromCollection,
  compact = false,
}: PreviewCardProps) {
  const Icon = kindIcons[card.kind as keyof typeof kindIcons] ?? Type;
  const [menuOpen, setMenuOpen] = useState(false);
  const [busyAction, setBusyAction] = useState<BusyAction>(null);
  const [itemCollectionIds, setItemCollectionIds] = useState<string[]>(
    itemCollectionIdsProp ?? [],
  );
  const [flashCollectionId, setFlashCollectionId] = useState<string | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const flashTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(() => {
    if (itemCollectionIdsProp) {
      setItemCollectionIds(itemCollectionIdsProp);
    }
  }, [itemCollectionIdsProp]);

  useEffect(() => {
    if (!menuOpen) return;
    void getItemCollections(card.id).then(setItemCollectionIds).catch(() => undefined);
  }, [menuOpen, card.id]);

  useEffect(() => {
    if (!menuOpen) return;
    const close = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false);
      }
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [menuOpen]);

  useEffect(() => {
    return () => {
      if (flashTimerRef.current) clearTimeout(flashTimerRef.current);
    };
  }, []);

  const showCollections = collections.length > 0 && (onAddToCollection || onRemoveFromCollection);
  const hasActions = Boolean(
    onCopy || onCopyPlain || onPin || onFavorite || onDelete || showCollections,
  );
  const isBusy = busyAction !== null;

  const handleCollectionToggle = async (collectionId: string, inCollection: boolean) => {
    const actionKey: BusyAction = `collection:${collectionId}`;
    setBusyAction(actionKey);
    try {
      if (inCollection) {
        await onRemoveFromCollection?.(collectionId);
        setItemCollectionIds((ids) => ids.filter((id) => id !== collectionId));
      } else {
        await onAddToCollection?.(collectionId);
        setItemCollectionIds((ids) => [...ids, collectionId]);
        setFlashCollectionId(collectionId);
        if (flashTimerRef.current) clearTimeout(flashTimerRef.current);
        flashTimerRef.current = setTimeout(() => setFlashCollectionId(null), 1200);
      }
      setMenuOpen(false);
    } finally {
      setBusyAction(null);
    }
  };

  return (
    <div
      role="option"
      aria-selected={selected}
      onClick={onSelect}
      onDoubleClick={() => {
        if (onCopy) void runAction(onCopy, setBusyAction, "copy");
      }}
      className={cn(
        "group flex cursor-pointer flex-col rounded-xl border px-3 py-2.5 transition-all",
        selected
          ? "border-accent/60 bg-accent/10 ring-1 ring-accent/30"
          : "border-border/60 bg-surface-elevated/80 hover:border-border hover:bg-surface-elevated",
        compact && "py-2",
        isBusy && "opacity-90",
      )}
    >
      <div className="flex gap-3">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center overflow-hidden rounded-lg border border-border/50 bg-surface">
          {card.thumbnail ? (
            <img src={card.thumbnail} alt="" className="h-full w-full object-cover" />
          ) : (
            <Icon className="h-4 w-4 text-muted" />
          )}
        </div>

        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-center gap-1.5">
            <p className="truncate text-sm font-medium text-zinc-900 dark:text-zinc-100">
              {card.title}
            </p>
            <div className="flex shrink-0 items-center gap-1">
              {card.badges.includes("pinned") && <Pin className="h-3 w-3 text-accent" />}
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
      </div>

      {hasActions && (
        <div
          className={cn(
            "mt-2 flex items-center gap-0.5 border-t border-border/40 pt-1.5 transition-opacity duration-150",
            compact
              ? "opacity-0 group-hover:opacity-100 group-focus-within:opacity-100"
              : "opacity-50 group-hover:opacity-100",
          )}
        >
          {onCopy && (
            <ActionButton
              label="Copy"
              disabled={isBusy}
              busy={busyAction === "copy"}
              onClick={() => void runAction(onCopy, setBusyAction, "copy")}
            >
              <Clipboard className="h-3 w-3" />
            </ActionButton>
          )}
          {onCopyPlain && (
            <ActionButton
              label="Copy as plain text"
              disabled={isBusy}
              busy={busyAction === "copyPlain"}
              onClick={() => void runAction(onCopyPlain, setBusyAction, "copyPlain")}
            >
              <ClipboardCopy className="h-3 w-3" />
            </ActionButton>
          )}
          {onPin && (
            <ActionButton
              label={card.isPinned ? "Unpin" : "Pin"}
              disabled={isBusy}
              busy={busyAction === "pin"}
              onClick={() => void runAction(onPin, setBusyAction, "pin")}
            >
              <Pin className={cn("h-3 w-3", card.isPinned && "text-accent")} />
            </ActionButton>
          )}
          {onFavorite && (
            <ActionButton
              label={card.isFavorited ? "Unfavorite" : "Favorite"}
              disabled={isBusy}
              busy={busyAction === "favorite"}
              onClick={() => void runAction(onFavorite, setBusyAction, "favorite")}
            >
              <Star
                className={cn(
                  "h-3 w-3",
                  card.isFavorited && "fill-amber-400 text-amber-400",
                )}
              />
            </ActionButton>
          )}
          {showCollections && (
            <div className="relative" ref={menuRef}>
              <ActionButton
                label="Add to collection"
                disabled={isBusy}
                busy={busyAction?.startsWith("collection:") ?? false}
                onClick={() => setMenuOpen((v) => !v)}
              >
                <FolderPlus className="h-3 w-3" />
              </ActionButton>
              {menuOpen && (
                <div className="absolute bottom-[calc(100%+4px)] left-0 z-50 min-w-[168px] rounded-lg border border-border/60 bg-surface py-1 shadow-lg">
                  {collections.map((c) => {
                    const inCollection = itemCollectionIds.includes(c.id);
                    const isFlashing = flashCollectionId === c.id;
                    const rowBusy = busyAction === `collection:${c.id}`;
                    return (
                      <button
                        key={c.id}
                        type="button"
                        disabled={isBusy}
                        onClick={(e) => {
                          e.stopPropagation();
                          void handleCollectionToggle(c.id, inCollection);
                        }}
                        className={cn(
                          "flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs hover:bg-surface-elevated disabled:opacity-50",
                          isFlashing && "bg-accent/10",
                        )}
                      >
                        <span
                          className="h-2 w-2 shrink-0 rounded-full"
                          style={{ backgroundColor: c.color }}
                        />
                        <span className="flex-1 truncate">{c.name}</span>
                        {rowBusy ? (
                          <Loader2 className="h-3 w-3 animate-spin text-muted" />
                        ) : (inCollection || isFlashing) ? (
                          <Check className="h-3 w-3 text-accent" />
                        ) : null}
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          )}
          {onDelete && (
            <ActionButton
              label="Delete"
              danger
              disabled={isBusy}
              busy={busyAction === "delete"}
              onClick={() => void runAction(onDelete, setBusyAction, "delete")}
            >
              <Trash2 className="h-3 w-3" />
            </ActionButton>
          )}
        </div>
      )}
    </div>
  );
}
