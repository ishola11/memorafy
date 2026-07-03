import { useState } from "react";
import { Eye, EyeOff, KeyRound, Loader2, MailCheck } from "lucide-react";
import {
  authChangePassword,
  authLogin,
  authRequestPasswordReset,
  authResendConfirmation,
  authSignup,
} from "@/lib/api";
import { cn } from "@/lib/utils";
import type { SyncState } from "@memora/shared-types";

type AuthMode = "signin" | "signup" | "forgot" | "confirm-pending" | "reset-sent";

const inputClass =
  "w-full rounded-lg border border-border/60 bg-surface px-3 py-2.5 text-sm outline-none focus:border-accent/50 focus:ring-1 focus:ring-accent/30";

function PasswordInput({
  value,
  onChange,
  placeholder,
  autoComplete,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  autoComplete?: string;
}) {
  const [visible, setVisible] = useState(false);
  const Icon = visible ? EyeOff : Eye;
  return (
    <div className="relative">
      <input
        type={visible ? "text" : "password"}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        autoComplete={autoComplete}
        required
        className={cn(inputClass, "pr-10")}
      />
      <button
        type="button"
        tabIndex={-1}
        aria-label={visible ? "Hide password" : "Show password"}
        onClick={() => setVisible((v) => !v)}
        className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted hover:text-zinc-700 dark:hover:text-zinc-200"
      >
        <Icon className="h-4 w-4" />
      </button>
    </div>
  );
}

export function AuthForms({
  configured,
  initialEmail,
  onSignedIn,
}: {
  configured: boolean;
  initialEmail?: string | null;
  onSignedIn: (state: SyncState) => void;
}) {
  const [mode, setMode] = useState<AuthMode>("signin");
  const [email, setEmail] = useState(initialEmail ?? "");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const switchMode = (next: AuthMode) => {
    setMode(next);
    setError(null);
    setNotice(null);
    setPassword("");
    setConfirmPassword("");
  };

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await action();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleSignIn = (e: React.FormEvent) => {
    e.preventDefault();
    void run(async () => {
      const state = await authLogin(email, password);
      setPassword("");
      onSignedIn(state);
    });
  };

  const handleSignUp = (e: React.FormEvent) => {
    e.preventDefault();
    if (password !== confirmPassword) {
      setError("Passwords don't match.");
      return;
    }
    if (password.length < 8) {
      setError("Password must be at least 8 characters.");
      return;
    }
    void run(async () => {
      const result = await authSignup(email, password);
      setPassword("");
      setConfirmPassword("");
      if (result.needsEmailConfirmation) {
        setMode("confirm-pending");
      } else {
        onSignedIn(result);
      }
    });
  };

  const handleForgot = (e: React.FormEvent) => {
    e.preventDefault();
    void run(async () => {
      await authRequestPasswordReset(email);
      setMode("reset-sent");
    });
  };

  const handleResend = () => {
    void run(async () => {
      await authResendConfirmation(email);
      setNotice("Confirmation email re-sent. Check your inbox (and spam folder).");
    });
  };

  if (mode === "confirm-pending") {
    return (
      <div className="space-y-4 rounded-xl border border-border/60 bg-surface-elevated/40 p-5 text-center">
        <MailCheck className="mx-auto h-8 w-8 text-accent" />
        <div>
          <p className="text-sm font-medium">Check your inbox</p>
          <p className="mt-1 text-sm text-muted">
            We sent a confirmation link to <span className="font-medium">{email}</span>. Click
            it, then come back and sign in.
          </p>
        </div>
        {notice && <p className="text-xs text-green-600 dark:text-green-400">{notice}</p>}
        {error && <p className="text-xs text-red-500">{error}</p>}
        <div className="flex flex-col gap-2">
          <button
            type="button"
            onClick={() => switchMode("signin")}
            className="w-full rounded-xl bg-accent py-2.5 text-sm font-medium text-white hover:bg-accent/90"
          >
            I've confirmed — sign in
          </button>
          <button
            type="button"
            onClick={handleResend}
            disabled={busy}
            className="text-xs text-muted underline-offset-2 hover:underline disabled:opacity-50"
          >
            {busy ? "Sending…" : "Resend the email"}
          </button>
        </div>
      </div>
    );
  }

  if (mode === "reset-sent") {
    return (
      <div className="space-y-4 rounded-xl border border-border/60 bg-surface-elevated/40 p-5 text-center">
        <MailCheck className="mx-auto h-8 w-8 text-accent" />
        <div>
          <p className="text-sm font-medium">Reset link sent</p>
          <p className="mt-1 text-sm text-muted">
            If an account exists for <span className="font-medium">{email}</span>, a password
            reset link is on its way. Follow it in your browser, then sign in here with your
            new password.
          </p>
        </div>
        <button
          type="button"
          onClick={() => switchMode("signin")}
          className="w-full rounded-xl bg-accent py-2.5 text-sm font-medium text-white hover:bg-accent/90"
        >
          Back to sign in
        </button>
      </div>
    );
  }

  if (mode === "forgot") {
    return (
      <form onSubmit={handleForgot} className="space-y-4">
        <div>
          <p className="text-sm font-medium">Reset your password</p>
          <p className="mt-1 text-xs text-muted">
            Enter your account email and we'll send you a reset link.
          </p>
        </div>
        <div>
          <label className="mb-1.5 block text-xs text-muted">Email</label>
          <input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
            autoComplete="email"
            className={inputClass}
          />
        </div>
        {error && <p className="text-sm text-red-500">{error}</p>}
        <button
          type="submit"
          disabled={busy || !configured}
          className="flex w-full items-center justify-center gap-2 rounded-xl bg-accent py-2.5 text-sm font-medium text-white hover:bg-accent/90 disabled:opacity-50"
        >
          {busy && <Loader2 className="h-4 w-4 animate-spin" />}
          Send reset link
        </button>
        <button
          type="button"
          onClick={() => switchMode("signin")}
          className="w-full text-center text-xs text-muted underline-offset-2 hover:underline"
        >
          Back to sign in
        </button>
      </form>
    );
  }

  const isSignup = mode === "signup";

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-2 gap-1 rounded-xl border border-border/60 bg-surface-elevated/40 p-1">
        {(
          [
            { id: "signin", label: "Sign in" },
            { id: "signup", label: "Create account" },
          ] as const
        ).map((opt) => (
          <button
            key={opt.id}
            type="button"
            onClick={() => switchMode(opt.id)}
            className={cn(
              "rounded-lg py-2 text-sm transition-colors",
              mode === opt.id
                ? "bg-surface font-medium text-zinc-900 shadow-sm dark:text-zinc-100"
                : "text-muted hover:text-zinc-800 dark:hover:text-zinc-200",
            )}
          >
            {opt.label}
          </button>
        ))}
      </div>

      <form onSubmit={isSignup ? handleSignUp : handleSignIn} className="space-y-4">
        <div>
          <label className="mb-1.5 block text-xs text-muted">Email</label>
          <input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
            disabled={!configured}
            autoComplete="email"
            className={inputClass}
          />
        </div>
        <div>
          <label className="mb-1.5 block text-xs text-muted">Password</label>
          <PasswordInput
            value={password}
            onChange={setPassword}
            placeholder={isSignup ? "At least 8 characters" : undefined}
            autoComplete={isSignup ? "new-password" : "current-password"}
          />
        </div>
        {isSignup && (
          <div>
            <label className="mb-1.5 block text-xs text-muted">Confirm password</label>
            <PasswordInput
              value={confirmPassword}
              onChange={setConfirmPassword}
              autoComplete="new-password"
            />
          </div>
        )}
        {error && <p className="text-sm text-red-500">{error}</p>}
        <button
          type="submit"
          disabled={busy || !configured}
          className="flex w-full items-center justify-center gap-2 rounded-xl bg-accent py-2.5 text-sm font-medium text-white hover:bg-accent/90 disabled:opacity-50"
        >
          {busy && <Loader2 className="h-4 w-4 animate-spin" />}
          {isSignup ? "Create account" : "Sign in to sync"}
        </button>
        {!isSignup && (
          <button
            type="button"
            onClick={() => switchMode("forgot")}
            className="w-full text-center text-xs text-muted underline-offset-2 hover:underline"
          >
            Forgot your password?
          </button>
        )}
      </form>
    </div>
  );
}

