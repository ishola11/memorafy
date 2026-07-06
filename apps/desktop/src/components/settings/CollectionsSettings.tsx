import { Loader2, Pencil, Trash2 } from "lucide-react";
import { useState } from "react";
import type { Collection } from "@memorafy/shared-types";
import { createCollection, deleteCollection, updateCollection } from "@/lib/api";
import { cn } from "@/lib/utils";

const COLOR_PRESETS = [
  "#6366f1",
  "#8b5cf6",
  "#ec4899",
  "#ef4444",
  "#f59e0b",
  "#22c55e",
  "#06b6d4",
  "#64748b",
];

interface CollectionsSettingsProps {
  collections: Collection[];
  onChanged: () => Promise<void>;
}

export function CollectionsSettings({ collections, onChanged }: CollectionsSettingsProps) {
  const [name, setName] = useState("");
  const [color, setColor] = useState(COLOR_PRESETS[0]!);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState("");
  const [editColor, setEditColor] = useState(COLOR_PRESETS[0]!);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    setLoading(true);
    setError(null);
    try {
      await createCollection({ name: name.trim(), color });
      setName("");
      await onChanged();
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const startEdit = (c: Collection) => {
    setEditingId(c.id);
    setEditName(c.name);
    setEditColor(c.color);
  };

  const handleSaveEdit = async () => {
    if (!editingId || !editName.trim()) return;
    setLoading(true);
    setError(null);
    try {
      await updateCollection({ id: editingId, name: editName.trim(), color: editColor });
      setEditingId(null);
      await onChanged();
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleDelete = async (id: string, collectionName: string) => {
    if (!window.confirm(`Delete "${collectionName}"? Items won't be deleted.`)) return;
    setLoading(true);
    setError(null);
    try {
      await deleteCollection(id);
      if (editingId === id) setEditingId(null);
      await onChanged();
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-base font-semibold text-zinc-900 dark:text-zinc-50">Collections</h2>
        <p className="mt-1 text-sm text-muted">
          Organize clips into labeled groups. Filter by collection in the tray panel.
        </p>
      </div>

      <form onSubmit={(e) => void handleCreate(e)} className="rounded-xl border border-border/60 bg-surface-elevated/50 p-4">
        <p className="mb-3 text-xs font-medium uppercase tracking-wide text-muted">New collection</p>
        <div className="flex flex-col gap-3 sm:flex-row sm:items-end">
          <div className="flex-1">
            <label className="mb-1.5 block text-xs text-muted">Name</label>
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. Work, Ideas…"
              className="w-full rounded-lg border border-border/60 bg-surface px-3 py-2 text-sm outline-none focus:border-accent/50 focus:ring-1 focus:ring-accent/30"
            />
          </div>
          <div>
            <label className="mb-1.5 block text-xs text-muted">Color</label>
            <div className="flex flex-wrap gap-1.5">
              {COLOR_PRESETS.map((preset) => (
                <button
                  key={preset}
                  type="button"
                  onClick={() => setColor(preset)}
                  className={cn(
                    "h-7 w-7 rounded-md ring-2 ring-offset-2 ring-offset-surface transition-all",
                    color === preset ? "ring-accent" : "ring-transparent hover:ring-border",
                  )}
                  style={{ backgroundColor: preset }}
                  aria-label={`Color ${preset}`}
                />
              ))}
            </div>
          </div>
          <button
            type="submit"
            disabled={loading || !name.trim()}
            className="shrink-0 rounded-lg bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent/90 disabled:opacity-50"
          >
            {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : "Create"}
          </button>
        </div>
      </form>

      <div className="space-y-2">
        {collections.length === 0 ? (
          <p className="rounded-xl border border-dashed border-border/60 px-4 py-8 text-center text-sm text-muted">
            No collections yet. Create one above.
          </p>
        ) : (
          collections.map((c) => (
            <div
              key={c.id}
              className="flex items-center gap-3 rounded-xl border border-border/60 bg-surface-elevated/40 px-3 py-2.5"
            >
              {editingId === c.id ? (
                <>
                  <div className="flex flex-1 flex-wrap items-center gap-2">
                    <input
                      value={editName}
                      onChange={(e) => setEditName(e.target.value)}
                      className="min-w-[120px] flex-1 rounded-lg border border-border/60 bg-surface px-2.5 py-1.5 text-sm outline-none focus:border-accent/50"
                    />
                    <div className="flex gap-1">
                      {COLOR_PRESETS.map((preset) => (
                        <button
                          key={preset}
                          type="button"
                          onClick={() => setEditColor(preset)}
                          className={cn(
                            "h-5 w-5 rounded ring-2 ring-offset-1 ring-offset-surface",
                            editColor === preset ? "ring-accent" : "ring-transparent",
                          )}
                          style={{ backgroundColor: preset }}
                        />
                      ))}
                    </div>
                  </div>
                  <button
                    type="button"
                    onClick={() => void handleSaveEdit()}
                    disabled={loading}
                    className="rounded-lg bg-accent px-3 py-1.5 text-xs font-medium text-white"
                  >
                    Save
                  </button>
                  <button
                    type="button"
                    onClick={() => setEditingId(null)}
                    className="text-xs text-muted hover:text-zinc-700 dark:hover:text-zinc-200"
                  >
                    Cancel
                  </button>
                </>
              ) : (
                <>
                  <span
                    className="h-3 w-3 shrink-0 rounded-full"
                    style={{ backgroundColor: c.color }}
                  />
                  <span className="flex-1 text-sm font-medium">{c.name}</span>
                  <span className="text-xs text-muted">{c.itemCount} items</span>
                  <button
                    type="button"
                    onClick={() => startEdit(c)}
                    className="rounded-md p-1.5 text-muted hover:bg-surface hover:text-zinc-700 dark:hover:text-zinc-200"
                  >
                    <Pencil className="h-3.5 w-3.5" />
                  </button>
                  <button
                    type="button"
                    onClick={() => void handleDelete(c.id, c.name)}
                    className="rounded-md p-1.5 text-muted hover:bg-red-500/10 hover:text-red-500"
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </button>
                </>
              )}
            </div>
          ))
        )}
      </div>

      {error && <p className="text-sm text-red-500">{error}</p>}
    </div>
  );
}
