import type { TabKey } from "@/types";
import { useAppStore } from "@/store/useAppStore";

const TABS: { key: TabKey; label: string }[] = [
  { key: "dramas", label: "剧目拆解" },
  { key: "specs", label: "资产配置" },
  { key: "outputs", label: "产出浏览" },
  { key: "understand", label: "模型测试" },
  { key: "logs", label: "执行日志" },
];

export function TabNav() {
  const activeTab = useAppStore((s) => s.activeTab);
  const setTab = useAppStore((s) => s.setTab);

  return (
    <nav className="flex items-center gap-0.5">
      {TABS.map((t) => {
        const active = t.key === activeTab;
        return (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            className={`relative rounded-mocha px-3 py-1.5 text-[13px] font-medium transition-colors ${
              active
                ? "text-mocha-text"
                : "text-mocha-muted hover:text-mocha-subtext"
            }`}
          >
            {t.label}
            {active && (
              <span className="absolute inset-x-2.5 -bottom-px h-0.5 rounded-full bg-mocha-accent" />
            )}
          </button>
        );
      })}
    </nav>
  );
}
