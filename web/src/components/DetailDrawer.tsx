"use client";

import { X } from "lucide-react";
import { useEffect, useId } from "react";
import { createPortal } from "react-dom";

type DetailDrawerProps = {
  open: boolean;
  onClose: () => void;
  title: string;
  subtitle?: string;
  children: React.ReactNode;
};

export function DetailDrawer({
  open,
  onClose,
  title,
  subtitle,
  children,
}: DetailDrawerProps) {
  const titleId = useId();

  useEffect(() => {
    if (!open) return undefined;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    document.body.classList.add("shell-scroll-lock");
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.classList.remove("shell-scroll-lock");
    };
  }, [open, onClose]);

  if (!open || typeof document === "undefined") return null;

  return createPortal(
    <div className="detail-scrim" onClick={onClose} role="presentation">
      <aside
        className="detail-panel"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onClick={(event) => event.stopPropagation()}
      >
        <div className="detail-panel-handle lg:hidden" aria-hidden />
        <header className="detail-panel-header">
          <div className="min-w-0 flex-1">
            <div className="text-[10px] uppercase tracking-[0.14em] text-cyan-300/70">
              Review detail
            </div>
            <h3 id={titleId} className="mt-1 truncate text-lg font-semibold text-[#edf6ff] sm:text-xl">
              {title}
            </h3>
            {subtitle && (
              <p className="mt-1 truncate text-xs text-[#71849a]">{subtitle}</p>
            )}
          </div>
          <button
            type="button"
            aria-label="关闭详情"
            className="shrink-0 rounded-lg border border-white/[0.06] p-2 text-[#71849a] transition hover:border-rose-300/20 hover:text-rose-200"
            onClick={onClose}
          >
            <X size={16} />
          </button>
        </header>
        <div className="detail-panel-body">{children}</div>
      </aside>
    </div>,
    document.body,
  );
}
