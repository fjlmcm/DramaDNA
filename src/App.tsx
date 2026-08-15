import { useEffect } from "react";
import { TitleBar } from "@/components/chrome/TitleBar";
import { DramasView } from "@/components/tabs/DramasView";
import { SpecsView } from "@/components/tabs/SpecsView";
import { OutputsView } from "@/components/tabs/OutputsView";
import { UnderstandView } from "@/components/tabs/UnderstandView";
import { LogsView } from "@/components/tabs/LogsView";
import { SettingsView } from "@/components/settings/SettingsView";
import { useAppStore } from "@/store/useAppStore";

export default function App() {
  const activeTab = useAppStore((s) => s.activeTab);
  const settingsOpen = useAppStore((s) => s.settingsOpen);
  const loadAll = useAppStore((s) => s.loadAll);

  useEffect(() => {
    loadAll().catch((e) => console.error("初始数据加载失败:", e));
  }, [loadAll]);

  return (
    <div className="relative flex h-screen flex-col bg-mocha-mantle text-mocha-text">
      <TitleBar />
      <main className="min-h-0 flex-1 overflow-auto">
        {activeTab === "dramas" && <DramasView />}
        {activeTab === "specs" && <SpecsView />}
        {activeTab === "outputs" && <OutputsView />}
        {activeTab === "understand" && <UnderstandView />}
        {activeTab === "logs" && <LogsView />}
      </main>
      {settingsOpen && <SettingsView />}
    </div>
  );
}
