"use client";

import { ExternalLink, FileSearch, ShieldAlert } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import { AuthGate } from "@/components/AuthGate";
import { PageHeader } from "@/components/PageHeader";
import { api } from "@/lib/api";
import type { ReviewItem } from "@/lib/types";

function dateOffset(days: number) {
  const d = new Date();
  d.setDate(d.getDate() - days);
  return d.toISOString().slice(0, 10);
}

function DeepContent() {
  const [kind, setKind] = useState("mr");
  const [start, setStart] = useState(dateOffset(14));
  const [end, setEnd] = useState(dateOffset(0));
  const [onlyLow, setOnlyLow] = useState(false);
  const [items, setItems] = useState<ReviewItem[]>([]);
  const [threshold, setThreshold] = useState(60);
  const [selected, setSelected] = useState(0);

  const load = useCallback(async () => {
    const params = new URLSearchParams({ kind, start, end });
    const data = await api<{ items: ReviewItem[]; stats: { score_threshold: number } }>(
      `/api/reviews?${params}`,
    );
    setItems(data.items);
    setThreshold(data.stats.score_threshold);
    setSelected(0);
  }, [kind, start, end]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void load().catch(() => undefined);
    }, 0);
    return () => window.clearTimeout(timer);
  }, [load]);

  const view = useMemo(() => {
    return onlyLow ? items.filter((x) => Number(x.score || 0) < threshold) : items;
  }, [items, onlyLow, threshold]);

  const safeSelected = Math.min(selected, Math.max(0, view.length - 1));
  const row = view[safeSelected];

  return (
    <div className="space-y-5">
      <PageHeader
        eyebrow="Review explorer"
        title="深度审查"
        description="在统一工作区中浏览 AI 审查详情，聚焦低分记录并回溯关联提交。"
        icon={FileSearch}
      />
      <div className="panel grid gap-3 rounded-2xl p-4 md:grid-cols-4">
        <label className="field-label">
          类型
          <select
            className="field"
            value={kind}
            onChange={(e) => setKind(e.target.value)}
          >
            <option value="mr">合并请求</option>
            <option value="push">代码推送</option>
          </select>
        </label>
        <label className="field-label">
          开始
          <input className="field" type="date" value={start} onChange={(e) => setStart(e.target.value)} />
        </label>
        <label className="field-label">
          结束
          <input className="field" type="date" value={end} onChange={(e) => setEnd(e.target.value)} />
        </label>
        <label className="flex cursor-pointer items-center gap-3 rounded-xl border border-white/[0.06] bg-black/15 px-3 text-xs text-[#9aabbc]">
          <input
            className="accent-rose-300"
            type="checkbox"
            checked={onlyLow}
            onChange={(e) => {
              setOnlyLow(e.target.checked);
              setSelected(0);
            }}
          />
          <ShieldAlert size={14} className="text-amber-200" />
          仅看低分（&lt; {threshold}）
        </label>
      </div>

      <div className="grid gap-4 lg:grid-cols-[minmax(0,320px)_1fr]">
        <div className="panel max-h-[42vh] overflow-auto rounded-2xl p-3 lg:max-h-[72vh]">
          <div className="mb-3 px-1">
            <div className="text-sm font-semibold text-[#e6effa]">审查记录</div>
            <div className="mt-0.5 text-[10px] text-[#61758b]">{view.length} 条可用记录</div>
          </div>
          {view.length === 0 && (
            <div className="py-8 text-center text-sm text-[var(--color-mute)]">暂无记录</div>
          )}
          {view.map((item, idx) => (
            <button
              key={idx}
              type="button"
              onClick={() => setSelected(idx)}
              className={`mb-2 w-full rounded-xl border px-3 py-3 text-left transition ${
                safeSelected === idx
                  ? "border-cyan-300/25 bg-cyan-300/[0.07]"
                  : "border-white/[0.055] bg-black/15 hover:border-cyan-300/15"
              }`}
            >
              <div className="flex items-center justify-between gap-2">
                <div className="truncate text-xs font-semibold text-[#dce7f2]">{item.project_name}</div>
                <span className={`rounded-md px-1.5 py-0.5 mono text-[9px] ${Number(item.score || 0) < threshold ? "bg-rose-300/10 text-rose-200" : "bg-mint/10 text-mint"}`}>{item.score}</span>
              </div>
              <div className="mt-1.5 text-[10px] text-[#687c92]">
                {item.author} · {item.updated_at}
              </div>
            </button>
          ))}
        </div>

        <div className="panel min-h-[420px] rounded-2xl p-4 sm:min-h-[520px] sm:p-7">
          {!row ? (
            <div className="py-16 text-center text-[var(--color-mute)]">选择左侧记录查看 Deep Review</div>
          ) : (
            <>
              <div className="mb-6 grid grid-cols-2 gap-2 sm:grid-cols-4">
                {[
                  ["项目", row.project_name],
                  ["开发者", row.author],
                  ["得分", String(row.score ?? "-")],
                  ["变更", row.delta || "-"],
                ].map(([k, v]) => (
                  <div key={k} className="rounded-xl border border-white/[0.06] bg-black/15 p-3">
                    <div className="text-[10px] text-[#60748a]">{k}</div>
                    <div className="mt-1 truncate mono text-sm text-[#dce8f4]">{v}</div>
                  </div>
                ))}
              </div>
              <div className="mb-3 text-sm text-[var(--color-mute)]">
                提交：{row.commit_messages || "-"}
              </div>
              {row.url && (
                <a href={row.url} target="_blank" rel="noreferrer" className="inline-flex items-center gap-1.5 text-xs text-cyan-300 hover:text-cyan-100">
                  打开 MR/PR
                  <ExternalLink size={12} />
                </a>
              )}
              <div className="mt-5 border-t border-white/[0.06] pt-5">
                <div className="mb-4 text-sm font-semibold text-[#e6effa]">AI 审查报告</div>
                <div className="markdown-body max-w-none text-sm">
                  <ReactMarkdown>{row.review_result || "_（无审查正文）_"}</ReactMarkdown>
                </div>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

export default function DeepPage() {
  return (
    <AuthGate>
      <DeepContent />
    </AuthGate>
  );
}
