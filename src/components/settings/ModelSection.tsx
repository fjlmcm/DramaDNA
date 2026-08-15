import { useState } from "react";
import { Plus, Pencil, Trash2 } from "lucide-react";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { useAppStore } from "@/store/useAppStore";
import type { Model } from "@/types";
import { VIDEO_INPUT_METHODS } from "@/types";
import { Button } from "@/components/ui/Button";
import { Badge } from "@/components/ui/Badge";
import { EmptyState } from "@/components/ui/EmptyState";
import { ModelForm } from "./ModelForm";

const METHOD_LABEL: Record<string, string> = Object.fromEntries(
  VIDEO_INPUT_METHODS.map((m) => [m.value, m.label]),
);

export function ModelSection() {
  const models = useAppStore((s) => s.models);
  const providers = useAppStore((s) => s.providers);
  const removeModel = useAppStore((s) => s.removeModel);
  const [editing, setEditing] = useState<Model | null>(null);
  const [creating, setCreating] = useState(false);

  const providerName = (id: string) =>
    providers.find((p) => p.id === id)?.name ?? "未知供应商";

  const onDelete = async (m: Model) => {
    const ok = await ask(
      `删除模型「${m.displayName}」?其下的方案会一并删除(批量任务引用的方案需先手动清理)。`,
      { title: "确认删除", kind: "warning" },
    );
    if (!ok) return;
    try {
      await removeModel(m.id);
    } catch (e) {
      await message(String(e), { title: "删除失败", kind: "error" });
    }
  };

  return (
    <div>
      <div className="mb-4 flex items-start justify-between">
        <div>
          <h2 className="text-sm font-semibold text-mocha-text">模型</h2>
          <p className="mt-0.5 text-xs text-mocha-muted">
            每个模型归属一个供应商,供方案引用。
          </p>
        </div>
        <Button onClick={() => setCreating(true)} disabled={providers.length === 0}>
          <Plus size={15} /> 新增模型
        </Button>
      </div>

      {providers.length === 0 ? (
        <EmptyState
          title="请先添加供应商"
          desc="模型必须归属于某个供应商,先到「模型供应商」页面添加。"
        />
      ) : models.length === 0 ? (
        <EmptyState title="还没有模型" desc="点击右上角新增第一个模型。" />
      ) : (
        <ul className="flex flex-col gap-2">
          {models.map((m) => (
            <li
              key={m.id}
              className="pane-inset flex items-center gap-3 px-4 py-3"
            >
              <span
                className={`h-2 w-2 shrink-0 rounded-full ${
                  m.enabled ? "bg-mocha-green" : "bg-mocha-rim"
                }`}
                title={m.enabled ? "已启用" : "已停用"}
              />
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="truncate text-sm font-medium text-mocha-text">
                    {m.displayName}
                  </span>
                  <Badge text={providerName(m.providerId)} tone="blue" />
                </div>
                <p className="mt-0.5 truncate font-mono text-xs text-mocha-muted">
                  {m.modelId}
                </p>
              </div>
              <span className="shrink-0 text-xs text-mocha-muted">
                {METHOD_LABEL[m.videoInputMethod] ?? m.videoInputMethod}
              </span>
              <button
                onClick={() => setEditing(m)}
                aria-label="编辑"
                className="text-mocha-muted transition-colors hover:text-mocha-text"
              >
                <Pencil size={15} />
              </button>
              <button
                onClick={() => onDelete(m)}
                aria-label="删除"
                className="text-mocha-muted transition-colors hover:text-mocha-rose"
              >
                <Trash2 size={15} />
              </button>
            </li>
          ))}
        </ul>
      )}

      {creating && <ModelForm onClose={() => setCreating(false)} />}
      {editing && <ModelForm model={editing} onClose={() => setEditing(null)} />}
    </div>
  );
}