/** Collapsible change-password block for the signed-in Account view. */
export function ChangePasswordForm() {
  const [open, setOpen] = useState(false);
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (newPassword !== confirmPassword) {
      setError("Passwords don't match.");
      return;
    }
    if (newPassword.length < 8) {
      setError("Password must be at least 8 characters.");
      return;
    }
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await authChangePassword(newPassword);
      setNewPassword("");
      setConfirmPassword("");
      setNotice("Password updated.");
      setOpen(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-3">
      <button
        type="button"
        onClick={() => {
          setOpen((v) => !v);
          setError(null);
          setNotice(null);
        }}
        className="flex w-full items-center justify-center gap-2 rounded-xl border border-border/60 py-2.5 text-sm hover:bg-surface-elevated"
      >
        <KeyRound className="h-4 w-4" />
        Change password
      </button>
      {notice && (
        <p className="text-center text-xs text-green-600 dark:text-green-400">{notice}</p>
      )}
      {open && (
        <form
          onSubmit={(e) => void handleSubmit(e)}
          className="space-y-3 rounded-xl border border-border/60 bg-surface-elevated/40 p-4"
        >
          <div>
            <label className="mb-1.5 block text-xs text-muted">New password</label>
            <PasswordInput
              value={newPassword}
              onChange={setNewPassword}
              placeholder="At least 8 characters"
              autoComplete="new-password"
            />
          </div>
          <div>
            <label className="mb-1.5 block text-xs text-muted">Confirm new password</label>
            <PasswordInput
              value={confirmPassword}
              onChange={setConfirmPassword}
              autoComplete="new-password"
            />
          </div>
          {error && <p className="text-sm text-red-500">{error}</p>}
          <button
            type="submit"
            disabled={busy}
            className="flex w-full items-center justify-center gap-2 rounded-xl bg-accent py-2 text-sm font-medium text-white hover:bg-accent/90 disabled:opacity-50"
          >
            {busy && <Loader2 className="h-4 w-4 animate-spin" />}
            Update password
          </button>
        </form>
      )}
    </div>
  );
}
