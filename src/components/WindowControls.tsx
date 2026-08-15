import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * 全平台一致的 ─ □ ✕ 窗口按钮组。配合 tauri.conf.json `decorations: false`。
 * Win11 视觉规范:46px 宽,关闭键 hover 红、其余 hover 摩卡 overlay。
 */
export function WindowControls() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const win = getCurrentWindow();
        setMaximized(await win.isMaximized());
        unlisten = await win.onResized(async () => {
          try {
            setMaximized(await win.isMaximized());
          } catch {
            /* 窗口销毁中,忽略 */
          }
        });
      } catch {
        /* 非 Tauri 环境(浏览器开发),静默 */
      }
    })();
    return () => {
      unlisten?.();
    };
  }, []);

  // getCurrentWindow() 在非 Tauri 环境会抛,统一容错。
  const safeWindow = () => {
    try {
      return getCurrentWindow();
    } catch {
      return null;
    }
  };
  const handleMinimize = () => safeWindow()?.minimize().catch(() => {});
  const handleMaximize = () => safeWindow()?.toggleMaximize().catch(() => {});
  const handleClose = () => safeWindow()?.close().catch(() => {});

  return (
    <div className="flex h-full items-stretch">
      <ControlButton onClick={handleMinimize} aria-label="最小化" title="最小化">
        <MinimizeIcon />
      </ControlButton>
      <ControlButton
        onClick={handleMaximize}
        aria-label={maximized ? "还原" : "最大化"}
        title={maximized ? "还原" : "最大化"}
      >
        {maximized ? <RestoreIcon /> : <MaximizeIcon />}
      </ControlButton>
      <ControlButton
        onClick={handleClose}
        aria-label="关闭"
        title="关闭"
        variant="close"
      >
        <CloseIcon />
      </ControlButton>
    </div>
  );
}

interface ControlButtonProps {
  onClick: () => void;
  "aria-label": string;
  title: string;
  variant?: "default" | "close";
  children: React.ReactNode;
}

function ControlButton({
  onClick,
  variant = "default",
  children,
  ...rest
}: ControlButtonProps) {
  const base =
    "grid w-[46px] place-items-center text-mocha-subtext transition-colors duration-100";
  const hover =
    variant === "close"
      ? "hover:bg-[oklch(55%_0.21_25)] hover:text-white"
      : "hover:bg-mocha-overlay hover:text-mocha-text";
  return (
    <button
      type="button"
      onClick={onClick}
      className={`${base} ${hover}`}
      {...rest}
    >
      {children}
    </button>
  );
}

/* ── inline SVG glyphs ── Win11/Fluent 风格 10×10 viewBox,1px stroke ── */

function MinimizeIcon() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden>
      <path d="M0 5 H10" stroke="currentColor" strokeWidth="1" fill="none" />
    </svg>
  );
}

function MaximizeIcon() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden>
      <rect
        x="0.5"
        y="0.5"
        width="9"
        height="9"
        fill="none"
        stroke="currentColor"
        strokeWidth="1"
      />
    </svg>
  );
}

function RestoreIcon() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden>
      <rect
        x="0.5"
        y="2.5"
        width="7"
        height="7"
        fill="none"
        stroke="currentColor"
        strokeWidth="1"
      />
      <path
        d="M2.5 0.5 H9.5 V7.5 H7.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1"
      />
    </svg>
  );
}

function CloseIcon() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden>
      <path
        d="M0 0 L10 10 M10 0 L0 10"
        stroke="currentColor"
        strokeWidth="1"
        fill="none"
      />
    </svg>
  );
}
