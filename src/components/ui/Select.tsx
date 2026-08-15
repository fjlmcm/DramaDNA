import type { SelectHTMLAttributes } from "react";

interface Option {
  value: string;
  label: string;
}

interface Props extends SelectHTMLAttributes<HTMLSelectElement> {
  options: Option[];
}

export function Select({ options, className = "", ...rest }: Props) {
  return (
    <select
      {...rest}
      className={`rounded-mocha border border-mocha-rim/60 bg-mocha-crust/60 px-3 py-1.5 text-sm text-mocha-text outline-none transition-colors focus:border-mocha-accent/70 ${className}`}
    >
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  );
}
