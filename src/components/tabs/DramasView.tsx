import { useCallback, useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Clapperboard, FolderPlus, Play, RotateCcw, Square, Trash2 } from "lucide-react";
import type { AssetSpec, DnaTask, Drama, Episode, SpecStage } from "@/types";
import { STAGE_LABELS, STAGE_ORDER } from "@/types";
import { dna } from "@/services/ipc";
import { Button } from "@/components/ui/Button";
import { Badge } from "@/components/ui/Badge";
import { EmptyState } from "@/components/ui/EmptyState";
import { Modal } from "@/components/ui/Modal";

const POLL_MS = 2500;

/** 单资产在当前剧的任务汇总。 */
interface SpecProgress {
  spec: AssetSpec;
  total: number;
  done: number;
  failed: number;
  processing: number;
}

export function DramasView() {
  const [dramas, setDramas] = useState<Drama[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [episodes, setEpisodes] = useState<Episode[]>([]);
  const [tasks, setTasks] = useState<DnaTask[]>([]);
  const [specs, setSpecs] = useState<AssetSpec[]>([]);
  const [running, setRunning] = useState(false);
  const [activity, setActivity] = useState<{ text: string; ageSecs: number } | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [rerunOpen, setRerunOpen] = useState(false);
  const [showFailures, setShowFailures] = useState(false);

  const active = dramas.find((d) => d.id === activeId) ?? null;

  const reloadDramas = useCallback(async () => {
    const list = await dna.listDramas();
    setDramas(list);
    setActiveId((cur) => cur ?? list[0]?.id ?? null);
  }, []);

  useEffect(() => {
    reloadDramas().catch((e) => setError(String(e)));
    dna.listSpecs().then(setSpecs).catch(() => {});
  }, [reloadDramas]);

  // 选中剧:加载分集 + 任务,运行中轮询。
  useEffect(() => {
    if (!activeId) return;
    let alive = true;
    const tick = async () => {
      try {
        const [eps, ts, run, act] = await Promise.all([
          dna.listEpisodes(activeId),
          dna.listTasks(activeId),
          dna.pipelineRunning(activeId),
          dna.activity().catch(() => null),
        ]);
        if (!alive) return;
        setEpisodes(eps);
        setTasks(ts);
        setRunning(run);
        setActivity(act);
      } catch (e) {
        if (alive) setError(String(e));
      }
    };
    tick();
    const timer = setInterval(tick, POLL_MS);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [activeId]);

  const progress = useMemo(() => {
    const byStage = new Map<SpecStage, SpecProgress[]>();
    for (const stage of STAGE_ORDER) byStage.set(stage, []);
    for (const spec of specs) {
      if (!spec.enabled && !tasks.some((t) => t.specId === spec.id)) continue;
      const mine = tasks.filter((t) => t.specId === spec.id);
      byStage.get(spec.stage)?.push({
        spec,
        total: mine.length,
        done: mine.filter((t) => t.status === "done").length,
        failed: mine.filter((t) => t.status === "failed").length,
        processing: mine.filter((t) => t.status === "processing").length,
      });
    }
    return byStage;
  }, [specs, tasks]);

  const failures = useMemo(
    () => tasks.filter((t) => t.status === "failed"),
    [tasks],
  );
  const totalDone = tasks.filter((t) => t.status === "done").length;
  const allDone = tasks.length > 0 && totalDone === tasks.length;

  const guard = async (fn: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const importDir = () =>
    guard(async () => {
      const dir = await open({ directory: true, title: "选择剧目目录(一部剧一个目录)" });
      if (typeof dir !== "string") return;
      const drama = await dna.importDrama(dir);
      await reloadDramas();
      setActiveId(drama.id);
    });

  const specName = (id: string) => specs.find((s) => s.id === id)?.name ?? id;
  const epNo = (t: DnaTask) => {
    const ep = episodes.find((e) => e.id === t.episodeId);
    if (ep) return `第${ep.epNo}集`;
    if (t.segmentNo === 0) return "合并";
    if (t.segmentNo != null) return `段${t.segmentNo}`;
    return "全剧";
  };

  return (
    <div className="flex h-full">
      {/* 左栏:剧列表 */}
      <aside className="flex w-64 shrink-0 flex-col border-r border-mocha-rim/40">
        <div className="flex items-center justify-between px-4 py-3">
          <span className="text-xs font-semibold uppercase tracking-wide text-mocha-muted">
            剧目
          </span>
          <Button variant="ghost" className="!px-2 !py-1 text-xs" onClick={importDir} disabled={busy}>
            <FolderPlus size={14} className="mr-1 inline" />
            导入
          </Button>
        </div>
        <div className="min-h-0 flex-1 overflow-auto px-2 pb-2">
          {dramas.map((d) => (
            <button
              key={d.id}
              onClick={() => setActiveId(d.id)}
              className={`mb-1 w-full rounded-mocha px-3 py-2 text-left transition-colors ${
                d.id === activeId
                  ? "bg-mocha-surface text-mocha-text"
                  : "text-mocha-subtext hover:bg-mocha-surface/50"
              }`}
            >
              <p className="truncate text-[13px] font-medium">{d.name}</p>
              <p className="mt-0.5 text-[11px] text-mocha-muted">
                {d.episodeCount} 集 · {(d.totalDurationSec / 60).toFixed(0)} 分钟
              </p>
            </button>
          ))}
          {dramas.length === 0 && (
            <p className="px-3 py-6 text-center text-xs text-mocha-muted">
              还没有剧目,点右上「导入」选择剧目录
            </p>
          )}
        </div>
      </aside>

      {/* 右侧:详情 */}
      {!active ? (
        <div className="flex-1">
          <EmptyState
            icon={<Clapperboard size={40} />}
            title="导入一部剧开始拆解"
            desc="选择「一部剧一个目录」的素材文件夹,文件名需含「第NN集」"
            action={
              <Button onClick={importDir} disabled={busy}>
                <FolderPlus size={15} className="mr-1.5 inline" />
                导入剧目录
              </Button>
            }
          />
        </div>
      ) : (
        <section className="min-w-0 flex-1 overflow-auto p-5">
          {/* 头部 */}
          <header className="mb-4 flex flex-wrap items-center gap-3">
            <div className="min-w-0 flex-1">
              <h2 className="truncate text-lg font-semibold text-mocha-text">{active.name}</h2>
              <p className="mt-0.5 text-xs text-mocha-muted">
                {active.episodeCount} 集 · 总时长 {(active.totalDurationSec / 60).toFixed(1)} 分钟
                {tasks.length > 0 && ` · 任务 ${totalDone}/${tasks.length}`}
              </p>
            </div>
            {running ? (
              <>
                <Badge text="拆解中" tone="green" />
                <Button variant="ghost" disabled={busy} onClick={() => guard(() => dna.cancelPipeline(active.id))}>
                  <Square size={14} className="mr-1 inline" />
                  停止
                </Button>
              </>
            ) : (
              <>
                {failures.length > 0 && (
                  <Button
                    variant="ghost"
                    disabled={busy}
                    onClick={() =>
                      guard(async () => {
                        await dna.retryFailed(active.id);
                        await dna.runPipeline(active.id);
                      })
                    }
                  >
                    <RotateCcw size={14} className="mr-1 inline" />
                    重试失败({failures.length})
                  </Button>
                )}
                <Button
                  disabled={busy}
                  onClick={() =>
                    allDone
                      ? setRerunOpen(true)
                      : guard(() => dna.runPipeline(active.id))
                  }
                >
                  <Play size={14} className="mr-1 inline" />
                  {allDone ? "重新拆解" : tasks.length > 0 ? "继续拆解" : "开始拆解"}
                </Button>
              </>
            )}
            <Button
              variant="danger"
              disabled={busy || running}
              onClick={() =>
                guard(async () => {
                  await dna.deleteDrama(active.id);
                  setActiveId(null);
                  await reloadDramas();
                })
              }
            >
              <Trash2 size={14} />
            </Button>
          </header>

          {error && (
            <p className="mb-3 rounded-mocha bg-mocha-rose/10 px-3 py-2 text-xs text-mocha-rose">
              {error}
            </p>
          )}

          {/* 当前活动状态行 —— 转码/上传/模型调用等无进度条阶段的实时反馈 */}
          {running && activity && activity.text && (
            <p className="mb-3 flex items-center gap-2 rounded-mocha bg-mocha-surface/60 px-3 py-2 font-mono text-xs text-mocha-subtext">
              <span className="inline-block h-1.5 w-1.5 animate-pulse rounded-full bg-mocha-green" />
              {activity.text}
              <span className="ml-auto text-[10px] text-mocha-muted">
                {activity.ageSecs < 3 ? "刚刚" : `${activity.ageSecs}s 前`}
              </span>
            </p>
          )}

          {/* 阶段进度 */}
          {tasks.length === 0 ? (
            <div className="pane p-6 text-center text-sm text-mocha-muted">
              点「开始拆解」建立任务并执行:A 全局资产 → B 分集资产 → C 聚合资产。
              中途可停止,再次点击从断点继续。
            </div>
          ) : (
            STAGE_ORDER.map((stage) => {
              const rows = progress.get(stage) ?? [];
              if (rows.length === 0) return null;
              return (
                <div key={stage} className="mb-4">
                  <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-mocha-muted">
                    {STAGE_LABELS[stage]}
                  </h3>
                  <div className="pane divide-y divide-mocha-rim/30">
                    {rows.map(({ spec, total, done, failed, processing }) => (
                      <div key={spec.id} className="flex items-center gap-3 px-4 py-2.5">
                        <span className="w-24 shrink-0 text-[13px] font-medium text-mocha-text">
                          {spec.name}
                        </span>
                        <div className="h-1.5 min-w-0 flex-1 overflow-hidden rounded-full bg-mocha-crust">
                          <div className="flex h-full">
                            <div
                              className="h-full bg-mocha-green transition-all"
                              style={{ width: `${total ? (done / total) * 100 : 0}%` }}
                            />
                            <div
                              className="h-full bg-mocha-rose transition-all"
                              style={{ width: `${total ? (failed / total) * 100 : 0}%` }}
                            />
                          </div>
                        </div>
                        <span className="w-20 shrink-0 text-right text-xs tabular-nums text-mocha-muted">
                          {done}/{total}
                          {processing > 0 && <span className="text-mocha-blue"> ·{processing}</span>}
                          {failed > 0 && <span className="text-mocha-rose"> ✕{failed}</span>}
                        </span>
                        <Button
                          variant="ghost"
                          className="!px-2 !py-0.5 text-[11px]"
                          disabled={busy || running}
                          title="清空该资产结果并重跑"
                          onClick={() =>
                            guard(async () => {
                              await dna.rerunSpec(active.id, spec.id);
                              await dna.runPipeline(active.id);
                            })
                          }
                        >
                          重跑
                        </Button>
                      </div>
                    ))}
                  </div>
                </div>
              );
            })
          )}

          {/* 失败明细 */}
          {failures.length > 0 && (
            <div className="mb-4">
              <button
                className="mb-2 text-xs font-medium text-mocha-rose hover:underline"
                onClick={() => setShowFailures((v) => !v)}
              >
                {showFailures ? "收起" : "展开"}失败任务({failures.length})
              </button>
              {showFailures && (
                <div className="pane divide-y divide-mocha-rim/30">
                  {failures.map((t) => (
                    <div key={t.id} className="px-4 py-2">
                      <p className="text-xs font-medium text-mocha-text">
                        {specName(t.specId)} · {epNo(t)}
                      </p>
                      <p className="mt-0.5 break-all text-[11px] text-mocha-rose">{t.error}</p>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* 分集清单 */}
          <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-mocha-muted">
            分集({episodes.length})
          </h3>
          <div className="pane max-h-72 overflow-auto">
            {episodes.map((e) => (
              <div
                key={e.id}
                className="flex items-baseline gap-3 border-b border-mocha-rim/20 px-4 py-1.5 last:border-0"
              >
                <span className="w-14 shrink-0 text-xs tabular-nums text-mocha-muted">
                  第{e.epNo}集
                </span>
                <span className="min-w-0 flex-1 truncate text-[13px] text-mocha-subtext">
                  {e.title || "—"}
                </span>
                <span className="shrink-0 text-[11px] tabular-nums text-mocha-muted">
                  {e.durationSec ? `${e.durationSec.toFixed(0)}s` : ""}
                </span>
              </div>
            ))}
          </div>
        </section>
      )}

      {/* 重新拆解确认 */}
      {rerunOpen && active && (
        <Modal
          title="重新拆解整部剧?"
          onClose={() => setRerunOpen(false)}
          footer={
            <>
              <Button variant="ghost" onClick={() => setRerunOpen(false)}>
                取消
              </Button>
              <Button
                disabled={busy}
                onClick={() =>
                  guard(async () => {
                    await dna.resetDramaTasks(active.id);
                    await dna.runPipeline(active.id);
                    setRerunOpen(false);
                  })
                }
              >
                确认重新拆解
              </Button>
            </>
          }
        >
          <p className="text-sm leading-relaxed text-mocha-subtext">
            将清空本剧全部任务结果并重新调用模型(重新产生 API 费用),
            产出 md 文件会被新结果覆盖。仅想重跑个别资产时,请改用资产行内的「重跑」。
          </p>
        </Modal>
      )}

    </div>
  );
}
