import { useCallback, useEffect, useState } from "react";
import { Plus, Play, Square, Download, ListChecks, Trash2 } from "lucide-react";
import { ask, save } from "@tauri-apps/plugin-dialog";
import { ipc } from "@/services/ipc";
import type { BatchJob, JobItem } from "@/types";
import { Button } from "@/components/ui/Button";
import { Badge } from "@/components/ui/Badge";
import { EmptyState } from "@/components/ui/EmptyState";
import { NewJobDialog } from "@/components/batch/NewJobDialog";

type Tone = "accent" | "blue" | "green" | "mauve" | "muted";

const JOB_TONE: Record<string, Tone> = {
  pending: "muted",
  running: "blue",
  done: "green",
  cancelled: "muted",
};

const JOB_LABEL: Record<string, string> = {
  pending: "待运行",
  running: "运行中",
  done: "已完成",
  cancelled: "已取消",
};

export function BatchView() {
  const [jobs, setJobs] = useState<BatchJob[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [items, setItems] = useState<JobItem[]>([]);
  const [creating, setCreating] = useState(false);

  const reloadJobs = useCallback(async () => {
    try {
      setJobs(await ipc.listBatchJobs());
    } catch (e) {
      console.error("加载批量任务失败:", e);
    }
  }, []);

  const reloadItems = useCallback(async (jobId: string) => {
    try {
      setItems(await ipc.listJobItems(jobId));
    } catch (e) {
      console.error("加载任务单元失败:", e);
    }
  }, []);

  useEffect(() => {
    reloadJobs();
  }, [reloadJobs]);

  useEffect(() => {
    if (selectedId) reloadItems(selectedId);
    else setItems([]);
  }, [selectedId, reloadItems]);

  // 轮询:运行中的任务持续刷新进度。
  useEffect(() => {
    const timer = setInterval(async () => {
      const fresh = await ipc.listBatchJobs().catch(() => null);
      if (!fresh) return;
      setJobs(fresh);
      if (selectedId && fresh.some((j) => j.id === selectedId)) {
        const running = fresh.some(
          (j) => j.id === selectedId && j.status === "running",
        );
        if (running) reloadItems(selectedId);
      }
    }, 1500);
    return () => clearInterval(timer);
  }, [selectedId, reloadItems]);

  const selected = jobs.find((j) => j.id === selectedId) ?? null;

  const onRun = async (job: BatchJob) => {
    await ipc.runBatchJob(job.id);
    reloadJobs();
  };
  const onCancel = async (job: BatchJob) => {
    await ipc.cancelBatchJob(job.id);
    reloadJobs();
  };
  const onExport = async (job: BatchJob) => {
    const path = await save({
      defaultPath: `${job.name}-结果.json`,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (path) await ipc.exportJobResults(job.id, path);
  };
  const onDelete = async (job: BatchJob) => {
    const ok = await ask(
      `删除批量任务「${job.name}」?其下所有单元会一并删除。`,
      { title: "确认删除", kind: "warning" },
    );
    if (!ok) return;
    await ipc.deleteBatchJob(job.id);
    if (selectedId === job.id) setSelectedId(null);
    reloadJobs();
  };

  return (
    <div className="flex h-full">
      {/* 任务列表 */}
      <div className="flex w-72 shrink-0 flex-col border-r border-mocha-rim/40">
        <div className="flex shrink-0 items-center justify-between border-b border-mocha-rim/40 px-3 py-2.5">
          <span className="text-xs font-medium text-mocha-subtext">
            批量任务
          </span>
          <button
            onClick={() => setCreating(true)}
            className="flex items-center gap-1 rounded-mocha bg-mocha-surface px-2 py-1 text-xs text-mocha-subtext transition-colors hover:bg-mocha-overlay hover:text-mocha-text"
          >
            <Plus size={13} /> 新建
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-auto p-2">
          {jobs.length === 0 ? (
            <p className="px-2 py-6 text-center text-xs text-mocha-muted">
              还没有批量任务
            </p>
          ) : (
            jobs.map((job) => (
              <button
                key={job.id}
                onClick={() => setSelectedId(job.id)}
                className={`mb-1 block w-full rounded-mocha px-3 py-2.5 text-left transition-colors ${
                  job.id === selectedId
                    ? "bg-mocha-overlay"
                    : "hover:bg-mocha-surface"
                }`}
              >
                <div className="flex items-center gap-2">
                  <span className="min-w-0 flex-1 truncate text-sm text-mocha-text">
                    {job.name}
                  </span>
                  <Badge
                    text={JOB_LABEL[job.status] ?? job.status}
                    tone={JOB_TONE[job.status] ?? "muted"}
                  />
                </div>
                <Progress done={job.doneItems} total={job.totalItems} />
              </button>
            ))
          )}
        </div>
      </div>

      {/* 任务详情 */}
      <div className="flex min-w-0 flex-1 flex-col">
        {selected ? (
          <>
            <div className="flex shrink-0 items-center gap-3 border-b border-mocha-rim/40 px-4 py-3">
              <div className="min-w-0 flex-1">
                <h2 className="truncate text-sm font-semibold text-mocha-text">
                  {selected.name}
                </h2>
                <p className="mt-0.5 text-xs text-mocha-muted">
                  {selected.doneItems} / {selected.totalItems} 已完成
                </p>
              </div>
              {selected.status === "running" ? (
                <Button variant="danger" onClick={() => onCancel(selected)}>
                  <Square size={14} /> 取消
                </Button>
              ) : (
                selected.doneItems < selected.totalItems && (
                  <Button onClick={() => onRun(selected)}>
                    <Play size={14} />
                    {selected.doneItems > 0 ? "继续" : "运行"}
                  </Button>
                )
              )}
              <Button variant="ghost" onClick={() => onExport(selected)}>
                <Download size={14} /> 导出
              </Button>
              {selected.status !== "running" && (
                <Button variant="ghost" onClick={() => onDelete(selected)}>
                  <Trash2 size={14} /> 删除
                </Button>
              )}
            </div>

            <div className="min-h-0 flex-1 overflow-auto p-4">
              <ul className="flex flex-col gap-2">
                {items.map((item) => (
                  <ItemRow key={item.id} item={item} />
                ))}
              </ul>
            </div>
          </>
        ) : (
          <EmptyState
            icon={<ListChecks size={36} strokeWidth={1.4} />}
            title="选择或新建批量任务"
            desc="选择文件列表与方案,批量生成结果并导出,支持中断恢复。"
          />
        )}
      </div>

      {creating && (
        <NewJobDialog
          onClose={() => setCreating(false)}
          onCreated={(job) => {
            reloadJobs();
            setSelectedId(job.id);
          }}
        />
      )}
    </div>
  );
}

function Progress({ done, total }: { done: number; total: number }) {
  const pct = total > 0 ? Math.round((done / total) * 100) : 0;
  return (
    <div className="mt-1.5 h-1 w-full overflow-hidden rounded-full bg-mocha-overlay">
      <div
        className="h-full rounded-full bg-mocha-accent transition-[width] duration-300"
        style={{ width: `${pct}%` }}
      />
    </div>
  );
}

const ITEM_TONE: Record<string, string> = {
  pending: "bg-mocha-rim",
  processing: "bg-mocha-blue animate-pulse",
  done: "bg-mocha-green",
  failed: "bg-mocha-rose",
  cancelled: "bg-mocha-rim",
};

function ItemRow({ item }: { item: JobItem }) {
  const name = item.filePath.split("/").pop() ?? item.filePath;
  return (
    <li className="pane-inset px-4 py-3">
      <div className="flex items-center gap-2.5">
        <span
          className={`h-2 w-2 shrink-0 rounded-full ${ITEM_TONE[item.status] ?? "bg-mocha-rim"}`}
        />
        <span className="min-w-0 flex-1 truncate text-xs font-medium text-mocha-text">
          {name}
        </span>
      </div>
      {item.status === "failed" && item.error && (
        <p className="mt-1.5 line-clamp-2 pl-[18px] text-xs leading-relaxed text-mocha-rose">
          {item.error}
        </p>
      )}
      {item.resultText && (
        <p className="mt-1.5 line-clamp-3 pl-[18px] text-xs leading-relaxed text-mocha-subtext">
          {item.resultText}
        </p>
      )}
    </li>
  );
}
