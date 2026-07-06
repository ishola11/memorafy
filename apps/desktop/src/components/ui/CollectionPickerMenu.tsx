import { useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Check, Loader2 } from "lucide-react";
import type { Collection } from "@memorafy/shared-types";
import { cn } from "@/lib/utils";

interface CollectionPickerMenuProps {
  open: boolean;
  anchorRef: React.RefObject<HTMLElement | null>;
  collections: Collection[];
  itemCollectionIds: string[];
  flashCollectionId: string | null;
  busyCollectionId: string | null;
  disabled?: boolean;
  onToggle: (collectionId: string, inCollection: boolean) => void;
  onClose: () => void;
}

export function CollectionPickerMenu({
  open,
  anchorRef,
  collections,
  itemCollectionIds,
  flashCollectionId,
  busyCollectionId,
  disabled,
  onToggle,
  onClose,
}: CollectionPickerMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [layout, setLayout] = useState<{
    left: number;
    top: number;
    above: boolean;
  } | null>(null);

  useLayoutEffect(() => {
    if (!open) {
      setLayout(null);
      return;
    }
    const anchor = anchorRef.current;
    if (!anchor) return;

    const update = () => {
      const rect = anchor.getBoundingClientRect();
      const menuHeight = Math.min(collections.length * 34 + 12, 240);
      const spaceAbove = rect.top;
      const spaceBelow = window.innerHeight - rect.bottom;
      const above = spaceAbove >= menuHeight || spaceAbove > spaceBelow;

      setLayout({
        left: rect.left + rect.width / 2,
        top: above ? rect.top - 8 : rect.bottom + 8,
        above,
      });
    };

    update();
    window.addEventListener("resize", update);
    window.addEventListener("scroll", update, true);
    return () => {
      window.removeEventListener("resize", update);
      window.removeEventListener("scroll", update, true);
    };
  }, [open, anchorRef, collections.length]);

  useLayoutEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      const target = e.target as Node;
      if (menuRef.current?.contains(target)) return;
      if (anchorRef.current?.contains(target)) return;
      onClose();
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [open, onClose, anchorRef]);

  if (!open || !layout) return null;

  return createPortal(
    <div
      ref={menuRef}
      className="fixed z-[9999] min-w-[188px] rounded-xl border border-border/60 bg-surface py-1 shadow-2xl"
      style={{
        left: layout.left,
        top: layout.top,
        transform: layout.above ? "translate(-50%, -100%)" : "translate(-50%, 0)",
      }}
      onMouseDown={(e) => e.stopPropagation()}
    >
      {collections.map((c) => {
        const inCollection = itemCollectionIds.includes(c.id);
        const isFlashing = flashCollectionId === c.id;
        const rowBusy = busyCollectionId === c.id;
        return (
          <button
            key={c.id}
            type="button"
            disabled={disabled}
            onClick={(e) => {
              e.stopPropagation();
              onToggle(c.id, inCollection);
            }}
            className={cn(
              "flex w-full items-center gap-2 px-3 py-2 text-left text-xs hover:bg-surface-elevated disabled:opacity-50",
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
            ) : inCollection || isFlashing ? (
              <Check className="h-3 w-3 text-accent" />
            ) : null}
          </button>
        );
      })}
    </div>,
    document.body,
  );
}
