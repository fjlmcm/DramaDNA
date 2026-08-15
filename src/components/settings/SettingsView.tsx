import { useState } from "react";
import { ArrowLeft } from "lucide-react";
import { useAppStore } from "@/store/useAppStore";
import { ProviderSection } from "./ProviderSection";
import { ModelSection } from "./ModelSection";
import { PipelineSettings } from "./PipelineSettings";
import { CacheSection } from "./CacheSection";

type Section = "providers" | "models" | "pipeline" | "cache";

const SECTIONS: [Section, string][] = [
  ["providers", "模型供应商"],
  ["models", "模型"],
  ["pipeline", "拆解管线"],
  ["cache", "缓存"],
];

export function SettingsView() {
  const closeSettings = useAppStore((s) => s.closeSettings);
  const [section, setSection] = useState<Section>("providers");

  return (
    <div className="absolute inset-x-0 bottom-0 top-11 z-40 flex flex-col bg-mocha-mantle">
      <div className="flex shrink-0 items-center gap-3 border-b border-mocha-rim/40 px-3 py-2.5">
        <button
          onClick={closeSettings}
          className="flex items-center gap-1 rounded-mocha px-2 py-1 text-sm text-mocha-subtext transition-colors hover:bg-mocha-overlay hover:text-mocha-text"
        >
          <ArrowLeft size={15} /> 返回
        </button>
        <h1 className="text-sm font-semibold text-mocha-text">设置</h1>
      </div>

      <div className="flex min-h-0 flex-1">
        <aside className="w-44 shrink-0 border-r border-mocha-rim/40 p-2">
          {SECTIONS.map(([key, label]) => (
            <button
              key={key}
              onClick={() => setSection(key)}
              className={`mb-0.5 block w-full rounded-mocha px-3 py-2 text-left text-[13px] transition-colors ${
                section === key
                  ? "bg-mocha-overlay text-mocha-text"
                  : "text-mocha-muted hover:text-mocha-subtext"
              }`}
            >
              {label}
            </button>
          ))}
        </aside>

        <div className="min-h-0 flex-1 overflow-auto p-6">
          {section === "providers" && <ProviderSection />}
          {section === "models" && <ModelSection />}
          {section === "pipeline" && <PipelineSettings />}
          {section === "cache" && <CacheSection />}
        </div>
      </div>
    </div>
  );
}
