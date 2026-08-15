import { open } from "@tauri-apps/plugin-dialog";
import { Play, FileVideo, Square } from "lucide-react";
import { useAppStore } from "@/store/useAppStore";
import { ipc } from "@/services/ipc";
import type { Scheme } from "@/types";
import { Button } from "@/components/ui/Button";
import { EmptyState } from "@/components/ui/EmptyState";
import { ResultColumn } from "@/components/understand/ResultColumn";

export function UnderstandView() {
  const models = useAppStore((s) => s.models);
  const schemes = useAppStore((s) => s.schemes);
  // 会话状态提升到 store —— 切换 Tab 后仍保留。
  const { videoPath, prompt, modelIds, results, running } = useAppStore(
    (s) => s.understand,
  );
  const setUnderstand = useAppStore((s) => s.setUnderstand);

  const enabledModels = models.filter((m) => m.enabled);
  const modelLabel = (id: string) =>
    models.find((m) => m.id === id)?.displayName ?? "未知模型";

  const toggleModel = (id: string) => {
    setUnderstand((u) => ({
      modelIds: u.modelIds.includes(id)
        ? u.modelIds.filter((x) => x !== id)
        : [...u.modelIds, id],
    }));
  };

  const loadScheme = (scheme: Scheme) => {
    setUnderstand((u) => ({
      prompt: scheme.prompt,
      modelIds: u.modelIds.includes(scheme.modelId)
        ? u.modelIds
        : [...u.modelIds, scheme.modelId],
    }));
  };

  const pickVideo = async () => {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "视频",
          extensions: ["mp4", "mov", "mkv", "avi", "webm", "m4v"],
        },
      ],
    });
    if (typeof selected === "string") setUnderstand({ videoPath: selected });
  };

  const canRun = !!videoPath && modelIds.length > 0 && !running;

  const run = async () => {
    if (!videoPath || modelIds.length === 0) return;
    // 为每个模型生成独立 run_id,后端按此 id 注册 cancel token。
    const runs = modelIds.map((id) => ({ id, runId: crypto.randomUUID() }));
    setUnderstand({
      running: true,
      results: Object.fromEntries(
        runs.map(({ id, runId }) => [
          id,
          { text: "", status: "running" as const, runId },
        ]),
      ),
    });

    await Promise.allSettled(
      runs.map(({ id, runId }) =>
        ipc
          .understandVideoStream(id, prompt, videoPath, runId, (e) => {
            setUnderstand((u) => {
              const cur = u.results[id] ?? {
                text: "",
                status: "running" as const,
              };
              const next =
                e.type === "delta"
                  ? { ...cur, text: cur.text + e.text }
                  : e.type === "done"
                    ? { ...cur, status: "done" as const, runId: undefined }
                    : {
                        ...cur,
                        status: "error" as const,
                        error: e.message,
                        runId: undefined,
                      };
              return { results: { ...u.results, [id]: next } };
            });
          })
          .catch((err) => {
            setUnderstand((u) => ({
              results: {
                ...u.results,
                [id]: {
                  text: u.results[id]?.text ?? "",
                  status: "error" as const,
                  error: String(err),
                  runId: undefined,
                },
              },
            }));
          }),
      ),
    );
    setUnderstand({ running: false });
  };

  const cancel = () => {
    // 给所有仍在 running 的列发取消信号;UI 状态由 stream 返回 Err 后自然变 error。
    for (const r of Object.values(results)) {
      if (r.status === "running" && r.runId) {
        void ipc.cancelUnderstandVideo(r.runId).catch(() => {});
      }
    }
  };

  if (enabledModels.length === 0) {
    return (
      <EmptyState
        title="还没有可用模型"
        desc="请先到设置中添加供应商与模型,并确保模型已启用。"
      />
    );
  }

  return (
    <div className="flex h-full flex-col">
      {/* 视频选择 + 运行 */}
      <div className="flex shrink-0 items-center gap-3 border-b border-mocha-rim/40 px-4 py-3">
        <Button variant="ghost" onClick={pickVideo}>
          <FileVideo size={15} /> 选择视频
        </Button>
        <span className="min-w-0 flex-1 truncate text-xs text-mocha-muted">
          {videoPath ? videoPath.split("/").pop() : "未选择视频"}
        </span>
        {running ? (
          <Button variant="danger" onClick={cancel}>
            <Square size={15} /> 取消
          </Button>
        ) : (
          <Button onClick={run} disabled={!canRun}>
            <Play size={15} /> 一键运行
          </Button>
        )}
      </div>

      {/* 提示词 + 模型选择 */}
      <div className="shrink-0 border-b border-mocha-rim/40 px-4 py-3">
        <div className="mb-2 flex items-center gap-2">
          <span className="text-xs font-medium text-mocha-subtext">
            提示词(多模型共用)
          </span>
          <div className="flex-1" />
          {schemes.length > 0 && (
            <select
              value=""
              onChange={(e) => {
                const s = schemes.find((x) => x.id === e.target.value);
                if (s) loadScheme(s);
              }}
              className="rounded-mocha border border-mocha-rim/60 bg-mocha-crust/60 px-2 py-1 text-xs text-mocha-subtext outline-none"
            >
              <option value="">从方案加载…</option>
              {schemes.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </select>
          )}
        </div>

        <textarea
          value={prompt}
          onChange={(e) => setUnderstand({ prompt: e.target.value })}
          rows={3}
          placeholder="对视频提出的理解要求…"
          className="w-full resize-y rounded-mocha border border-mocha-rim/60 bg-mocha-crust/60 px-3 py-2 text-sm leading-relaxed text-mocha-text outline-none placeholder:text-mocha-muted focus:border-mocha-accent/70"
        />

        <div className="mt-2.5">
          <span className="text-xs font-medium text-mocha-subtext">
            参与对比的模型({modelIds.length})
          </span>
          <div className="mt-1.5 flex flex-wrap gap-1.5">
            {enabledModels.map((m) => {
              const on = modelIds.includes(m.id);
              return (
                <button
                  key={m.id}
                  onClick={() => toggleModel(m.id)}
                  className={`rounded-mocha px-2.5 py-1 text-xs font-medium transition-colors ${
                    on
                      ? "bg-mocha-accent text-mocha-crust"
                      : "bg-mocha-surface text-mocha-subtext hover:bg-mocha-overlay hover:text-mocha-text"
                  }`}
                >
                  {m.displayName}
                </button>
              );
            })}
          </div>
        </div>
      </div>

      {/* 多列结果 */}
      <div className="min-h-0 flex-1 overflow-auto p-4">
        {modelIds.length === 0 ? (
          <EmptyState title="选择模型后,多列对比结果将在这里并排显示" />
        ) : (
          <div className="flex h-full gap-3">
            {modelIds.map((id) => (
              <ResultColumn
                key={id}
                modelLabel={modelLabel(id)}
                result={results[id]}
              />
            ))}
          </div>
        )}
      </div>

    </div>
  );
}
