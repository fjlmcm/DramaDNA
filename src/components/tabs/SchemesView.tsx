import { useState } from "react";
import { Plus, Pencil, Trash2 } from "lucide-react";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { useAppStore } from "@/store/useAppStore";
import type { Scheme } from "@/types";
import { Button } from "@/components/ui/Button";
import { Badge } from "@/components/ui/Badge";
import { EmptyState } from "@/components/ui/EmptyState";
import { SchemeForm } from "@/components/schemes/SchemeForm";

export function SchemesView() {
  const schemes = useAppStore((s) => s.schemes);
  const models = useAppStore((s) => s.models);
  const removeScheme = useAppStore((s) => s.removeScheme);
  const [editing, setEditing] = useState<Scheme | null>(null);
  const [creating, setCreating] = useState(false);

  const hasModels = models.some((m) => m.enabled);
  const modelLabel = (id: string) =>
    models.find((m) => m.id === id)?.displayName ?? "未知模型";

  const onDelete = async (s: Scheme) => {
    const ok = await ask(`删除方案「${s.name}」?`, {
      title: "确认删除",
      kind: "warning",
    });
    if (!ok) return;
    try {
      await removeScheme(s.id);
    } catch (e) {
      await message(String(e), { title: "删除失败", kind: "error" });
    }
  };

  return (
    <div className="h-full overflow-auto p-6">
      <div className="mb-4 flex items-start justify-between">
        <div>
          <h2 className="text-sm font-semibold text-mocha-text">方案管理</h2>
          <p className="mt-0.5 text-xs text-mocha-muted">
            方案 = 单个模型 + 提示词,可在视频理解与批量处理中复用。
          </p>
        </div>
        <Button onClick={() => setCreating(true)} disabled={!hasModels}>
          <Plus size={15} /> 新建方案
        </Button>
      </div>

      {!hasModels ? (
        <EmptyState
          title="请先添加模型"
          desc="方案需绑定一个模型,先到设置中添加供应商与模型。"
        />
      ) : schemes.length === 0 ? (
        <EmptyState
          title="还没有方案"
          desc="新建方案,或在视频理解页把模型 + 提示词保存为方案。"
        />
      ) : (
        <ul className="flex flex-col gap-2">
          {schemes.map((s) => (
            <li key={s.id} className="pane-inset flex items-start gap-3 px-4 py-3">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="truncate text-sm font-medium text-mocha-text">
                    {s.name}
                  </span>
                  <Badge text={modelLabel(s.modelId)} tone="blue" />
                </div>
                <p className="mt-1 line-clamp-2 text-xs leading-relaxed text-mocha-muted">
                  {s.prompt}
                </p>
              </div>
              <button
                onClick={() => setEditing(s)}
                aria-label="编辑"
                className="mt-0.5 text-mocha-muted transition-colors hover:text-mocha-text"
              >
                <Pencil size={15} />
              </button>
              <button
                onClick={() => onDelete(s)}
                aria-label="删除"
                className="mt-0.5 text-mocha-muted transition-colors hover:text-mocha-rose"
              >
                <Trash2 size={15} />
              </button>
            </li>
          ))}
        </ul>
      )}

      {creating && <SchemeForm onClose={() => setCreating(false)} />}
      {editing && (
        <SchemeForm scheme={editing} onClose={() => setEditing(null)} />
      )}
    </div>
  );
}
