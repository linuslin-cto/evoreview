"use client";

import {
  ArrowUpRight,
  ChevronLeft,
  ChevronRight,
  Filter,
  GitMerge,
  RefreshCw,
  Search,
  SlidersHorizontal,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import { AuthGate } from "@/components/AuthGate";
import { DetailDrawer } from "@/components/DetailDrawer";
import { KpiGrid } from "@/components/KpiGrid";
import { MonitorFocusPanel, MonitorFocusTrigger } from "@/components/MonitorFocusPanel";
import { PageHeader } from "@/components/PageHeader";
import { StatsCharts } from "@/components/StatsCharts";
import { api } from "@/lib/api";
import type { ReviewItem, ReviewStats } from "@/lib/types";

const PAGE_SIZE = 10;

function dateOffset(days: number) {
  const d = new Date();
  d.setDate(d.getDate() - days);
  return d.toISOString().slice(0, 10);
}

function ReviewsContent() {
  const [kind, setKind] = useState<"mr" | "push">("mr");
  const [start, setStart] = useState(dateOffset(7));
  const [end, setEnd] = useState(dateOffset(0));
  const [q, setQ] = useState("");
  const [debouncedQ, setDebouncedQ] = useState("");
  const [minScore, setMinScore] = useState(0);
  const [maxScore, setMaxScore] = useState(100);
  const [authors, setAuthors] = useState<string[]>([]);
  const [projects, setProjects] = useState<string[]>([]);
  const [authorOptions, setAuthorOptions] = useState<string[]>([]);
  const [projectOptions, setProjectOptions] = useState<string[]>([]);
  const [items, setItems] = useState<ReviewItem[]>([]);
  const [stats, setStats] = useState<ReviewStats | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [page, setPage] = useState(1);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [selectedReview, setSelectedReview] = useState<ReviewItem | null>(null);
  const [monitorOpen, setMonitorOpen] = useState(false);

  const loadFilters = useCallback(async () => {
    const data = await api<{ authors: string[]; projects: string[] }>(
      `/api/filters?kind=${kind}&start=${start}&end=${end}`,
    );
    setAuthorOptions(data.authors);
    setProjectOptions(data.projects);
  }, [kind, start, end]);

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const params = new URLSearchParams({
        kind,
        start,
        end,
        min_score: String(minScore),
        max_score: String(maxScore),
      });
      if (debouncedQ.trim()) params.set("q", debouncedQ.trim());
      if (authors.length) params.set("authors", authors.join(","));
      if (projects.length) params.set("projects", projects.join(","));
      const data = await api<{ items: ReviewItem[]; stats: ReviewStats }>(
        `/api/reviews?${params}`,
      );
      setItems(data.items);
      setStats(data.stats);
      setPage(1);
    } catch (err) {
      setError(err instanceof Error ? err.message : "加载失败");
    } finally {
      setLoading(false);
    }
  }, [kind, start, end, debouncedQ, minScore, maxScore, authors, projects]);

  useEffect(() => {
    const timer = window.setTimeout(() => setDebouncedQ(q), 350);
    return () => window.clearTimeout(timer);
  }, [q]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void loadFilters().catch(() => undefined);
    }, 0);
    return () => window.clearTimeout(timer);
  }, [loadFilters]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void load().catch(() => undefined);
    }, 0);
    return () => window.clearTimeout(timer);
  }, [load]);

  const totalPages = Math.max(1, Math.ceil(items.length / PAGE_SIZE));
  const tableRows = useMemo(
    () => items.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE),
    [items, page],
  );
  const activeFilterCount =
    authors.length +
    projects.length +
    Number(minScore > 0) +
    Number(maxScore < 100);

  const renderReviewRow = (row: ReviewItem, idx: number) => {
    const lowScore = Number(row.score || 0) < Number(stats?.score_threshold || 60);
    return (
      <tr
        key={`${row.project_name}-${row.updated_at}-${idx}`}
        className="cursor-pointer border-t border-white/[0.045]"
        onClick={() => setSelectedReview(row)}
      >
        <td className="px-4 py-3.5">
          <div className="font-medium text-[#e1ebf6]">{row.project_name || "-"}</div>
          <div className="mt-1 mono text-[10px] text-[#60748a]">
            {row.source_branch || row.branch || "unknown"}
            {row.target_branch ? ` → ${row.target_branch}` : ""}
          </div>
        </td>
        <td className="px-4 py-3.5">
          <span className="rounded-lg bg-cyan-300/[0.07] px-2 py-1 mono text-[10px] text-cyan-100">
            {row.author || "unknown"}
          </span>
        </td>
        <td className="max-w-[310px] px-4 py-3.5">
          <div className="truncate text-xs text-[#93a4b7]">{row.commit_messages || "无提交信息"}</div>
        </td>
        <td className="px-4 py-3.5 mono text-[10px] text-[#8ca0b5]">{row.delta || "-"}</td>
        <td className="px-4 py-3.5">
          <div className="flex items-center gap-2">
            <span className={`mono text-xs font-semibold ${lowScore ? "text-rose-200" : "text-mint"}`}>
              {row.score ?? "-"}
            </span>
            <div
              className="h-1 w-14 overflow-hidden rounded bg-white/[0.06]"
              role="progressbar"
              aria-label={`代码质量得分 ${row.score ?? 0}`}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={Number(row.score || 0)}
            >
              <div
                className={`h-full rounded ${lowScore ? "bg-rose-300" : "bg-mint"}`}
                style={{ width: `${Math.min(100, Number(row.score || 0))}%` }}
              />
            </div>
          </div>
        </td>
        <td className="px-4 py-3.5 mono text-[10px] text-[#687c92]">{row.updated_at}</td>
        <td className="px-4 py-3.5 text-right">
          {row.url ? (
            <a
              href={row.url}
              target="_blank"
              rel="noreferrer"
              aria-label="打开 MR/PR"
              className="inline-flex rounded-lg border border-white/[0.06] p-2 text-[#71849a] transition hover:border-cyan-300/25 hover:text-cyan-200"
              onClick={(event) => event.stopPropagation()}
            >
              <ArrowUpRight size={14} />
            </a>
          ) : (
            "-"
          )}
        </td>
      </tr>
    );
  };

  const renderReviewCard = (row: ReviewItem, idx: number) => {
    const lowScore = Number(row.score || 0) < Number(stats?.score_threshold || 60);
    return (
      <button
        key={`card-${row.project_name}-${row.updated_at}-${idx}`}
        type="button"
        className="w-full rounded-xl border border-white/[0.06] bg-black/15 p-4 text-left transition hover:border-cyan-300/20 hover:bg-cyan-300/[0.03]"
        onClick={() => setSelectedReview(row)}
      >
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="truncate text-sm font-semibold text-[#e6effa]">{row.project_name || "-"}</div>
            <div className="mt-1 mono text-[10px] text-[#60748a]">
              {row.source_branch || row.branch || "unknown"}
              {row.target_branch ? ` → ${row.target_branch}` : ""}
            </div>
          </div>
          <span className={`rounded-md px-2 py-1 mono text-[10px] ${lowScore ? "bg-rose-300/10 text-rose-200" : "bg-mint/10 text-mint"}`}>
            {row.score ?? "-"}
          </span>
        </div>
        <div className="mt-3 flex flex-wrap items-center gap-2 text-[10px] text-[#71849a]">
          <span className="rounded-md bg-cyan-300/[0.07] px-2 py-1 text-cyan-100">{row.author || "unknown"}</span>
          <span>{row.delta || "-"}</span>
          <span>{row.updated_at}</span>
        </div>
        <p className="mt-3 line-clamp-2 text-xs leading-5 text-[#93a4b7]">
          {row.commit_messages || "无提交信息"}
        </p>
      </button>
    );
  };

  return (
    <div className="space-y-5">
      <PageHeader
        eyebrow="Review operations"
        title="代码审查总览"
        description="聚合合并请求与代码推送，快速定位质量波动、低分风险与团队审查趋势。"
        icon={GitMerge}
        action={
          <button type="button" className="tech-button" onClick={() => load()} disabled={loading}>
            <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
            {loading ? "正在同步" : "同步数据"}
          </button>
        }
      />

      <div className="panel rounded-2xl p-3 sm:p-4">
        <div className="flex flex-col gap-3 xl:flex-row xl:items-center">
          <div className="flex shrink-0 gap-1 rounded-xl border border-white/[0.06] bg-black/20 p-1">
          {(
            [
              ["mr", "合并请求"],
              ["push", "代码推送"],
            ] as const
          ).map(([id, label]) => (
            <button
              key={id}
              type="button"
              aria-pressed={kind === id}
              onClick={() => {
                setKind(id);
                setAuthors([]);
                setProjects([]);
              }}
              className={`rounded-lg px-4 py-2 text-xs font-semibold transition ${
                kind === id
                  ? "bg-cyan-300/10 text-cyan-100 shadow-sm"
                  : "text-[#6e8197] hover:text-[#c5d1df]"
              }`}
            >
              {label}
            </button>
          ))}
          </div>

          <div className="relative min-w-0 flex-1">
            <Search size={15} className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-[#5e7288]" />
            <input
              className="field pl-10"
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="搜索项目、开发者、提交信息或审查内容..."
            />
          </div>

          <div className="grid grid-cols-2 gap-2 sm:flex">
            <label className="relative">
              <span className="sr-only">开始日期</span>
              <input className="field min-w-0 sm:w-[142px]" type="date" value={start} onChange={(e) => setStart(e.target.value)} />
            </label>
            <label className="relative">
              <span className="sr-only">结束日期</span>
              <input className="field min-w-0 sm:w-[142px]" type="date" value={end} onChange={(e) => setEnd(e.target.value)} />
            </label>
            <button
              type="button"
              className={`tech-button tech-button-secondary col-span-2 ${filtersOpen ? "border-cyan-300/35 text-cyan-100" : ""}`}
              onClick={() => setFiltersOpen((value) => !value)}
            >
              <SlidersHorizontal size={14} />
              高级筛选
              {activeFilterCount > 0 && (
                <span className="rounded-full bg-cyan-300 px-1.5 py-0.5 mono text-[9px] text-[#061019]">{activeFilterCount}</span>
              )}
            </button>
          </div>
        </div>

        {filtersOpen && (
          <div className="mt-4 grid gap-3 border-t border-white/[0.06] pt-4 sm:grid-cols-2 lg:grid-cols-4">
            <label className="field-label">
              开发者
              <select
                className="field"
                value={authors[0] || ""}
                onChange={(e) => setAuthors(e.target.value ? [e.target.value] : [])}
              >
                <option value="">全部开发者</option>
                {authorOptions.map((author) => <option key={author} value={author}>{author}</option>)}
              </select>
            </label>
            <label className="field-label">
              项目
              <select
                className="field"
                value={projects[0] || ""}
                onChange={(e) => setProjects(e.target.value ? [e.target.value] : [])}
              >
                <option value="">全部项目</option>
                {projectOptions.map((project) => <option key={project} value={project}>{project}</option>)}
              </select>
            </label>
            <label className="field-label">
              最低分
              <input className="field" type="number" min={0} max={100} value={minScore} onChange={(e) => setMinScore(Number(e.target.value || 0))} />
            </label>
            <label className="field-label">
              最高分
              <input className="field" type="number" min={0} max={100} value={maxScore} onChange={(e) => setMaxScore(Number(e.target.value || 100))} />
            </label>
          </div>
        )}

        {error && (
          <div className="mt-3 rounded-lg border border-rose-300/15 bg-rose-300/[0.06] px-3 py-2 text-xs text-rose-200">
            {error}
          </div>
        )}

        {activeFilterCount > 0 && (
          <div className="mt-3 flex items-center gap-2 text-[10px] text-[#687c92]">
            <Filter size={12} />
            已应用 {activeFilterCount} 个筛选条件
            <button
              type="button"
              className="ml-1 text-cyan-300 hover:text-cyan-100"
              onClick={() => {
                setAuthors([]);
                setProjects([]);
                setMinScore(0);
                setMaxScore(100);
              }}
            >
              清除
            </button>
          </div>
        )}
      </div>

      <div className="panel overflow-hidden rounded-2xl">
        <div className="flex items-center justify-between border-b border-white/[0.06] px-4 py-3.5 sm:px-5">
          <div>
            <div className="text-sm font-semibold text-[#e6effa]">审查记录</div>
            <div className="mt-0.5 text-[10px] text-[#61758b]">共 {items.length} 条结果，点击记录查看详情</div>
          </div>
          <div className="mono text-[10px] text-[#61758b]">PAGE {page}/{totalPages}</div>
        </div>
        <div className="overflow-x-auto review-table-wrap">
          <table className="data-table w-full min-w-[920px] text-left text-sm">
            <thead className="bg-white/[0.018] text-[10px] uppercase tracking-[0.08em] text-[#64778e]">
              <tr>
                <th className="px-4 py-3 font-semibold">项目 / 分支</th>
                <th className="px-4 py-3 font-semibold">开发者</th>
                <th className="px-4 py-3 font-semibold">提交摘要</th>
                <th className="px-4 py-3 font-semibold">变更</th>
                <th className="px-4 py-3 font-semibold">质量得分</th>
                <th className="px-4 py-3 font-semibold">更新时间</th>
                <th className="px-4 py-3" />
              </tr>
            </thead>
            <tbody>
              {tableRows.length === 0 ? (
                <tr>
                  <td colSpan={7} className="px-4 py-16 text-center">
                    <GitMerge size={22} className="mx-auto mb-2 text-[#465a70]" />
                    <div className="text-sm text-[#71849a]">当前筛选条件下暂无审查记录</div>
                  </td>
                </tr>
              ) : (
                tableRows.map((row, idx) => renderReviewRow(row, idx))
              )}
            </tbody>
          </table>
        </div>
        <div className="review-card-list p-4">
          {tableRows.length === 0 ? (
            <div className="py-12 text-center">
              <GitMerge size={22} className="mx-auto mb-2 text-[#465a70]" />
              <div className="text-sm text-[#71849a]">当前筛选条件下暂无审查记录</div>
            </div>
          ) : (
            tableRows.map((row, idx) => renderReviewCard(row, idx))
          )}
        </div>
        {items.length > PAGE_SIZE && (
          <div className="flex items-center justify-between border-t border-white/[0.06] px-4 py-3">
            <span className="text-[10px] text-[#60748a]">
              {(page - 1) * PAGE_SIZE + 1}–{Math.min(page * PAGE_SIZE, items.length)} / {items.length}
            </span>
            <div className="flex gap-1">
              <button type="button" aria-label="上一页" className="rounded-lg border border-white/[0.06] p-2 text-[#71849a] disabled:opacity-30" disabled={page === 1} onClick={() => setPage((value) => Math.max(1, value - 1))}>
                <ChevronLeft size={14} />
              </button>
              <button type="button" aria-label="下一页" className="rounded-lg border border-white/[0.06] p-2 text-[#71849a] disabled:opacity-30" disabled={page === totalPages} onClick={() => setPage((value) => Math.min(totalPages, value + 1))}>
                <ChevronRight size={14} />
              </button>
            </div>
          </div>
        )}
      </div>

      <section className="space-y-3">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <div className="text-sm font-semibold text-[#e6effa]">数据洞察</div>
            <div className="mt-0.5 text-[10px] text-[#61758b]">
              默认嵌入页面展示；需要聚焦时可进入窗口化大屏，不会遮挡侧栏导航
            </div>
          </div>
          <MonitorFocusTrigger onClick={() => setMonitorOpen(true)} disabled={!stats} />
        </div>
        <KpiGrid stats={stats} />
        <StatsCharts stats={stats} />
      </section>

      <MonitorFocusPanel open={monitorOpen} onClose={() => setMonitorOpen(false)} stats={stats} />

      <DetailDrawer
        open={Boolean(selectedReview)}
        onClose={() => setSelectedReview(null)}
        title={selectedReview?.project_name || "审查详情"}
        subtitle={
          selectedReview
            ? `${selectedReview.author} · ${selectedReview.updated_at}`
            : undefined
        }
      >
        {selectedReview && (
          <>
            <div className="mb-5 grid grid-cols-1 gap-2 min-[420px]:grid-cols-3">
              <div className="rounded-xl border border-white/[0.06] bg-white/[0.025] p-3">
                <div className="text-[10px] text-[#60748a]">得分</div>
                <div className="mt-1 mono text-lg text-cyan-100">{selectedReview.score ?? "-"}</div>
              </div>
              <div className="rounded-xl border border-white/[0.06] bg-white/[0.025] p-3">
                <div className="text-[10px] text-[#60748a]">变更</div>
                <div className="mt-1 mono text-sm text-[#d8e2ef]">{selectedReview.delta || "-"}</div>
              </div>
              <div className="rounded-xl border border-white/[0.06] bg-white/[0.025] p-3">
                <div className="text-[10px] text-[#60748a]">类型</div>
                <div className="mt-1 text-sm text-[#d8e2ef]">{kind === "mr" ? "合并请求" : "代码推送"}</div>
              </div>
            </div>
            <div className="mb-5 rounded-xl border border-white/[0.06] bg-black/20 p-4">
              <div className="mb-2 text-[10px] uppercase tracking-wider text-[#60748a]">Commit</div>
              <p className="text-sm leading-6 text-[#acbac9]">{selectedReview.commit_messages || "无提交信息"}</p>
            </div>
            <div className="markdown-body">
              <ReactMarkdown>{selectedReview.review_result || "_（无审查正文）_"}</ReactMarkdown>
            </div>
          </>
        )}
      </DetailDrawer>
    </div>
  );
}

export default function ReviewsPage() {
  return (
    <AuthGate>
      <ReviewsContent />
    </AuthGate>
  );
}
