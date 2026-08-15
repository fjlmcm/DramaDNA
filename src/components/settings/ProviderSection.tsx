import { useState } from "react";
import { Plus, Pencil, Trash2 } from "lucide-react";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { useAppStore } from "@/store/useAppStore";
import type { Provider } from "@/types";
import { PROVIDER_KINDS } from "@/types";
import { Button } from "@/components/ui/Button";
import { Badge } from "@/components/ui/Badge";
import { EmptyState } from "@/components/ui/EmptyState";
import { ProviderForm } from "./ProviderForm";

const KIND_LABEL: Record<string, string> = Object.fromEntries(
  PROVIDER_KINDS.map((k) => [k.value, k.label]),
);

export function ProviderSection() {
  const providers = useAppStore((s) => s.providers);
  const models = useAppStore((s) => s.models);
  const removeProvider = useAppStore((s) => s.removeProvider);
  const [editing, setEditing] = useState<Provider | null>(null);
  const [creating, setCreating] = useState(false);

  const modelCount = (pid: string) =>
    models.filter((m) => m.providerId === pid).length;

  const onDelete = async (p: Provider) => {
    const ok = await ask(
      `删除供应商「${p.name}」?其下所有模型与方案会一并删除(批量任务引用的方案需先手动清理)。`,
      { title: "确认删除", kind: "warning" },
    );
    if (!ok) return;
    try {
      await removeProvider(p.id);
    } catch (e) {
      await message(String(e), { title: "删除失败", kind: "error" });
    }
  };

  return (
    <div>
      <div className="mb-4 flex items-start justify-between">
        <div>
          <h2 className="text-sm font-semibold text-mocha-text">模型供应商</h2>
          <p className="mt-0.5 text-xs text-mocha-muted">
            配置 API 接入点与密钥,支持中转供应商。
          </p>
        </div>
        <Button onClick={() => setCreating(true)}>
          <Plus size={15} /> 新增供应商
        </Button>
      </div>

      {providers.length === 0 ? (
        <EmptyState
          title="还没有供应商"
          desc="点击右上角新增,接入火山引擎、阿里百炼或 OpenAI 兼容中转站。"
        />
      ) : (
        <ul className="flex flex-col gap-2">
          {providers.map((p) => (
            <li
              key={p.id}
              className="pane-inset flex items-center gap-3 px-4 py-3"
            >
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="truncate text-sm font-medium text-mocha-text">
                    {p.name}
                  </span>
                  <Badge text={KIND_LABEL[p.kind] ?? p.kind} tone="accent" />
                </div>
                <p className="mt-0.5 truncate text-xs text-mocha-muted">
                  {p.baseUrl}
                </p>
              </div>
              <span className="shrink-0 text-xs text-mocha-muted">
                {modelCount(p.id)} 个模型
              </span>
              <button
                onClick={() => setEditing(p)}
                aria-label="编辑"
                className="text-mocha-muted transition-colors hover:text-mocha-text"
              >
                <Pencil size={15} />
              </button>
              <button
                onClick={() => onDelete(p)}
                aria-label="删除"
                className="text-mocha-muted transition-colors hover:text-mocha-rose"
              >
                <Trash2 size={15} />
              </button>
            </li>
          ))}
        </ul>
      )}

      {creating && <ProviderForm onClose={() => setCreating(false)} />}
      {editing && (
        <ProviderForm provider={editing} onClose={() => setEditing(null)} />
      )}
    </div>
  );
}
