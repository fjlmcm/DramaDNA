import { useState } from "react";
import { useAppStore } from "@/store/useAppStore";
import type { Model, ModelInput, VideoInputMethod } from "@/types";
import {
  VIDEO_INPUT_METHODS,
  RESOLUTION_TIERS,
  AUDIO_TIERS,
  parseConstraints,
  resolutionTier,
} from "@/types";
import { Modal } from "@/components/ui/Modal";
import { Button } from "@/components/ui/Button";
import { Field } from "@/components/ui/Field";
import { TextInput } from "@/components/ui/TextInput";
import { Select } from "@/components/ui/Select";

interface Props {
  model?: Model;
  onClose: () => void;
}

export function ModelForm({ model, onClose }: Props) {
  const providers = useAppStore((s) => s.providers);
  const addModel = useAppStore((s) => s.addModel);
  const editModel = useAppStore((s) => s.editModel);

  const [providerId, setProviderId] = useState(
    model?.providerId ?? providers[0]?.id ?? "",
  );
  const [modelId, setModelId] = useState(model?.modelId ?? "");
  const [displayName, setDisplayName] = useState(model?.displayName ?? "");
  const [videoInputMethod, setVideoInputMethod] = useState<VideoInputMethod>(
    model?.videoInputMethod ?? "base64",
  );
  const [enabled, setEnabled] = useState(model?.enabled ?? true);

  // 视频限制 —— 体积/帧率为数值,分辨率/音频为档位。时长不设限。
  const c0 = parseConstraints(model?.constraints ?? "{}");
  const [maxMb, setMaxMb] = useState(Math.round(c0.maxBytes / 1024 / 1024));
  const [maxFps, setMaxFps] = useState(c0.maxFps);
  const [resolution, setResolution] = useState(resolutionTier(c0.maxWidth));
  const [audioBitrate, setAudioBitrate] = useState(
    AUDIO_TIERS.some((t) => t.value === c0.audioBitrate)
      ? c0.audioBitrate
      : 64000,
  );

  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");

  const valid =
    providerId !== "" && modelId.trim() !== "" && displayName.trim() !== "";

  const submit = async () => {
    if (!valid) return;
    setBusy(true);
    setErr("");
    const longEdge =
      RESOLUTION_TIERS.find((t) => t.value === resolution)?.longEdge ?? 854;
    const input: ModelInput = {
      providerId,
      modelId: modelId.trim(),
      displayName: displayName.trim(),
      videoInputMethod,
      enabled,
      constraints: JSON.stringify({
        maxBytes: Math.max(1, maxMb) * 1024 * 1024,
        maxWidth: longEdge,
        maxHeight: longEdge,
        maxFps: Math.max(1, maxFps),
        audioBitrate,
      }),
    };
    try {
      if (model) await editModel(model.id, input);
      else await addModel(input);
      onClose();
    } catch (e) {
      setErr(String(e));
      setBusy(false);
    }
  };

  return (
    <Modal
      title={model ? "编辑模型" : "新增模型"}
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            取消
          </Button>
          <Button onClick={submit} disabled={!valid || busy}>
            {busy ? "保存中…" : "保存"}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3.5">
        <Field label="所属供应商">
          <Select
            value={providerId}
            onChange={(e) => setProviderId(e.target.value)}
            options={providers.map((p) => ({ value: p.id, label: p.name }))}
          />
        </Field>
        <Field
          label="模型标识"
          hint="供应商侧的模型 ID,例如 doubao-seed-2-0-lite-260428"
        >
          <TextInput
            value={modelId}
            onChange={(e) => setModelId(e.target.value)}
            placeholder="doubao-seed-2-0-lite-260428"
          />
        </Field>
        <Field label="显示名称">
          <TextInput
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
            placeholder="例如:豆包 Seed 2.0 Lite"
          />
        </Field>
        <Field
          label="视频输入方式"
          hint="视频文件通常较大,优先用 File API;Base64 仅适合小片段。"
        >
          <Select
            value={videoInputMethod}
            onChange={(e) =>
              setVideoInputMethod(e.target.value as VideoInputMethod)
            }
            options={VIDEO_INPUT_METHODS.map((m) => ({
              value: m.value,
              label: m.label,
            }))}
          />
        </Field>

        {/* 视频限制 */}
        <div className="rounded-mocha border border-mocha-rim/50 p-3">
          <p className="text-xs font-medium text-mocha-subtext">视频限制</p>
          <p className="mb-2.5 mt-0.5 text-[11px] leading-snug text-mocha-muted">
            视频超出限制时,先用 ffmpeg 本地转码(降分辨率/帧率)再上传;时长不限。
          </p>
          <div className="grid grid-cols-2 gap-2.5">
            <NumberField label="最大体积 (MB)" value={maxMb} onChange={setMaxMb} />
            <NumberField
              label="最大帧率 (fps)"
              value={maxFps}
              onChange={setMaxFps}
            />
            <TierSelect
              label="分辨率"
              value={resolution}
              onChange={setResolution}
              options={RESOLUTION_TIERS.map((t) => ({
                value: t.value,
                label: t.label,
              }))}
            />
            <TierSelect
              label="音频"
              value={String(audioBitrate)}
              onChange={(v) => setAudioBitrate(Number(v))}
              options={AUDIO_TIERS.map((t) => ({
                value: String(t.value),
                label: t.label,
              }))}
            />
          </div>
        </div>

        <div className="flex items-center justify-between pt-0.5">
          <span className="text-xs font-medium text-mocha-subtext">启用</span>
          <button
            type="button"
            onClick={() => setEnabled(!enabled)}
            role="switch"
            aria-checked={enabled}
            className={`relative h-6 w-11 rounded-full transition-colors ${
              enabled ? "bg-mocha-accent" : "bg-mocha-overlay"
            }`}
          >
            <span
              className={`absolute top-0.5 block h-5 w-5 rounded-full bg-mocha-crust transition-transform ${
                enabled ? "translate-x-5" : "translate-x-0.5"
              }`}
            />
          </button>
        </div>
        {err && <p className="text-xs text-mocha-rose">{err}</p>}
      </div>
    </Modal>
  );
}

function NumberField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (n: number) => void;
}) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-[11px] text-mocha-muted">{label}</span>
      <input
        type="number"
        min={1}
        value={value}
        onChange={(e) => onChange(Number(e.target.value) || 0)}
        className="rounded-mocha border border-mocha-rim/60 bg-mocha-crust/60 px-2 py-1 text-xs text-mocha-text outline-none transition-colors focus:border-mocha-accent/70"
      />
    </label>
  );
}

function TierSelect({
  label,
  value,
  onChange,
  options,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  options: { value: string; label: string }[];
}) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-[11px] text-mocha-muted">{label}</span>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="rounded-mocha border border-mocha-rim/60 bg-mocha-crust/60 px-2 py-1 text-xs text-mocha-text outline-none transition-colors focus:border-mocha-accent/70"
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </label>
  );
}
