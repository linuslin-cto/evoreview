"use client";

import { Maximize2, X } from "lucide-react";
import { useEffect, useId } from "react";
import { createPortal } from "react-dom";
import { KpiGrid } from "@/components/KpiGrid";
import { StatsCharts } from "@/components/StatsCharts";
import type { ReviewStats } from "@/lib/types";

type MonitorFocusPanelProps = {
  open: boolean;
  onClose: () => void;
  stats: ReviewStats | null;
};

export function MonitorFocusPanel({ open, onClose, stats }: MonitorFocusPanelProps) {
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
    <div className="monitor-scrim" onClick={onClose} role="presentation">
      <section
        className="monitor-window"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onClick={(event) => event.stopPropagation()}
      >
        <header className="monitor-window-header">
          <div className="flex min-w-0 items-center gap-3">
            <div className="rounded-lg bg-cyan-300/10 p-2 text-cyan-200">
              <Maximize2 size={16} />
            </div>
            <div className="min-w-0">
              <h2 id={titleId} className="truncate text-sm font-semibold text-[#edf6ff] sm:text-base">
                数据洞察大屏
              </h2>
              <p className="mt-0.5 text-[10px] text-[#61758b]">
                窗口化展示，保留导航上下文 · 按 Esc 或点击遮罩退出
              </p>
            </div>
          </div>
          <button
            type="button"
            className="tech-button tech-button-secondary shrink-0"
            onClick={onClose}
          >
            <X size={14} />
            退出大屏
          </button>
        </header>
        <div className="monitor-window-body">
          <KpiGrid stats={stats} />
          <StatsCharts stats={stats} dense />
        </div>
      </section>
    </div>,
    document.body,
  );
}

export function MonitorFocusTrigger({
  onClick,
  disabled,
}: {
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      className="tech-button tech-button-secondary w-full sm:w-auto"
      onClick={onClick}
      disabled={disabled}
    >
      <Maximize2 size={14} />
      展开数据大屏
    </button>
  );
}
