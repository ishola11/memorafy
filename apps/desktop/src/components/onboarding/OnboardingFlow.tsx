import { useEffect, useState } from "react";
import { ClipboardList, Cloud, Keyboard, Lock, ShieldCheck, Sparkles } from "lucide-react";
import { AuthForms } from "@/components/settings/AuthForms";
import { getSyncState, setLaunchAtLogin, setOnboardingCompleted } from "@/lib/api";
import { cn } from "@/lib/utils";

const IS_MAC = navigator.userAgent.includes("Mac");
const QUICK_PASTE_SHORTCUT = IS_MAC ? "⌘ ⇧ V" : "Ctrl + Shift + V";

type Step = 0 | 1 | 2;

function StepDots({ step }: { step: Step }) {
  return (
    <div className="flex justify-center gap-1.5">
      {[0, 1, 2].map((i) => (
        <span
          key={i}
          className={cn(
            "h-1.5 rounded-full transition-all",
            i === step ? "w-6 bg-accent" : "w-1.5 bg-border",
          )}
        />
      ))}
    </div>
  );
}

function FeatureRow({
  icon: Icon,
  title,
  children,
}: {
  icon: typeof Cloud;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex gap-3 rounded-xl border border-border/60 bg-surface-elevated/40 p-4 text-left">
      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-accent/15">
        <Icon className="h-5 w-5 text-accent" />
      </div>
      <div>
        <p className="text-sm font-medium">{title}</p>
        <p className="mt-0.5 text-xs leading-relaxed text-muted">{children}</p>
      </div>
    </div>
  );
}

/**
 * First-launch welcome flow. Completion is persisted the moment the user
 * moves past the final step (or skips sync), so a crash mid-flow doesn't
 * trap them in onboarding forever — while closing the window early simply
 * shows the flow again next launch.
 */
