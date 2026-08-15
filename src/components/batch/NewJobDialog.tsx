import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen } from "lucide-react";
import { useAppStore } from "@/store/useAppStore";
import { ipc } from "@/services/ipc";
import type { BatchJob } from "@/types";
import { Modal } from "@/components/ui/Modal";
import { Button } from "@/components/ui/Button";
import { Field } from "@/components/ui/Field";
import { TextInput } from "@/components/ui/TextInput";

interface Props {
  onClose: () => void;
  onCreated: (job: BatchJob) => void;
}

export function NewJobDialog({ onClose, onCreated }: Props) {
  const schemes = useAppStore((s) => s.schemes);
  const [name, setName] = useState("");
  const [schemeId, setSchemeId] = useState(schemes[0]?.id ?? "");
  const [files, setFiles] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");

  const pickFiles = async () => {
    const selected = await open({
      multiple: true,
      filters: [
        {
          name: "视频",
          extensions: ["mp4", "mov", "mkv", "avi", "webm", "m4v"],
        },
      ],
    });
    if (Array.isArray(selected)) setFiles(selected);
  };

  const valid = name.trim() !== "" && schemeId !== "" && files.length > 0;

  const submit = async () => {
    if (!valid) return;
    setBusy(true);
    setErr("");
    try {
      const job = await ipc.createBatchJob(name.trim(), schemeId, files);
      onCreated(job);
      onClose();
    } catch (e) {
      setErr(String(e));
      setBusy(false);
    }
  };

  return (
    <Modal
      title="新建批量任务"
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            取消
          </Button>
          <Button onClick={submit} disabled={!valid || busy}>
            {busy ? "创建中…" : "创建任务"}
          </Button>
        </>
      }
    >
      {schemes.length === 0 ? (
        <p className="py-4 text-center text-xs text-mocha-muted">
          请先在「方案管理」创建至少一个方案。
        </p>
      ) : (
        <div className="flex flex-col gap-3.5">
          <Field label="任务名称">
            <TextInput
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="例如:第一季短剧批量分析"
              autoFocus
            />
          </Field>
          <Field label="方案" hint="批量任务对所有文件使用同一个方案。">
            <select
              value={schemeId}
              onChange={(e) => setSchemeId(e.target.value)}
              className="rounded-mocha border border-mocha-rim/60 bg-mocha-crust/60 px-3 py-1.5 text-sm text-mocha-text outline-none focus:border-mocha-accent/70"
            >
              {schemes.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </select>
          </Field>
          <Field label="视频文件">
            <button
              onClick={pickFiles}
              className="flex items-center justify-center gap-1.5 rounded-mocha border border-dashed border-mocha-rim/70 bg-mocha-crust/40 px-3 py-2.5 text-xs text-mocha-subtext transition-colors hover:border-mocha-accent/60 hover:text-mocha-text"
            >
              <FolderOpen size={14} />
              {files.length > 0 ? `已选 ${files.length} 个文件` : "选择视频文件…"}
            </button>
          </Field>
          {err && <p className="text-xs text-mocha-rose">{err}</p>}
        </div>
      )}
    </Modal>
  );
}
