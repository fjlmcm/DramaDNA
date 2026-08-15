import type { ReactNode } from "react";

interface Props {
  icon?: ReactNode;
  title: string;
  desc?: string;
  action?: ReactNode;
}

export function EmptyState({ icon, title, desc, action }: Props) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 px-6 py-16 text-center">
      {icon && <div className="text-mocha-rim">{icon}</div>}
      <div>
        <p className="text-sm font-medium text-mocha-subtext">{title}</p>
        {desc && (
          <p className="mt-1 max-w-sm text-xs leading-relaxed text-mocha-muted">
            {desc}
          </p>
        )}
      </div>
      {action}
    </div>
  );
}
