import type { ResultState } from "@/types";

interface Props {
  modelLabel: string;
  result: ResultState | undefined;
}

export function ResultColumn({ modelLabel, result }: Props) {
  const status = result?.status ?? "running";
  return (
    <div className="pane-inset flex min-w-[280px] flex-1 flex-col">
      <div className="flex shrink-0 items-center justify-between border-b border-mocha-rim/40 px-3 py-2">
        <span className="truncate text-xs font-medium text-mocha-text">
          {modelLabel}
        </span>
        <StatusDot status={status} />
      </div>
      <div className="min-h-0 flex-1 overflow-auto px-3 py-2.5">
        {status === "error" ? (
          <p className="text-xs leading-relaxed text-mocha-rose">
            {result?.error ?? "未知错误"}
          </p>
        ) : result?.text ? (
          <p className="whitespace-pre-wrap text-[13px] leading-relaxed text-mocha-subtext">
            {result.text}
            {status === "running" && (
              <span className="ml-0.5 animate-pulse text-mocha-accent">▋</span>
            )}
          </p>
        ) : (
          <p className="text-xs text-mocha-muted">
            {status === "running" ? "等待响应…" : "无结果"}
          </p>
        )}
      </div>
    </div>
  );
}

function StatusDot({ status }: { status: ResultState["status"] }) {
  const tone: Record<ResultState["status"], string> = {
    running: "bg-mocha-blue animate-pulse",
    done: "bg-mocha-green",
    error: "bg-mocha-rose",
  };
  return <span className={`h-2 w-2 shrink-0 rounded-full ${tone[status]}`} />;
}
