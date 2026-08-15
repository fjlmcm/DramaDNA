import { useCallback, useEffect, useState } from "react";
import { Trash2, FolderOpen, RefreshCw } from "lucide-react";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import { ipc } from "@/services/ipc";
import type { CacheStats } from "@/types";
import { Button } from "@/components/ui/Button";

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

export function CacheSection() {
  const [stats, setStats] = useState<CacheStats | null>(null);
  const [busy, setBusy] = useState(false);

  const reload = useCallback(async () => {
    try {
      setStats(await ipc.cacheStats());
    } catch (e) {
      console.error("读取缓存统计失败:", e);
    }
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  const onClear = async () => {
    if (!stats || stats.fileCount === 0) return;
    const ok = await ask(
      `清空预处理缓存?将删除 ${stats.fileCount} 个文件,共 ${formatBytes(stats.totalBytes)}。\n\n源视频和应用数据不受影响,下次处理同样视频会重新转码。`,
      { title: "确认清空缓存", kind: "warning" },
    );
    if (!ok) return;
    setBusy(true);
    try {
      const removed = await ipc.clearCache();
      await reload();
      await message(`已释放 ${formatBytes(removed)} 空间。`, {
        title: "缓存已清空",
        kind: "info",
      });
    } catch (e) {
      await message(String(e), { title: "清空失败", kind: "error" });
    } finally {
      setBusy(false);
    }
  };

  const onOpen = async () => {
    if (!stats) return;
    try {
      await openPath(stats.path);
    } catch (e) {
      await message(String(e), { title: "打开失败", kind: "error" });
    }
  };

  return (
    <div>
      <div className="mb-4">
        <h2 className="text-sm font-semibold text-mocha-text">缓存</h2>
        <p className="mt-0.5 text-xs text-mocha-muted">
          视频预处理(转码)产物的本地缓存 ——
          同视频同约束第二次处理直接命中,免重复转码。
        </p>
      </div>

      <div className="pane-inset p-4">
        <div className="flex items-baseline gap-3">
          <div className="flex-1">
            <div className="text-2xl font-semibold text-mocha-text tabular-nums">
              {stats ? formatBytes(stats.totalBytes) : "—"}
            </div>
            <div className="mt-1 text-xs text-mocha-muted">
              {stats ? `${stats.fileCount} 个文件` : "加载中…"}
            </div>
          </div>
          <button
            onClick={reload}
            aria-label="刷新"
            className="rounded-mocha p-1.5 text-mocha-muted transition-colors hover:bg-mocha-overlay hover:text-mocha-text"
          >
            <RefreshCw size={14} />
          </button>
        </div>

        <div className="mt-3.5 truncate font-mono text-[11px] text-mocha-muted">
          {stats?.path ?? ""}
        </div>

        <div className="mt-3.5 flex items-center gap-2.5">
          <Button
            variant="danger"
            onClick={onClear}
            disabled={busy || !stats || stats.fileCount === 0}
          >
            <Trash2 size={14} /> {busy ? "清空中…" : "清空缓存"}
          </Button>
          <Button variant="ghost" onClick={onOpen} disabled={!stats}>
            <FolderOpen size={14} /> 在文件管理器打开
          </Button>
        </div>
      </div>
    </div>
  );
}
