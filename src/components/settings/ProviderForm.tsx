import { useState } from "react";
import { useAppStore } from "@/store/useAppStore";
import type { Provider, ProviderInput, ProviderKind } from "@/types";
import { PROVIDER_KINDS } from "@/types";
import { Modal } from "@/components/ui/Modal";
import { Button } from "@/components/ui/Button";
import { Field } from "@/components/ui/Field";
import { TextInput } from "@/components/ui/TextInput";
import { Select } from "@/components/ui/Select";

interface Props {
  provider?: Provider;
  onClose: () => void;
}

export function ProviderForm({ provider, onClose }: Props) {
  const addProvider = useAppStore((s) => s.addProvider);
  const editProvider = useAppStore((s) => s.editProvider);

  const [name, setName] = useState(provider?.name ?? "");
  const [kind, setKind] = useState<ProviderKind>(provider?.kind ?? "volcengine");
  const [baseUrl, setBaseUrl] = useState(provider?.baseUrl ?? "");
  const [apiKey, setApiKey] = useState(provider?.apiKey ?? "");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");

  const valid = name.trim() !== "" && baseUrl.trim() !== "";

  const submit = async () => {
    if (!valid) return;
    setBusy(true);
    setErr("");
    const input: ProviderInput = {
      name: name.trim(),
      kind,
      baseUrl: baseUrl.trim(),
      apiKey,
    };
    try {
      if (provider) await editProvider(provider.id, input);
      else await addProvider(input);
      onClose();
    } catch (e) {
      setErr(String(e));
      setBusy(false);
    }
  };

  return (
    <Modal
      title={provider ? "编辑供应商" : "新增供应商"}
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
        <Field label="名称">
          <TextInput
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="例如:火山引擎"
            autoFocus
          />
        </Field>
        <Field
          label="供应商类型"
          hint={PROVIDER_KINDS.find((k) => k.value === kind)?.hint}
        >
          <Select
            value={kind}
            onChange={(e) => setKind(e.target.value as ProviderKind)}
            options={PROVIDER_KINDS.map((k) => ({
              value: k.value,
              label: k.label,
            }))}
          />
        </Field>
        <Field label="API Base URL">
          <TextInput
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            placeholder="https://ark.cn-beijing.volces.com/api/v3"
          />
        </Field>
        <Field label="API Key" hint="明文保存在本地数据库。">
          <TextInput
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder="sk-..."
          />
        </Field>
        {err && <p className="text-xs text-mocha-rose">{err}</p>}
      </div>
    </Modal>
  );
}
