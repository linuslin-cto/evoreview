"use client";

import { UserRoundSearch } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { AuthGate } from "@/components/AuthGate";
import { KpiGrid } from "@/components/KpiGrid";
import { PageHeader } from "@/components/PageHeader";
import { StatsCharts } from "@/components/StatsCharts";
import { api } from "@/lib/api";
import type { MemberSummary, ReviewItem, ReviewStats } from "@/lib/types";

function dateOffset(days: number) {
  const d = new Date();
  d.setDate(d.getDate() - days);
  return d.toISOString().slice(0, 10);
}

function MembersContent() {
  const [start, setStart] = useState(dateOffset(30));
  const [end, setEnd] = useState(dateOffset(0));
  const [source, setSource] = useState("mr");
  const [summary, setSummary] = useState<MemberSummary[]>([]);
  const [items, setItems] = useState<ReviewItem[]>([]);
  const [stats, setStats] = useState<ReviewStats | null>(null);
  const [picked, setPicked] = useState("");
  const [threshold, setThreshold] = useState(60);

  const load = useCallback(async () => {
    const params = new URLSearchParams({ source, start, end });
    const data = await api<{
      summary: MemberSummary[];
      items: ReviewItem[];
      stats: ReviewStats;
      score_threshold: number;
    }>(`/api/members?${params}`);
    setSummary(data.summary);
    setItems(data.items);
    setStats(data.stats);
    setThreshold(data.score_threshold);
    setPicked((prev) => prev || data.summary[0]?.author || "");
  }, [source, start, end]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void load().catch((err) => console.error("Failed to load members data:", err));
    }, 0);
    return () => window.clearTimeout(timer);
  }, [load]);

  const memberItems = useMemo(
    () => items.filter((x) => x.author === picked),
    [items, picked],
  );

  return (
    <div className="space-y-5">
      <PageHeader
        eyebrow="Team intelligence"
        title="成员质量洞察"
        description="从审查次数、质量得分、风险记录和代码变更量等维度观察团队工程表现。"
        icon={UserRoundSearch}
      />
      <div className="panel grid gap-3 rounded-2xl p-4 md:grid-cols-4">
        <label className="field-label">
          开始
          <input className="field" type="date" value={start} onChange={(e) => setStart(e.target.value)} />
        </label>
        <label className="field-label">
          结束
          <input className="field" type="date" value={end} onChange={(e) => setEnd(e.target.value)} />
        </label>
        <label className="field-label">
          数据来源
          <select
            className="field"
            value={source}
            onChange={(e) => setSource(e.target.value)}
          >
            <option value="mr">合并请求</option>
            <option value="push">代码推送</option>
            <option value="all">全部</option>
          </select>
        </label>
        <div className="flex items-end">
          <div className="w-full rounded-xl border border-amber-300/10 bg-amber-300/[0.045] px-3 py-3 text-xs text-[#899bad]">
            风险阈值 <span className="ml-1 mono font-semibold text-amber-200">{threshold}</span>
          </div>
        </div>
      </div>

      <KpiGrid stats={stats} />

      <div className="panel overflow-hidden rounded-2xl">
        <div className="border-b border-white/[0.06] px-5 py-4">
          <div className="text-sm font-semibold text-[#e6effa]">成员质量矩阵</div>
          <div className="mt-0.5 text-[10px] text-[#61758b]">点击成员查看详细审查记录</div>
        </div>
        <div className="overflow-auto">
          <table className="data-table w-full min-w-[900px] text-sm">
            <thead className="bg-white/[0.018] text-[10px] uppercase tracking-[0.08em] text-[#64778e]">
              <tr>
                <th className="px-3 py-2 text-left">开发者</th>
                <th className="px-3 py-2 text-left">审查次数</th>
                <th className="px-3 py-2 text-left">平均分</th>
                <th className="px-3 py-2 text-left">最低/最高</th>
                <th className="px-3 py-2 text-left">低分次数</th>
                <th className="px-3 py-2 text-left">新增/删除</th>
                <th className="px-3 py-2 text-left">项目数</th>
              </tr>
            </thead>
            <tbody>
              {summary.map((row) => (
                <tr
                  key={row.author}
                  tabIndex={0}
                  role="button"
                  aria-label={`查看 ${row.author} 的审查记录`}
                  className={`cursor-pointer border-t border-white/[0.045] ${
                    picked === row.author ? "bg-cyan-300/[0.06]" : ""
                  }`}
                  onClick={() => setPicked(row.author)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      setPicked(row.author);
                    }
                  }}
                >
                  <td className="px-3 py-3 mono text-cyan-100">{row.author}</td>
                  <td className="px-3 py-2">{row.review_count}</td>
                  <td className="px-3 py-2">{row.avg_score}</td>
                  <td className="px-3 py-2 mono text-xs">
                    {row.min_score}/{row.max_score}
                  </td>
                  <td className="px-3 py-2 text-rose-300">{row.low_count}</td>
                  <td className="px-3 py-2 mono text-xs">
                    +{row.additions}/-{row.deletions}
                  </td>
                  <td className="px-3 py-2">{row.project_count}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      {picked && (
        <div className="panel rounded-2xl p-4 sm:p-5">
          <div className="mb-4">
            <div className="text-sm font-semibold text-[#e6effa]">{picked} 的审查动态</div>
            <div className="mt-0.5 text-[10px] text-[#61758b]">Member review activity</div>
          </div>
          <div className="max-h-72 space-y-2 overflow-auto">
            {memberItems.map((item, i) => (
              <div key={i} className="rounded-xl border border-white/[0.055] bg-black/15 px-3 py-3 text-sm">
                <div className="flex flex-wrap gap-3 text-xs text-[var(--color-mute)]">
                  <span>{item.updated_at}</span>
                  <span>{item.project_name}</span>
                  <span className="text-cyan-200">score {item.score}</span>
                </div>
                <div className="mt-1">{item.commit_messages}</div>
              </div>
            ))}
          </div>
        </div>
      )}

      <StatsCharts stats={stats} />
    </div>
  );
}

export default function MembersPage() {
  return (
    <AuthGate>
      <MembersContent />
    </AuthGate>
  );
}
