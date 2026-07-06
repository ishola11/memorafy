import { useEffect, useRef, useState } from "react";
import { OnboardingFlow } from "@/components/onboarding/OnboardingFlow";
import { QuickPasteLauncher } from "@/components/quick-paste/QuickPasteLauncher";
import { SettingsPanel } from "@/components/settings/SettingsPanel";
import { TrayPanel, TrayShell } from "@/components/tray/TrayPanel";
import { ActionToast } from "@/components/ui/ActionToast";
import { SyncToast } from "@/components/ui/SyncToast";
import {
  getOnboardingCompleted,
  onCollectionsUpdated,
  onItemsUpdated,
  onQuickPasteVisibility,
  onSyncFinished,
  onThemeChanged,
  onTrayVisibility,
} from "@/lib/api";
import { applyTheme, initTheme, watchSystemTheme } from "@/lib/theme";
import type { ThemePreference } from "@memorafy/shared-types";
import { useActionToastStore } from "@/stores/action-toast-store";
import { useAppStore } from "@/stores/app-store";

/** Settings window content, gated behind first-launch onboarding. */
function SettingsWindow() {
  // null = still checking; avoids flashing settings before the welcome flow.
  const [onboarded, setOnboarded] = useState<boolean | null>(null);

  useEffect(() => {
    void getOnboardingCompleted()
      .then(setOnboarded)
      // If the check fails, don't lock the user out of settings.
      .catch(() => setOnboarded(true));
  }, []);

  if (onboarded === null) {
    return <div className="h-full bg-surface" />;
  }
  if (!onboarded) {
    return <OnboardingFlow onDone={() => setOnboarded(true)} />;
  }
  return <SettingsPanel />;
}

function getWindowMode(): "quick-paste" | "tray" | "settings" | "main" {
  const params = new URLSearchParams(window.location.search);
  const mode = params.get("window");
  if (mode === "quick-paste") return "quick-paste";
  if (mode === "tray") return "tray";
  if (mode === "settings") return "settings";
  return "main";
}

export default function App() {
  const windowMode = getWindowMode();
  const { setTrayOpen, refresh } = useAppStore();
  const themePrefRef = useRef<ThemePreference>("system");

  useEffect(() => {
    void initTheme().then((pref) => {
      themePrefRef.current = pref;
    });

    if (windowMode === "quick-paste") {
      useAppStore.setState({ quickPasteOpen: true });
    }
    if (windowMode === "tray") {
      setTrayOpen(true);
    }

    const unsubs: Array<() => void> = [];

    unsubs.push(
      watchSystemTheme(() => {
        if (themePrefRef.current === "system") {
          applyTheme("system");
        }
      }),
    );

    void onThemeChanged((preference) => {
      themePrefRef.current = preference;
      applyTheme(preference);
    }).then((unlisten) => unsubs.push(unlisten));

    void onQuickPasteVisibility((visible) => {
      useAppStore.setState({ quickPasteOpen: visible });
    }).then((unlisten) => unsubs.push(unlisten));

    void onTrayVisibility((visible) => {
      setTrayOpen(visible);
    }).then((unlisten) => unsubs.push(unlisten));

    void onItemsUpdated(() => {
      void refresh();
    }).then((unlisten) => unsubs.push(unlisten));

    void onCollectionsUpdated(() => {
      void refresh();
    }).then((unlisten) => unsubs.push(unlisten));

    void onSyncFinished((message) => {
      useActionToastStore
        .getState()
        .showActionToast(message, message.includes("failed") ? "error" : "success");
      void refresh();
    }).then((unlisten) => unsubs.push(unlisten));

    return () => {
      unsubs.forEach((fn) => fn());
    };
  }, [setTrayOpen, refresh, windowMode]);

  if (windowMode === "settings") {
    return (
      <TrayShell className="h-full overflow-hidden">
        <SettingsWindow />
      </TrayShell>
    );
  }

  if (windowMode === "tray") {
    return (
      <TrayShell className="box-border p-3">
        <TrayPanel />
        <ActionToast />
        <SyncToast />
      </TrayShell>
    );
  }

  if (windowMode === "quick-paste") {
    return (
      <TrayShell className="h-full overflow-hidden">
        <QuickPasteLauncher />
        <ActionToast />
        <SyncToast />
      </TrayShell>
    );
  }

  return (
    <TrayShell>
      <QuickPasteLauncher />
      <TrayPanel />
      <ActionToast />
      <SyncToast />
    </TrayShell>
  );
}
