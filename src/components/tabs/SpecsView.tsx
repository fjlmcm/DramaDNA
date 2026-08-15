import { useEffect, useMemo, useState } from "react";
import { ChevronDown, ChevronRight, RotateCcw } from "lucide-react";
import type { AssetSpec, SpecStage } from "@/types";
import { STAGE_LABELS, STAGE_ORDER } from "@/types";
import { dna } from "@/services/ipc";
import { useAppStore } from "@/store/useAppStore";
import { Button } from "@/components/ui/Button";
import { Badge } from "@/components/ui/Badge";
import { Field } from "@/components/ui/Field";
import { Modal } from "@/components/ui/Modal";
import { Select } from "@/components/ui/Select";

/** 解析 spec.inputs 的依赖 id 列表(剥掉 :all 后缀)。 */
const depsOf = (s: AssetSpec): string[] => {
  try {
    return (JSON.parse(s.inputs) as string[]).map((d) => d.replace(/:all$/, ""));
  } catch {
    return [];
  }
};

const SCOPE_LABELS: Record<string, string> = {
  per_segment: "按段(分段提取+合并)",
  per_episode: "按集",
  per_drama: "全剧一次",
};

export function SpecsView() {
  const models = useAppStore((s) => s.models);
  const [specs, setSpecs] = useState<AssetSpec[]>([]);
  const [openId, setOpenId] = useState<string | null>(null);
  const [draft, setDraft] = useState<AssetSpec | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** 待确认的开关操作:related = 启用时要连带启用的上游 / 停用时受影响的下游。 */
  const [confirmToggle, setConfirmToggle] = useState<{
    spec: AssetSpec;
    related: AssetSpec[];
  } | null>(null);

  useEffect(() => {
    dna.listSpecs().then(setSpecs).catch((e) => setError(String(e)));
  }, []);

  const byStage = useMemo(() => {
    const m = new Map<SpecStage, AssetSpec[]>();
    for (const st of STAGE_ORDER) m.set(st, []);
    for (const s of specs) m.get(s.stage)?.push(s);
    return m;
  }, [specs]);

  const modelOptions = [
    { value: "", label: "跟随管线默认模型" },
    ...models.filter((m) => m.enabled).map((m) => ({ value: m.id, label: m.displayName })),
  ];

  const toggle = (spec: AssetSpec) => {
    if (openId === spec.id) {
      setOpenId(null);
      setDraft(null);
    } else {
      setOpenId(spec.id);
      setDraft({ ...spec });
    }
  };

  const save = async () => {
    if (!draft) return;
    setBusy(true);
    setError(null);
    try {
      const updated = await dna.updateSpec(draft.id, {
        prompt: draft.prompt,
        mergePrompt: draft.mergePrompt,
        modelId: draft.modelId || null,
        enabled: draft.enabled,
        params: draft.params,
      });
      setSpecs((list) => list.map((s) => (s.id === updated.id ? updated : s)));
      setDraft(updated);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const reset = async () => {
    if (!draft) return;
    setBusy(true);
    setError(null);
    try {
      const restored = await dna.resetSpec(draft.id);
      setSpecs((list) => list.map((s) => (s.id === restored.id ? restored : s)));
      setDraft(restored);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  /** 传递闭包:collect(种子, 一步展开) 直到收敛。 */
  const closure = (seed: AssetSpec, expand: (s: AssetSpec) => AssetSpec[]): AssetSpec[] => {
    const seen = new Map<string, AssetSpec>();
    let frontier = [seed];
    while (frontier.length > 0) {
      frontier = frontier
        .flatMap(expand)
        .filter((s) => s.id !== seed.id && !seen.has(s.id));
      for (const s of frontier) seen.set(s.id, s);
    }
    return [...seen.values()];
  };

  /** 应用启用/停用到一组资产,保留其余字段。 */
  const applyEnabled = async (targets: AssetSpec[], enabled: boolean) => {
    setBusy(true);
    setError(null);
    try {
      for (const s of targets) {
        const updated = await dna.updateSpec(s.id, {
          prompt: s.prompt,
          mergePrompt: s.mergePrompt,
          modelId: s.modelId,
          enabled,
          params: s.params,
        });
        setSpecs((list) => list.map((x) => (x.id === updated.id ? updated : x)));
        if (draft?.id === updated.id) setDraft(updated);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  /** 行级开关入口:有依赖联动时先弹确认。 */
  const requestToggle = (spec: AssetSpec) => {
    const related = spec.enabled
      ? // 停用:列出仍启用、且(传递)依赖它的下游 —— 它们将不会建任务。
        closure(spec, (s) =>
          specs.filter((x) => x.enabled && depsOf(x).includes(s.id)),
        )
      : // 启用:遍历全部上游依赖闭包,连带启用其中仍停用的。
        closure(spec, (s) =>
          depsOf(s)
            .map((id) => specs.find((x) => x.id === id))
            .filter((x): x is AssetSpec => !!x),
        ).filter((x) => !x.enabled);
    if (related.length === 0) {
      void applyEnabled([spec], !spec.enabled);
    } else {
      setConfirmToggle({ spec, related });
    }
  };

  return (
    <div className="mx-auto max-w-4xl p-5">
      <p className="mb-4 text-xs leading-relaxed text-mocha-muted">
        每个资产 = 管线里的一次模型调用(「每次调用只做一件事」)。行首开关控制是否提取
        ——仅需部分项目时按需开关,依赖关系会联动提示;还可展开编辑 prompt、绑定模型。
        占位符 <code className="text-mocha-subtext">{"{drama_name} {ep_no} {ep_range}"}</code> 等在执行时替换;
        依赖资产会作为「参考资料」自动附加。
      </p>
      {error && (
        <p className="mb-3 rounded-mocha bg-mocha-rose/10 px-3 py-2 text-xs text-mocha-rose">{error}</p>
      )}
      {STAGE_ORDER.map((stage) => (
        <div key={stage} className="mb-5">
          <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-mocha-muted">
            {STAGE_LABELS[stage]}
          </h3>
          <div className="pane divide-y divide-mocha-rim/30">
            {(byStage.get(stage) ?? []).map((spec) => {
              const open = openId === spec.id;
              const model = models.find((m) => m.id === spec.modelId);
              return (
                <div key={spec.id}>
                  <div
                    className={`flex w-full items-center gap-2.5 px-4 py-2.5 ${
                      spec.enabled ? "" : "opacity-55"
                    }`}
                  >
                    {/* 行级启用开关 —— 只提取需要的项目 */}
                    <button
                      onClick={() => requestToggle(spec)}
                      disabled={busy}
                      title={spec.enabled ? "停用(拆解时不建任务)" : "启用"}
                      className={`relative h-4 w-7 shrink-0 rounded-full transition-colors ${
                        spec.enabled ? "bg-mocha-green/70" : "bg-mocha-rim"
                      }`}
                    >
                      <span
                        className={`absolute top-0.5 h-3 w-3 rounded-full bg-mocha-crust transition-all ${
                          spec.enabled ? "left-3.5" : "left-0.5"
                        }`}
                      />
                    </button>
                    <button
                      onClick={() => toggle(spec)}
                      className="flex min-w-0 flex-1 items-center gap-2.5 text-left hover:opacity-80"
                    >
                      {open ? (
                        <ChevronDown size={14} className="shrink-0 text-mocha-muted" />
                      ) : (
                        <ChevronRight size={14} className="shrink-0 text-mocha-muted" />
                      )}
                      <span className="w-24 shrink-0 text-[13px] font-medium text-mocha-text">
                        {spec.name}
                      </span>
                      <span className="text-[11px] text-mocha-muted">{SCOPE_LABELS[spec.scope]}</span>
                      {spec.needsVideo && <Badge text="视频" tone="blue" />}
                      <span className="min-w-0 flex-1 truncate text-right text-[11px] text-mocha-muted">
                        {model ? model.displayName : "默认模型"} · {spec.outputTemplate}
                      </span>
                    </button>
                  </div>
                  {open && draft && draft.id === spec.id && (
                    <div className="space-y-3 border-t border-mocha-rim/30 bg-mocha-crust/30 px-4 py-3">
                      <Field label="Prompt(执行时自动拼接上下文头与参考资料)">
                        <textarea
                          value={draft.prompt}
                          onChange={(e) => setDraft({ ...draft, prompt: e.target.value })}
                          rows={12}
                          className="w-full resize-y rounded-mocha border border-mocha-rim/60 bg-mocha-crust/60 px-3 py-2 font-mono text-xs leading-relaxed text-mocha-text outline-none focus:border-mocha-accent/70"
                        />
                      </Field>
                      {draft.scope === "per_segment" && (
                        <Field label="分段合并 Prompt(留空用内置模板)">
                          <textarea
                            value={draft.mergePrompt ?? ""}
                            onChange={(e) =>
                              setDraft({ ...draft, mergePrompt: e.target.value || null })
                            }
                            rows={4}
                            className="w-full resize-y rounded-mocha border border-mocha-rim/60 bg-mocha-crust/60 px-3 py-2 font-mono text-xs leading-relaxed text-mocha-text outline-none focus:border-mocha-accent/70"
                          />
                        </Field>
                      )}
                      <div className="flex flex-wrap items-end gap-4">
                        <Field label="绑定模型">
                          <Select
                            value={draft.modelId ?? ""}
                            onChange={(e) =>
                              setDraft({ ...draft, modelId: e.target.value || null })
                            }
                            options={modelOptions}
                          />
                        </Field>
                        <Field label="请求参数(json,如 max_tokens)">
                          <input
                            value={draft.params}
                            onChange={(e) => setDraft({ ...draft, params: e.target.value })}
                            className="w-56 rounded-mocha border border-mocha-rim/60 bg-mocha-crust/60 px-3 py-1.5 font-mono text-xs text-mocha-text outline-none focus:border-mocha-accent/70"
                          />
                        </Field>
                        <label className="flex items-center gap-2 pb-1.5 text-xs text-mocha-subtext">
                          <input
                            type="checkbox"
                            checked={draft.enabled}
                            onChange={(e) => setDraft({ ...draft, enabled: e.target.checked })}
                          />
                          启用
                        </label>
                        <div className="ml-auto flex gap-2">
                          {draft.builtin && (
                            <Button variant="ghost" disabled={busy} onClick={reset}>
                              <RotateCcw size={13} className="mr-1 inline" />
                              恢复默认
                            </Button>
                          )}
                          <Button disabled={busy} onClick={save}>
                            保存
                          </Button>
                        </div>
                      </div>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      ))}

      {/* 依赖联动确认 */}
      {confirmToggle && (
        <Modal
          title={confirmToggle.spec.enabled ? "停用该资产?" : "启用该资产及其依赖?"}
          onClose={() => setConfirmToggle(null)}
          footer={
            <>
              <Button variant="ghost" onClick={() => setConfirmToggle(null)}>
                取消
              </Button>
              <Button
                disabled={busy}
                onClick={async () => {
                  const { spec, related } = confirmToggle;
                  if (spec.enabled) {
                    await applyEnabled([spec], false);
                  } else {
                    await applyEnabled([spec, ...related], true);
                  }
                  setConfirmToggle(null);
                }}
              >
                {confirmToggle.spec.enabled ? "仍要停用" : "一并启用"}
              </Button>
            </>
          }
        >
          <p className="text-sm leading-relaxed text-mocha-subtext">
            {confirmToggle.spec.enabled ? (
              <>
                以下资产依赖「{confirmToggle.spec.name}」,停用后它们在拆解时将
                <span className="text-mocha-rose">不会建任务、不会产出</span>:
              </>
            ) : (
              <>
                「{confirmToggle.spec.name}」依赖以下已停用的资产(证据链缺一不可),将一并启用:
              </>
            )}
          </p>
          <p className="mt-2 text-sm font-medium text-mocha-text">
            {confirmToggle.related.map((s) => s.name).join("、")}
          </p>
        </Modal>
      )}
    </div>
  );
}
