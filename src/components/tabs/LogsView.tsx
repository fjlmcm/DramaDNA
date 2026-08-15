import { useEffect, useState } from "react";
import { Eraser, RefreshCw, ScrollText } from "lucide-react";
import { dna, ipc } from "@/services/ipc";
import type { DnaTaskView, Run } from "@/types";
import { Badge } from "@/components/ui/Badge";
import { EmptyState } from "@/components/ui/EmptyState";

type Sub = "tasks" | "runs" | "debug";

const SUBS: [Sub, string][] = [
  ["tasks", "拆解任务"],
  ["runs", "模型测试"],
  ["debug", "调试日志"],
];

function formatTime(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString("zh-CN");
}

function formatDuration(ms: number | null): string {
  if (ms == null) return "";
  return ms < 1000 ? `${ms}ms` : `${(ms / 1000).toFixed(1)}s`;
}

function statusTone(status: string): string {
  if (status === "done") return "bg-mocha-green";
  if (status === "failed") return "bg-mocha-rose";
  if (status === "processing") return "bg-mocha-blue animate-pulse";
  return "bg-mocha-rim";
}

export function LogsView() {
  const [sub, setSub] = useState<Sub>("tasks");
  const [tasks, setTasks] = useState<DnaTaskView[]>([]);
  const [runs, setRuns] = useState<Run[]>([]);
  const [debugLog, setDebugLog] = useState("");
  const [logPath, setLogPath] = useState("");
  const [expanded, setExpanded] = useState<string | null>(null);

  const reload = async (which: Sub = sub) => {
    try {
      if (which === "tasks") setTasks(await dna.listRecentTasks(300));
      else if (which === "runs") setRuns(await ipc.listRuns());
      else {
        setDebugLog(await ipc.readDebugLog());
        setLogPath(await ipc.debugLogPath());
      }
    } catch (e) {
      console.error("加载日志失败:", e);
    }
  };

  useEffect(() => {
    reload(sub);
  }, [sub]);

  const clearCurrent = async () => {
    try {
      if (sub === "runs") await ipc.clearRuns();
      else if (sub === "debug") await ipc.clearDebugLog();
      await reload(sub);
    } catch (e) {
      console.error("清空失败:", e);
    }
  };

  const unitLabel = (t: DnaTaskView) =>
    t.epNo != null ? `第${t.epNo}集` : t.segmentNo != null && t.segmentNo > 1 ? `#${t.segmentNo}` : "全剧";

  return (
    <div className="flex h-full flex-col p-6">
      <div className="mb-4 flex items-center gap-2">
        <div className="flex gap-0.5 rounded-mocha bg-mocha-surface p-0.5">
          {SUBS.map(([key, label]) => (
            <button
              key={key}
              onClick={() => setSub(key)}
              className={`rounded-[7px] px-3 py-1 text-xs font-medium transition-colors ${
                sub === key
                  ? "bg-mocha-overlay text-mocha-text"
                  : "text-mocha-muted hover:text-mocha-subtext"
              }`}
            >
              {label}
            </button>
          ))}
        </div>
        <div className="flex-1" />
        {sub !== "tasks" && (
          <button
            onClick={clearCurrent}
            className="flex items-center gap-1 rounded-mocha bg-mocha-surface px-2.5 py-1.5 text-xs text-mocha-subtext transition-colors hover:bg-mocha-rose/15 hover:text-mocha-rose"
          >
            <Eraser size={13} /> 清空
          </button>
        )}
        <button
          onClick={() => reload(sub)}
          className="flex items-center gap-1 rounded-mocha bg-mocha-surface px-2.5 py-1.5 text-xs text-mocha-subtext transition-colors hover:bg-mocha-overlay hover:text-mocha-text"
        >
          <RefreshCw size={13} /> 刷新
        </button>
      </div>

      {sub === "tasks" ? (
        tasks.length === 0 ? (
          <EmptyState
            icon={<ScrollText size={36} strokeWidth={1.4} />}
            title="还没有拆解任务记录"
            desc="在剧目拆解页启动管线后,任务执行记录会显示在这里。"
          />
        ) : (
          <ul className="flex min-h-0 flex-1 flex-col gap-1.5 overflow-auto">
            {tasks.map((t) => {
              const open = expanded === t.id;
              return (
                <li key={t.id} className="pane-inset px-4 py-2">
                  <button
                    onClick={() => setExpanded(open ? null : t.id)}
                    className="flex w-full items-center gap-2.5 text-left"
                  >
                    <span className={`h-2 w-2 shrink-0 rounded-full ${statusTone(t.status)}`} />
                    <span className="w-40 shrink-0 truncate text-xs text-mocha-muted">
                      {t.dramaName}
                    </span>
                    <span className="min-w-0 flex-1 truncate text-xs font-medium text-mocha-text">
                      {t.specName} · {unitLabel(t)}
                    </span>
                    <span className="shrink-0 text-[11px] tabular-nums text-mocha-muted">
                      {formatDuration(t.durationMs)}
                    </span>
                    <span className="shrink-0 text-[11px] text-mocha-muted">
                      {formatTime(t.updatedAt)}
                    </span>
                  </button>
                  {open && (
                    <div className="mt-2 border-t border-mocha-rim/30 pt-2 pl-[18px]">
                      {t.status === "failed" ? (
                        <p className="whitespace-pre-wrap break-all text-xs leading-relaxed text-mocha-rose">
                          {t.error ?? "未知错误"}
                        </p>
                      ) : (
                        <p className="text-[11px] text-mocha-muted">
                          状态 {t.status} · 产出 {t.resultChars} 字符
                        </p>
                      )}
                    </div>
                  )}
                </li>
              );
            })}
          </ul>
        )
      ) : sub === "runs" ? (
        runs.length === 0 ? (
          <EmptyState
            icon={<ScrollText size={36} strokeWidth={1.4} />}
            title="还没有测试记录"
            desc="在模型测试页运行后,记录会显示在这里。"
          />
        ) : (
          <ul className="flex min-h-0 flex-1 flex-col gap-2 overflow-auto">
            {runs.map((run) => {
              const open = expanded === run.id;
              const fileName = run.filePath.split("/").pop() ?? run.filePath;
              return (
                <li key={run.id} className="pane-inset px-4 py-3">
                  <button
                    onClick={() => setExpanded(open ? null : run.id)}
                    className="flex w-full items-center gap-2.5 text-left"
                  >
                    <span
                      className={`h-2 w-2 shrink-0 rounded-full ${
                        run.status === "done" ? "bg-mocha-green" : "bg-mocha-rose"
                      }`}
                    />
                    <span className="min-w-0 flex-1 truncate text-xs font-medium text-mocha-text">
                      {fileName}
                    </span>
                    <Badge text={run.modelLabel} tone="blue" />
                    <span className="shrink-0 text-[11px] text-mocha-muted">
                      {formatDuration(run.durationMs)}
                    </span>
                    <span className="shrink-0 text-[11px] text-mocha-muted">
                      {formatTime(run.createdAt)}
                    </span>
                  </button>
                  {open && (
                    <div className="mt-2.5 border-t border-mocha-rim/30 pt-2.5 pl-[18px]">
                      <p className="mb-1.5 text-[11px] text-mocha-muted">提示词:{run.prompt}</p>
                      {run.status === "failed" ? (
                        <p className="whitespace-pre-wrap text-xs leading-relaxed text-mocha-rose">
                          {run.error ?? "未知错误"}
                        </p>
                      ) : (
                        <p className="whitespace-pre-wrap text-[13px] leading-relaxed text-mocha-subtext">
                          {run.resultText ?? "(无内容)"}
                        </p>
                      )}
                    </div>
                  )}
                </li>
              );
            })}
          </ul>
        )
      ) : (
        <div className="flex min-h-0 flex-1 flex-col gap-1.5">
          {logPath && (
            <p className="shrink-0 font-mono text-[10px] text-mocha-muted">{logPath}</p>
          )}
          <pre className="min-h-0 flex-1 overflow-auto rounded-mocha border border-mocha-rim/40 bg-mocha-crust/60 p-3 font-mono text-[11px] leading-relaxed text-mocha-subtext">
            {debugLog || "(暂无日志 —— 应用运行后产生)"}
          </pre>
        </div>
      )}
    </div>
  );
}
