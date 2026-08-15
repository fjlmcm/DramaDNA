import type { InputHTMLAttributes } from "react";

export function TextInput({
  className = "",
  ...rest
}: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      {...rest}
      className={`rounded-mocha border border-mocha-rim/60 bg-mocha-crust/60 px-3 py-1.5 text-sm text-mocha-text outline-none transition-colors placeholder:text-mocha-muted focus:border-mocha-accent/70 ${className}`}
    />
  );
}
