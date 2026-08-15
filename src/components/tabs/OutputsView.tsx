import { useEffect, useState } from "react";
import { openPath } from "@tauri-apps/plugin-opener";
import { Code, FileText, FolderOpen, RefreshCw } from "lucide-react";
import type { Drama, OutputFile } from "@/types";
import { dna } from "@/services/ipc";
import { Button } from "@/components/ui/Button";
import { EmptyState } from "@/components/ui/EmptyState";
import { Markdown } from "@/components/ui/Markdown";
import { Select } from "@/components/ui/Select";

export function OutputsView() {
  const [dramas, setDramas] = useState<Drama[]>([]);
  const [dramaId, setDramaId] = useState<string>("");
  const [files, setFiles] = useState<OutputFile[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [content, setContent] = useState<string>("");
  const [raw, setRaw] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    dna
      .listDramas()
      .then((list) => {
        setDramas(list);
        setDramaId((cur) => cur || list[0]?.id || "");
      })
      .catch((e) => setError(String(e)));
  }, []);

  const reloadFiles = (id: string) => {
    if (!id) return;
    dna
      .listOutputs(id)
      .then((fs) => {
        setFiles(fs);
        setSelected((cur) => (cur && fs.some((f) => f.relPath === cur) ? cur : fs[0]?.relPath ?? null));
      })
      .catch((e) => setError(String(e)));
  };

  useEffect(() => {
    setFiles([]);
    setSelected(null);
    setContent("");
    reloadFiles(dramaId);
  }, [dramaId]);

  useEffect(() => {
    if (!dramaId || !selected) return;
    dna
      .readOutput(dramaId, selected)
      .then(setContent)
      .catch((e) => setContent(`读取失败: ${e}`));
  }, [dramaId, selected]);

  const openDir = async () => {
    if (!dramaId) return;
    try {
      await openPath(await dna.outputDir(dramaId));
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-2 border-b border-mocha-rim/40 px-4 py-2.5">
        <Select
          value={dramaId}
          onChange={(e) => setDramaId(e.target.value)}
          options={dramas.map((d) => ({ value: d.id, label: d.name }))}
          className="max-w-72"
        />
        <Button variant="ghost" className="!px-2.5 !py-1.5" onClick={() => reloadFiles(dramaId)} title="刷新">
          <RefreshCw size={14} />
        </Button>
        <Button variant="ghost" onClick={openDir} disabled={!dramaId}>
          <FolderOpen size={14} className="mr-1.5 inline" />
          打开拆解目录
        </Button>
        <Button
          variant="ghost"
          onClick={() => setRaw((v) => !v)}
          title={raw ? "切回渲染视图" : "查看 md 原文(核对逐字台词用)"}
        >
          <Code size={14} className="mr-1.5 inline" />
          {raw ? "渲染" : "原文"}
        </Button>
        {error && <span className="text-xs text-mocha-rose">{error}</span>}
      </div>

      {files.length === 0 ? (
        <div className="flex-1">
          <EmptyState
            icon={<FileText size={40} />}
            title="还没有产出"
            desc="在「剧目拆解」里跑完管线后,md 文档会出现在这里(也直接写入剧目录/拆解/)"
          />
        </div>
      ) : (
        <div className="flex min-h-0 flex-1">
          <aside className="w-72 shrink-0 overflow-auto border-r border-mocha-rim/40 p-2">
            {files.map((f) => (
              <button
                key={f.relPath}
                onClick={() => setSelected(f.relPath)}
                className={`mb-0.5 flex w-full items-baseline gap-2 rounded-mocha px-2.5 py-1.5 text-left transition-colors ${
                  f.relPath === selected
                    ? "bg-mocha-surface text-mocha-text"
                    : "text-mocha-subtext hover:bg-mocha-surface/50"
                }`}
              >
                <span className="min-w-0 flex-1 truncate text-xs">{f.relPath}</span>
                <span className="shrink-0 text-[10px] tabular-nums text-mocha-muted">
                  {(f.sizeBytes / 1024).toFixed(1)}k
                </span>
              </button>
            ))}
          </aside>
          <div className="min-w-0 flex-1 overflow-auto px-6 py-4">
            {raw ? (
              <pre className="whitespace-pre-wrap font-mono text-[12.5px] leading-relaxed text-mocha-subtext">
                {content}
              </pre>
            ) : (
              <Markdown text={content} />
            )}
          </div>
        </div>
      )}
    </div>
  );
}
