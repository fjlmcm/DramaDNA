import { useState } from "react";
import { useAppStore } from "@/store/useAppStore";
import type { Scheme, SchemeInput } from "@/types";
import { Modal } from "@/components/ui/Modal";
import { Button } from "@/components/ui/Button";
import { Field } from "@/components/ui/Field";
import { TextInput } from "@/components/ui/TextInput";

interface Props {
  scheme?: Scheme;
  onClose: () => void;
}

export function SchemeForm({ scheme, onClose }: Props) {
  const models = useAppStore((s) => s.models);
  const enabledModels = models.filter((m) => m.enabled);
  const addScheme = useAppStore((s) => s.addScheme);
  const editScheme = useAppStore((s) => s.editScheme);

  const [name, setName] = useState(scheme?.name ?? "");
  const [modelId, setModelId] = useState(
    scheme?.modelId ?? enabledModels[0]?.id ?? "",
  );
  const [prompt, setPrompt] = useState(scheme?.prompt ?? "");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");

  const valid = name.trim() !== "" && modelId !== "" && prompt.trim() !== "";

  const submit = async () => {
    if (!valid) return;
    setBusy(true);
    setErr("");
    const input: SchemeInput = {
      name: name.trim(),
      modelId,
      prompt: prompt.trim(),
    };
    try {
      if (scheme) await editScheme(scheme.id, input);
      else await addScheme(input);
      onClose();
    } catch (e) {
      setErr(String(e));
      setBusy(false);
    }
  };

  return (
    <Modal
      title={scheme ? "编辑方案" : "新建方案"}
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
        <Field label="方案名称">
          <TextInput
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="例如:剧情梗概提取"
            autoFocus
          />
        </Field>
        <Field label="模型">
          <select
            value={modelId}
            onChange={(e) => setModelId(e.target.value)}
            className="rounded-mocha border border-mocha-rim/60 bg-mocha-crust/60 px-3 py-1.5 text-sm text-mocha-text outline-none focus:border-mocha-accent/70"
          >
            {enabledModels.map((m) => (
              <option key={m.id} value={m.id}>
                {m.displayName}
              </option>
            ))}
          </select>
        </Field>
        <Field label="提示词">
          <textarea
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            rows={5}
            placeholder="对视频提出的理解要求…"
            className="resize-y rounded-mocha border border-mocha-rim/60 bg-mocha-crust/60 px-3 py-2 text-sm leading-relaxed text-mocha-text outline-none placeholder:text-mocha-muted focus:border-mocha-accent/70"
          />
        </Field>
        {err && <p className="text-xs text-mocha-rose">{err}</p>}
      </div>
    </Modal>
  );
}
