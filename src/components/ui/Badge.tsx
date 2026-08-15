type Tone = "accent" | "blue" | "green" | "mauve" | "muted";

interface Props {
  text: string;
  tone?: Tone;
}

const tones: Record<Tone, string> = {
  accent: "bg-mocha-accent/15 text-mocha-accent",
  blue: "bg-mocha-blue/15 text-mocha-blue",
  green: "bg-mocha-green/15 text-mocha-green",
  mauve: "bg-mocha-mauve/15 text-mocha-mauve",
  muted: "bg-mocha-overlay text-mocha-muted",
};

export function Badge({ text, tone = "muted" }: Props) {
  return (
    <span
      className={`inline-flex shrink-0 items-center rounded-full px-2 py-0.5 text-[11px] font-medium ${tones[tone]}`}
    >
      {text}
    </span>
  );
}
