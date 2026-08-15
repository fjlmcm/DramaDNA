import { useEffect, useState } from "react";
import { ipc } from "@/services/ipc";
import { useAppStore } from "@/store/useAppStore";
import { Button } from "@/components/ui/Button";
import { Field } from "@/components/ui/Field";
import { Select } from "@/components/ui/Select";

const DEFAULTS = {
  dna_video_model: "",
  dna_video_model_episode: "",
  dna_text_model: "",
  dna_concurrency: "10",
  dna_starts_per_min: "12",
};
type Keys = keyof typeof DEFAULTS;

export function PipelineSettings() {
  const models = useAppStore((s) => s.models);
  const [values, setValues] = useState({ ...DEFAULTS });
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    Promise.all(
      (Object.keys(DEFAULTS) as Keys[]).map(async (k) => [k, await ipc.getSetting(k)] as const),
    )
      .then((pairs) => {
        setValues((v) => {
          const next = { ...v };
          for (const [k, val] of pairs) if (val != null) next[k] = val;
          return next;
        });
      })
      .catch((e) => console.error("加载管线设置失败:", e));
  }, []);

  const save = async () => {
    setBusy(true);
    setSaved(false);
    try {
      for (const k of Object.keys(DEFAULTS) as Keys[]) {
        await ipc.setSetting(k, values[k]);
      }
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      console.error("保存管线设置失败:", e);
    } finally {
      setBusy(false);
    }
  };

  const modelOptions = (hint: string) => [
    { value: "", label: hint },
    ...models.filter((m) => m.enabled).map((m) => ({ value: m.id, label: m.displayName })),
  ];

  return (
    <div>
      <div className="mb-4">
        <h2 className="text-sm font-semibold text-mocha-text">拆解管线</h2>
        <p className="mt-0.5 text-xs text-mocha-muted">
          剧目拆解的默认模型与执行参数。单个资产可在「资产配置」里单独绑定模型覆盖默认。
        </p>
      </div>

      <div className="pane-inset space-y-4 p-4">
        <Field
          label="视频模型(全片任务)"
          hint="全剧拼接的视频调用。推荐豆包 Files API —— 唯一无时长墙的通路(Gemini 中转超 60 分钟必拒)。"
        >
          <Select
            value={values.dna_video_model}
            onChange={(e) => setValues({ ...values, dna_video_model: e.target.value })}
            options={modelOptions("未设置 —— 视频任务将报错提示")}
          />
        </Field>
        <Field
          label="分集视频模型(单集任务)"
          hint="台词/拆解卡/分镜表/标注等分集调用优先用此模型,留空则跟随视频模型。实测 Gemini Pro 画外音标注与画面文字最强。"
        >
          <Select
            value={values.dna_video_model_episode}
            onChange={(e) => setValues({ ...values, dna_video_model_episode: e.target.value })}
            options={modelOptions("跟随视频模型")}
          />
        </Field>
        <Field
          label="文本模型(C 聚合 / D 二创阶段)"
          hint="纯文本调用,吃全剧拆解文本,选上下文长、写作强的模型。"
        >
          <Select
            value={values.dna_text_model}
            onChange={(e) => setValues({ ...values, dna_text_model: e.target.value })}
            options={modelOptions("未设置 —— 文本任务将报错提示")}
          />
        </Field>
        <div className="flex gap-6">
          <Field label="并发数" hint="同时执行的任务单元数,默认 10。管线启动时读取,运行中修改需停止后重新继续才生效。">
            <input
              type="number"
              min={1}
              max={20}
              value={values.dna_concurrency}
              onChange={(e) => setValues({ ...values, dna_concurrency: e.target.value })}
              className="w-32 rounded-mocha border border-mocha-rim/60 bg-mocha-crust/60 px-3 py-1.5 text-sm text-mocha-text outline-none focus:border-mocha-accent/70"
            />
          </Field>
          <Field label="每分钟启动任务数" hint="平滑 TPM 消耗防限流。按账号配额 ÷ 单任务约 8 万 token 估算:100 万 TPM 填 12,提额后按比例上调。">
            <input
              type="number"
              min={1}
              max={100}
              value={values.dna_starts_per_min}
              onChange={(e) => setValues({ ...values, dna_starts_per_min: e.target.value })}
              className="w-32 rounded-mocha border border-mocha-rim/60 bg-mocha-crust/60 px-3 py-1.5 text-sm text-mocha-text outline-none focus:border-mocha-accent/70"
            />
          </Field>
        </div>
        <div className="flex items-center gap-2.5">
          <Button onClick={save} disabled={busy}>
            {busy ? "保存中…" : "保存"}
          </Button>
          {saved && <span className="text-xs text-mocha-green">已保存</span>}
        </div>
      </div>
    </div>
  );
}
