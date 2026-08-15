import { Settings } from "lucide-react";
import { WindowControls } from "@/components/WindowControls";
import { useAppStore } from "@/store/useAppStore";
import { TabNav } from "./TabNav";

export function TitleBar() {
  const openSettings = useAppStore((s) => s.openSettings);

  return (
    <header
      data-tauri-drag-region
      className="flex h-11 shrink-0 select-none items-center border-b border-mocha-rim/40"
    >
      <div data-tauri-drag-region className="flex items-center pl-4 pr-4">
        <span className="text-sm font-semibold tracking-wide text-mocha-accent">
          DramaDNA
        </span>
      </div>

      <TabNav />

      <div data-tauri-drag-region className="h-full flex-1" />

      <button
        onClick={openSettings}
        title="设置"
        aria-label="设置"
        className="mr-1 grid h-8 w-8 place-items-center rounded-mocha text-mocha-subtext transition-colors hover:bg-mocha-overlay hover:text-mocha-text"
      >
        <Settings size={16} />
      </button>

      <WindowControls />
    </header>
  );
}
