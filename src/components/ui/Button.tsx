import type { ButtonHTMLAttributes } from "react";

type Variant = "primary" | "ghost" | "danger";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
}

const variants: Record<Variant, string> = {
  primary: "bg-mocha-accent text-mocha-crust hover:brightness-110 active:brightness-95",
  ghost: "bg-mocha-surface text-mocha-subtext hover:bg-mocha-overlay hover:text-mocha-text",
  danger: "bg-transparent text-mocha-rose hover:bg-mocha-rose/15",
};

export function Button({ variant = "primary", className = "", ...rest }: Props) {
  return (
    <button
      {...rest}
      className={`inline-flex items-center justify-center gap-1.5 rounded-mocha px-3.5 py-1.5 text-sm font-medium transition-[filter,background-color,color] duration-150 disabled:cursor-not-allowed disabled:opacity-40 ${variants[variant]} ${className}`}
    />
  );
}
