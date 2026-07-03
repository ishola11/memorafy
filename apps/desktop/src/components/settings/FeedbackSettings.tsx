import { useEffect, useState } from "react";
import { Bug, ExternalLink, Lightbulb, Loader2, ShieldCheck } from "lucide-react";
import { getDiagnostics, submitFeedback } from "@/lib/api";
import { cn } from "@/lib/utils";
import type { Diagnostics, FeedbackKind, FeedbackSection } from "@memora/shared-types";

interface FieldSpec {
  key: string;
  label: string;
  placeholder: string;
  multiline: boolean;
  required?: boolean;
}

const BUG_FIELDS: FieldSpec[] = [
  { key: "description", label: "Description", placeholder: "What went wrong?", multiline: true, required: true },
  { key: "steps", label: "Steps to reproduce", placeholder: "1. Copy some text\n2. …", multiline: true },
  { key: "expected", label: "Expected result", placeholder: "What you expected to happen", multiline: true },
  { key: "actual", label: "Actual result", placeholder: "What actually happened", multiline: true },
];

const FEATURE_FIELDS: FieldSpec[] = [
  { key: "idea", label: "Idea", placeholder: "What would you like Memora to do?", multiline: true, required: true },
  { key: "why", label: "Why it would help", placeholder: "The problem it solves for you", multiline: true },
  { key: "workflow", label: "Your workflow", placeholder: "How you'd use it day to day", multiline: true },
];

