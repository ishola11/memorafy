import { Plus } from "lucide-react";

interface SnippetsTabHeaderProps {
  onNewSnippet: () => void;
}

export function SnippetsTabHeader({ onNewSnippet }: SnippetsTabHeaderProps) {
  return (
    <div className="flex shrink-0 items-center justify-between border-b border-border/60 px-4 py-2">
      <span className="text-xs text-muted">Reusable saved text</span>
      <button
        type="button"
        onClick={onNewSnippet}
        className="inline-flex items-center gap-1 rounded-lg bg-accent px-2.5 py-1.5 text-xs font-medium text-white hover:bg-accent/90"
      >
        <Plus className="h-3.5 w-3.5" />
        New snippet
      </button>
    </div>
  );
}
