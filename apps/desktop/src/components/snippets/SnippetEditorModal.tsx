import { Loader2, X } from "lucide-react";
import { useEffect, useState } from "react";
import { createSnippet, getItem, updateSnippet } from "@/lib/api";
import { cn } from "@/lib/utils";

export interface SnippetEditorState {
  mode: "create" | "edit";
  snippetId?: string;
  initialTitle?: string;
  initialText?: string;
  initialTrigger?: string | null;
}

interface SnippetEditorModalProps {
  editor: SnippetEditorState | null;
  onClose: () => void;
  onSaved: () => void | Promise<void>;
}

export function SnippetEditorModal({ editor, onClose, onSaved }: SnippetEditorModalProps) {
  const [title, setTitle] = useState("");
  const [text, setText] = useState("");
  const [trigger, setTrigger] = useState("");
  const [loading, setLoading] = useState(false);
  const [loadingItem, setLoadingItem] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!editor) return;

    setError(null);
    if (editor.mode === "create") {
      setTitle(editor.initialTitle ?? "");
      setText(editor.initialText ?? "");
      setTrigger(editor.initialTrigger ?? "");
      return;
    }

    if (!editor.snippetId) return;
    setLoadingItem(true);
    void getItem(editor.snippetId)
      .then((item) => {
        if (!item) {
          setError("Snippet not found");
          return;
        }
        setTitle(item.displayTitle ?? "");
        setText(item.plainText ?? item.previewText ?? "");
        setTrigger(item.trigger ?? "");
      })
      .catch((err) => setError(String(err)))
      .finally(() => setLoadingItem(false));
  }, [editor]);

  if (!editor) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!title.trim() || !text.trim()) return;

    setLoading(true);
    setError(null);
    try {
      const triggerVal = trigger.trim() || null;
      if (editor.mode === "create") {
        await createSnippet({ title: title.trim(), text: text.trim(), trigger: triggerVal });
      } else if (editor.snippetId) {
        await updateSnippet({
          id: editor.snippetId,
          title: title.trim(),
          text: text.trim(),
          trigger: triggerVal,
        });
      }
      await onSaved();
      onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center bg-black/40 p-4 backdrop-blur-[2px]"
      onMouseDown={onClose}
    >
      <div
        className={cn(
          "w-full max-w-md overflow-hidden rounded-xl border border-border/60 bg-surface shadow-[var(--panel-shadow)]",
        )}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-border/60 px-4 py-3">
          <h2 className="text-sm font-semibold text-zinc-900 dark:text-zinc-50">
            {editor.mode === "create" ? "New snippet" : "Edit snippet"}
          </h2>
          <button
            type="button"
            onClick={onClose}
            className="rounded-md p-1 text-muted hover:bg-surface-elevated hover:text-zinc-700 dark:hover:text-zinc-200"
            aria-label="Close"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {loadingItem ? (
          <div className="flex items-center justify-center py-16">
            <Loader2 className="h-5 w-5 animate-spin text-muted" />
          </div>
        ) : (
          <form onSubmit={(e) => void handleSubmit(e)} className="space-y-4 p-4">
            <div>
              <label className="mb-1.5 block text-xs text-muted">Title</label>
              <input
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                placeholder="e.g. Email signature"
                autoFocus
                className="w-full rounded-lg border border-border/60 bg-surface-elevated px-3 py-2 text-sm outline-none focus:border-accent/50 focus:ring-1 focus:ring-accent/30"
              />
            </div>

            <div>
              <label className="mb-1.5 block text-xs text-muted">Body</label>
              <textarea
                value={text}
                onChange={(e) => setText(e.target.value)}
                placeholder="Snippet text…"
                rows={6}
                className="w-full resize-y rounded-lg border border-border/60 bg-surface-elevated px-3 py-2 text-sm outline-none focus:border-accent/50 focus:ring-1 focus:ring-accent/30"
              />
            </div>

            <div>
              <label className="mb-1.5 block text-xs text-muted">
                Trigger <span className="text-muted/70">(optional)</span>
              </label>
              <input
                value={trigger}
                onChange={(e) => setTrigger(e.target.value)}
                placeholder="e.g. sig"
                className="w-full rounded-lg border border-border/60 bg-surface-elevated px-3 py-2 text-sm outline-none focus:border-accent/50 focus:ring-1 focus:ring-accent/30"
              />
            </div>

            {error && <p className="text-sm text-red-500">{error}</p>}

            <div className="flex justify-end gap-2 pt-1">
              <button
                type="button"
                onClick={onClose}
                className="rounded-lg px-3 py-2 text-sm text-muted hover:bg-surface-elevated hover:text-zinc-700 dark:hover:text-zinc-200"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={loading || !title.trim() || !text.trim()}
                className="inline-flex items-center gap-2 rounded-lg bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent/90 disabled:opacity-50"
              >
                {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : "Save"}
              </button>
            </div>
          </form>
        )}
      </div>
    </div>
  );
}
