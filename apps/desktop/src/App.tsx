import { useEffect } from "react";
import { QuickPasteLauncher } from "@/components/quick-paste/QuickPasteLauncher";
import { SettingsPanel } from "@/components/settings/SettingsPanel";
import { TrayPanel, TrayShell } from "@/components/tray/TrayPanel";
import { SyncToast } from "@/components/ui/SyncToast";
import { onItemsUpdated, onQuickPasteVisibility, onTrayVisibility } from "@/lib/api";
import { useAppStore } from "@/stores/app-store";

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
  const { setQuickPasteOpen, setTrayOpen, refresh } = useAppStore();

  useEffect(() => {
    document.documentElement.classList.add("dark");

    if (windowMode === "quick-paste") {
      setQuickPasteOpen(true);
    }
    if (windowMode === "tray") {
      setTrayOpen(true);
    }

    const unsubs: Array<() => void> = [];

    void onQuickPasteVisibility((visible) => {
      setQuickPasteOpen(visible);
    }).then((unlisten) => unsubs.push(unlisten));

    void onTrayVisibility((visible) => {
      setTrayOpen(visible);
    }).then((unlisten) => unsubs.push(unlisten));

    void onItemsUpdated(() => {
      void refresh();
    }).then((unlisten) => unsubs.push(unlisten));

    return () => {
      unsubs.forEach((fn) => fn());
    };
  }, [setQuickPasteOpen, setTrayOpen, refresh]);

  if (windowMode === "settings") {
    return (
      <TrayShell className="h-full overflow-hidden">
        <SettingsPanel />
      </TrayShell>
    );
  }

  if (windowMode === "tray") {
    return (
      <TrayShell>
        <TrayPanel />
        <SyncToast />
      </TrayShell>
    );
  }

  if (windowMode === "quick-paste") {
    return (
      <TrayShell>
        <QuickPasteLauncher />
        <SyncToast />
      </TrayShell>
    );
  }

  return (
    <TrayShell>
      <QuickPasteLauncher />
      <TrayPanel />
      <SyncToast />
    </TrayShell>
  );
}