export function FeedbackSettings({ userEmail }: { userEmail: string | null }) {
  const [kind, setKind] = useState<FeedbackKind>("bug");
  const [title, setTitle] = useState("");
  const [fields, setFields] = useState<Record<string, string>>({});
  const [email, setEmail] = useState(userEmail ?? "");
  const [includeDiagnostics, setIncludeDiagnostics] = useState(false);
  const [includeLogs, setIncludeLogs] = useState(false);
  const [diagnostics, setDiagnostics] = useState<Diagnostics | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const fieldSpecs = kind === "bug" ? BUG_FIELDS : FEATURE_FIELDS;

  // Refresh the preview whenever consent options change, so the user always
  // sees exactly what would be attached — nothing hidden.
  useEffect(() => {
    if (!includeDiagnostics) {
      setDiagnostics(null);
      return;
    }
    void getDiagnostics(includeLogs)
      .then(setDiagnostics)
      .catch(() => {
        setDiagnostics(null);
        setError("Couldn't collect diagnostics. You can still submit without them.");
      });
  }, [includeDiagnostics, includeLogs]);

  const switchKind = (next: FeedbackKind) => {
    setKind(next);
    setFields({});
    setMessage(null);
    setError(null);
  };

  const requiredFilled =
    title.trim().length > 0 &&
    fieldSpecs.filter((f) => f.required).every((f) => (fields[f.key] ?? "").trim().length > 0);

  const handleSubmit = async () => {
    setSubmitting(true);
    setMessage(null);
    setError(null);
    try {
      const sections: FeedbackSection[] = fieldSpecs
        .map((f) => ({ label: f.label, value: fields[f.key] ?? "" }))
        .filter((s) => s.value.trim().length > 0);

      await submitFeedback({
        kind,
        title: title.trim(),
        sections,
        contactEmail: email.trim() || null,
        diagnostics: includeDiagnostics ? diagnostics : null,
      });
      setMessage(
        "Your browser opened with a pre-filled report. Review it and press Submit there to send it.",
      );
      setTitle("");
      setFields({});
    } catch (err) {
      setError(String(err));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-base font-semibold">Feedback</h2>
        <p className="mt-1 text-sm text-muted">
          Found a bug or have an idea? Reports open as a pre-filled GitHub issue you review
          before anything is sent.
        </p>
      </div>

      <div className="grid grid-cols-2 gap-2">
        {(
          [
            { id: "bug", label: "Report a bug", icon: Bug },
            { id: "feature", label: "Feature request", icon: Lightbulb },
          ] as const
        ).map((opt) => {
          const Icon = opt.icon;
          const active = kind === opt.id;
          return (
            <button
              key={opt.id}
              type="button"
              onClick={() => switchKind(opt.id)}
              className={cn(
                "flex items-center justify-center gap-2 rounded-xl border px-3 py-3 text-sm transition-colors",
                active
                  ? "border-accent/50 bg-accent/10 text-accent ring-1 ring-accent/25"
                  : "border-border/60 bg-surface-elevated/40 text-muted hover:border-border hover:text-zinc-800 dark:hover:text-zinc-200",
              )}
            >
              <Icon className="h-4 w-4" />
              {opt.label}
            </button>
          );
        })}
      </div>

      <div className="space-y-4">
        <div>
          <label className="mb-1.5 block text-xs text-muted">Title</label>
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder={kind === "bug" ? "Short summary of the problem" : "Short summary of the idea"}
            className="w-full rounded-lg border border-border/60 bg-surface px-3 py-2.5 text-sm outline-none focus:border-accent/50 focus:ring-1 focus:ring-accent/30"
          />
        </div>

        {fieldSpecs.map((f) => (
          <div key={f.key}>
            <label className="mb-1.5 block text-xs text-muted">
              {f.label}
              {f.required && <span className="ml-1 text-red-400">*</span>}
            </label>
            <textarea
              value={fields[f.key] ?? ""}
              onChange={(e) => setFields((prev) => ({ ...prev, [f.key]: e.target.value }))}
              placeholder={f.placeholder}
              rows={f.multiline ? 3 : 1}
              className="w-full resize-y rounded-lg border border-border/60 bg-surface px-3 py-2.5 text-sm outline-none focus:border-accent/50 focus:ring-1 focus:ring-accent/30"
            />
          </div>
        ))}

        <div>
          <label className="mb-1.5 block text-xs text-muted">Contact email (optional)</label>
          <input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="So we can follow up"
            className="w-full rounded-lg border border-border/60 bg-surface px-3 py-2.5 text-sm outline-none focus:border-accent/50 focus:ring-1 focus:ring-accent/30"
          />
        </div>
      </div>

      <div className="space-y-3 rounded-xl border border-border/60 bg-surface-elevated/40 p-4">
        <label className="flex cursor-pointer items-center justify-between">
          <div>
            <p className="text-sm font-medium">Include diagnostics</p>
            <p className="text-xs text-muted">App version, OS, sync status, device ID.</p>
          </div>
          <input
            type="checkbox"
            checked={includeDiagnostics}
            onChange={(e) => setIncludeDiagnostics(e.target.checked)}
            className="h-4 w-4 rounded border-border accent-accent"
          />
        </label>

        {includeDiagnostics && (
          <label className="flex cursor-pointer items-center justify-between">
            <div>
              <p className="text-sm font-medium">Include recent logs</p>
              <p className="text-xs text-muted">The last ~40 log lines from this device.</p>
            </div>
            <input
              type="checkbox"
              checked={includeLogs}
              onChange={(e) => setIncludeLogs(e.target.checked)}
              className="h-4 w-4 rounded border-border accent-accent"
            />
          </label>
        )}

        {includeDiagnostics && diagnostics && (
          <div className="rounded-lg border border-border/60 bg-surface p-3">
            <p className="mb-2 text-xs font-medium uppercase tracking-wide text-muted">
              Exactly what will be included
            </p>
            <pre className="max-h-48 overflow-auto whitespace-pre-wrap break-all text-xs text-muted">
              {[
                `App version: ${diagnostics.appVersion}`,
                `OS: ${diagnostics.os} (${diagnostics.arch})`,
                `Sync configured: ${diagnostics.syncConfigured}`,
                `Signed in: ${diagnostics.loggedIn}`,
                `Pending changes: ${diagnostics.pendingCount}`,
                diagnostics.deviceId ? `Device ID: ${diagnostics.deviceId}` : null,
                diagnostics.accountId ? `Account ID: ${diagnostics.accountId}` : null,
                diagnostics.recentLogs ? `\nRecent logs:\n${diagnostics.recentLogs}` : null,
              ]
                .filter(Boolean)
                .join("\n")}
            </pre>
          </div>
        )}

        <p className="flex items-start gap-2 text-xs text-muted">
          <ShieldCheck className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          Only the information shown above will be submitted — and only after you review and
          confirm it in your browser. Nothing is sent in the background.
        </p>
      </div>

      {error && <p className="text-sm text-red-500">{error}</p>}
      {message && <p className="text-sm text-green-600 dark:text-green-400">{message}</p>}

      <button
        type="button"
        onClick={() => void handleSubmit()}
        disabled={submitting || !requiredFilled}
        className="flex w-full items-center justify-center gap-2 rounded-xl bg-accent py-2.5 text-sm font-medium text-white transition-colors hover:bg-accent/90 disabled:opacity-50"
      >
        {submitting ? <Loader2 className="h-4 w-4 animate-spin" /> : <ExternalLink className="h-4 w-4" />}
        {submitting ? "Preparing…" : "Review & submit on GitHub"}
      </button>
    </div>
  );
}
