"use client";

import { Bot, Download, FileBarChart, LoaderCircle, RefreshCw } from "lucide-react";
import { useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import { AuthGate } from "@/components/AuthGate";
import { KpiGrid } from "@/components/KpiGrid";
import { PageHeader } from "@/components/PageHeader";
import { api } from "@/lib/api";
import type { ReviewItem, ReviewStats } from "@/lib/types";

function dateOffset(days: number) {
  const d = new Date();
  d.setDate(d.getDate() - days);
  return d.toISOString().slice(0, 10);
}

function startOfWeek() {
  const d = new Date();
  const day = d.getDay() || 7;
  d.setDate(d.getDate() - day + 1);
  return d.toISOString().slice(0, 10);
}

function startOfMonth() {
  const d = new Date();
  return new Date(d.getFullYear(), d.getMonth(), 1).toISOString().slice(0, 10);
}

function ReportsContent() {
  const [preset, setPreset] = useState("7d");
  const [kind, setKind] = useState("mr");
  const [start, setStart] = useState(dateOffset(7));
  const [end, setEnd] = useState(dateOffset(0));
  const [items, setItems] = useState<ReviewItem[]>([]);
  const [stats, setStats] = useState<ReviewStats | null>(null);
  const [summary, setSummary] = useState("");
  const [loading, setLoading] = useState(false);

  const range = useMemo(() => {
    if (preset === "7d") return { start: dateOffset(7), end: dateOffset(0) };
    if (preset === "30d") return { start: dateOffset(30), end: dateOffset(0) };
    if (preset === "week") return { start: startOfWeek(), end: dateOffset(0) };
    if (preset === "month") return { start: startOfMonth(), end: dateOffset(0) };
    return { start, end };
  }, [preset, start, end]);

  async function load() {
    setLoading(true);
    try {
      const params = new URLSearchParams({
        kind,
        start: range.start,
        end: range.end,
      });
      const data = await api<{ items: ReviewItem[]; stats: ReviewStats }>(
        `/api/reviews?${params}`,
      );
      setItems(data.items);
      setStats(data.stats);
    } finally {
      setLoading(false);
    }
  }

  async function genSummary() {
    setLoading(true);
    try {
      const data = await api<{ content: string | null }>("/api/reports/summary", {
        method: "POST",
        body: JSON.stringify({ kind, start: range.start, end: range.end }),
      });
      setSummary(data.content || "无数据可生成");
    } catch (err) {
      setSummary(err instanceof Error ? err.message : "生成失败");
    } finally {
      setLoading(false);
    }
  }

  function downloadHtml() {
    const rows = items
      .map(
        (r) => `<tr>
          <td>${r.project_name || ""}</td>
          <td>${r.author || ""}</td>
          <td>${r.updated_at || ""}</td>
          <td>${r.score ?? ""}</td>
          <td>${r.delta || ""}</td>
          <td>${(r.commit_messages || "").replace(/</g, "&lt;")}</td>
          <td>${(r.review_result || "").replace(/</g, "&lt;").replace(/\n/g, "<br>")}</td>
        </tr>`,
      )
      .join("");
    const html = `<!DOCTYPE html><html><head><meta charset="utf-8"><title>獬豸审查报告</title>
      <style>body{font-family:sans-serif;background:#050b16;color:#c9d7e8;padding:24px}
      table{width:100%;border-collapse:collapse}th,td{border:1px solid #1e3a5f;padding:8px;vertical-align:top}
      th{background:#0f2138;color:#2de2e6}</style></head><body>
      <h1>獬豸代码审查报告</h1>
      <p>${range.start} ~ ${range.end}</p>
      <table><thead><tr><th>项目</th><th>开发者</th><th>时间</th><th>得分</th><th>变更</th><th>提交</th><th>审查</th></tr></thead>
      <tbody>${rows || "<tr><td colspan=7>暂无数据</td></tr>"}</tbody></table></body></html>`;
    const blob = new Blob([html], { type: "text/html;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `codereview-report-${range.start}_${range.end}.html`;
    a.click();
    URL.revokeObjectURL(url);
  }

  return (
    <div className="space-y-5">
      <PageHeader
        eyebrow="Intelligence reports"
        title="报告中心"
        description="按时间区间聚合审查数据，导出可分享报告，或调用大模型生成团队工作摘要。"
        icon={FileBarChart}
      />
      <div className="panel grid gap-3 rounded-2xl p-4 md:grid-cols-4">
        <label className="field-label">
          快捷区间
          <select
            className="field"
            value={preset}
            onChange={(e) => setPreset(e.target.value)}
          >
            <option value="7d">近 7 天</option>
            <option value="30d">近 30 天</option>
            <option value="week">本周</option>
            <option value="month">本月</option>
            <option value="custom">自定义</option>
          </select>
        </label>
        <label className="field-label">
          数据来源
          <select
            className="field"
            value={kind}
            onChange={(e) => setKind(e.target.value)}
          >
            <option value="mr">合并请求</option>
            <option value="push">代码推送</option>
          </select>
        </label>
        {preset === "custom" ? (
          <>
            <label className="field-label">
              开始
              <input className="field" type="date" value={start} onChange={(e) => setStart(e.target.value)} />
            </label>
            <label className="field-label">
              结束
              <input className="field" type="date" value={end} onChange={(e) => setEnd(e.target.value)} />
            </label>
          </>
        ) : (
          <div className="flex items-end md:col-span-2">
            <div className="w-full rounded-xl border border-white/[0.06] bg-black/15 px-4 py-3 mono text-xs text-cyan-100">
              {range.start} <span className="mx-2 text-[#586c83]">→</span> {range.end}
            </div>
          </div>
        )}
      </div>

      <div className="grid gap-3 sm:grid-cols-3">
        <button type="button" className="tech-button" onClick={load} disabled={loading}>
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          加载数据
        </button>
        <button type="button" className="tech-button tech-button-secondary" onClick={downloadHtml} disabled={!items.length}>
          <Download size={14} />
          下载 HTML 报告
        </button>
        <button type="button" className="tech-button tech-button-secondary" onClick={genSummary} disabled={loading}>
          {loading ? <LoaderCircle size={14} className="animate-spin" /> : <Bot size={14} />}
          生成 LLM 摘要
        </button>
      </div>

      <KpiGrid stats={stats} />

      {summary && (
        <div className="panel rounded-2xl p-5 sm:p-7">
          <div className="mb-1 flex items-center gap-2 text-sm font-semibold text-[#e6effa]">
            <Bot size={15} className="text-cyan-200" />
            AI 团队摘要
          </div>
          <div className="mb-5 text-[10px] uppercase tracking-[0.12em] text-[#566a81]">Generated intelligence</div>
          <div className="markdown-body text-sm">
            <ReactMarkdown>{summary}</ReactMarkdown>
          </div>
        </div>
      )}
    </div>
  );
}

export default function ReportsPage() {
  return (
    <AuthGate>
      <ReportsContent />
    </AuthGate>
  );
}