export function OnboardingFlow({ onDone }: { onDone: () => void }) {
  const [step, setStep] = useState<Step>(0);
  const [syncConfigured, setSyncConfigured] = useState(true);
  const [launchAtLogin, setLaunchAtLoginEnabled] = useState(true);

  useEffect(() => {
    void getSyncState()
      .then((s) => setSyncConfigured(s.configured))
      .catch(() => setSyncConfigured(false));
  }, []);

  const finish = () => {
    void setLaunchAtLogin(launchAtLogin).catch((err) =>
      console.error("could not set launch at login:", err),
    );
    // Persist first, then transition — if persisting fails we still let the
    // user through this session and log the retry-able failure.
    void setOnboardingCompleted().catch((err) =>
      console.error("could not persist onboarding completion:", err),
    );
    onDone();
  };

  return (
    <div className="flex h-full items-center justify-center overflow-y-auto bg-surface px-8 py-8 text-zinc-900 dark:text-zinc-100">
      <div className="w-full max-w-md space-y-6">
        {step === 0 && (
          <div className="space-y-5 text-center">
            <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-2xl bg-accent/15">
              <Sparkles className="h-7 w-7 text-accent" />
            </div>
            <div>
              <h1 className="text-xl font-semibold tracking-tight">Welcome to Memora</h1>
              <p className="mt-1.5 text-sm text-muted">
                Your personal cross-device memory for everything you copy.
              </p>
            </div>
            <div className="space-y-2.5">
              <FeatureRow icon={ClipboardList} title="Clipboard history">
                Everything you copy (text, links, code, images) is saved locally and
                searchable instantly.
              </FeatureRow>
              <FeatureRow icon={Keyboard} title="Quick Paste">
                Press <span className="font-medium text-zinc-800 dark:text-zinc-200">{QUICK_PASTE_SHORTCUT}</span>{" "}
                anywhere to search your history and paste. Memora lives in your{" "}
                {IS_MAC ? "menu bar" : "system tray"}. It has no main window.
              </FeatureRow>
              <FeatureRow icon={Cloud} title="Cross-device sync (optional)">
                Sign in to sync clips between your computers in real time. Works fully
                offline without an account.
              </FeatureRow>
            </div>
            <label className="flex cursor-pointer items-start gap-3 rounded-xl border border-border/60 bg-surface-elevated/40 px-4 py-3 text-left">
              <input
                type="checkbox"
                checked={launchAtLogin}
                onChange={(e) => setLaunchAtLoginEnabled(e.target.checked)}
                className="mt-0.5 h-4 w-4 shrink-0 rounded border-border accent-accent"
              />
              <div>
                <p className="text-sm font-medium">Launch at login</p>
                <p className="mt-0.5 text-xs leading-relaxed text-muted">
                  Start Memora when you sign in to this computer so your clipboard history
                  is always ready in the {IS_MAC ? "menu bar" : "system tray"}.
                </p>
              </div>
            </label>
            <button
              type="button"
              onClick={() => setStep(1)}
              className="w-full rounded-xl bg-accent py-2.5 text-sm font-medium text-white hover:bg-accent/90"
            >
              Continue
            </button>
          </div>
        )}

        {step === 1 && (
          <div className="space-y-5 text-center">
            <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-2xl bg-accent/15">
              <ShieldCheck className="h-7 w-7 text-accent" />
            </div>
            <div>
              <h1 className="text-xl font-semibold tracking-tight">Your data, your rules</h1>
              <p className="mt-1.5 text-sm text-muted">How Memora handles what you copy.</p>
            </div>
            <div className="space-y-2.5">
              <FeatureRow icon={Lock} title="Local-first">
                Your history lives in a database on this device. Nothing leaves it unless you
                turn on cloud sync.
              </FeatureRow>
              <FeatureRow icon={ShieldCheck} title="Passwords stay out">
                Content marked confidential by password managers is never captured. You can
                also pause capture any time from the tray.
              </FeatureRow>
              <FeatureRow icon={Cloud} title="End-to-end encrypted sync">
                If you enable sync, clips are encrypted on your device with a key derived
                from your password. The server can't read them. One caveat: resetting a
                forgotten password makes previously synced clips unreadable.
              </FeatureRow>
            </div>
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => setStep(0)}
                className="w-28 rounded-xl border border-border/60 py-2.5 text-sm hover:bg-surface-elevated"
              >
                Back
              </button>
              <button
                type="button"
                onClick={() => setStep(2)}
                className="flex-1 rounded-xl bg-accent py-2.5 text-sm font-medium text-white hover:bg-accent/90"
              >
                Continue
              </button>
            </div>
          </div>
        )}

        {step === 2 && (
          <div className="space-y-5">
            <div className="text-center">
              <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-2xl bg-accent/15">
                <Cloud className="h-7 w-7 text-accent" />
              </div>
              <h1 className="mt-4 text-xl font-semibold tracking-tight">Sync across devices?</h1>
              <p className="mt-1.5 text-sm text-muted">
                Create an account or sign in, or skip and use Memora locally. You can enable
                sync later in Settings.
              </p>
            </div>
            {syncConfigured ? (
              <>
                <AuthForms configured initialEmail={null} onSignedIn={() => finish()} />
                <div className="flex items-center justify-between">
                  <button
                    type="button"
                    onClick={() => setStep(1)}
                    className="text-xs text-muted underline-offset-2 hover:underline"
                  >
                    Back
                  </button>
                  <button
                    type="button"
                    onClick={finish}
                    className="text-xs text-muted underline-offset-2 hover:underline"
                  >
                    Skip for now (use locally)
                  </button>
                </div>
              </>
            ) : (
              <>
                <p className="rounded-xl border border-border/60 bg-surface-elevated/40 p-4 text-center text-sm text-muted">
                  Cloud sync isn't set up in this build. Memora works fully offline. You can
                  configure sync later; see the README for self-hosting instructions.
                </p>
                <button
                  type="button"
                  onClick={finish}
                  className="w-full rounded-xl bg-accent py-2.5 text-sm font-medium text-white hover:bg-accent/90"
                >
                  Start using Memora
                </button>
              </>
            )}
          </div>
        )}

        <StepDots step={step} />
      </div>
    </div>
  );
}
