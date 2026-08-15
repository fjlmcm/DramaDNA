import type { ReactNode } from "react";

interface Props {
  label: string;
  hint?: string;
  children: ReactNode;
}

export function Field({ label, hint, children }: Props) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="text-xs font-medium text-mocha-subtext">{label}</span>
      {children}
      {hint && <span className="text-[11px] leading-snug text-mocha-muted">{hint}</span>}
    </label>
  );
}
