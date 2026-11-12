"use client";

import {
  AlertTriangle,
  Braces,
  FolderGit2,
  Gauge,
  Users,
  type LucideIcon,
} from "lucide-react";
import type { ReviewStats } from "@/lib/types";

const CARDS: Array<{
  key: keyof ReviewStats | "delta";
  label: string;
  icon: LucideIcon;
  tone: string;
  hint: (s: ReviewStats) => string;
  value: (s: ReviewStats) => string;
}> = [
  {
    key: "total",
    label: "审查次数",
    icon: FolderGit2,
    tone: "text-cyan-200 bg-cyan-300/10",
    value: (s) => String(s.total),
    hint: (s) => `${s.projects} 个项目`,
  },
  {
    key: "avg_score",
    label: "平均得分",
    icon: Gauge,
    tone: "text-blue-200 bg-blue-300/10",
    value: (s) => s.avg_score.toFixed(1),
    hint: (s) => `阈值 ${s.score_threshold}`,
  },
  {
    key: "low_score_count",
    label: "低分预警",
    icon: AlertTriangle,
    tone: "text-amber-200 bg-amber-300/10",
    value: (s) => String(s.low_score_count),
    hint: (s) => `低于 ${s.score_threshold} 分`,
  },
  {
    key: "authors",
    label: "参与开发者",
    icon: Users,
    tone: "text-mint bg-mint/10",
    value: (s) => String(s.authors),
    hint: () => "去重人数",
  },
  {
    key: "delta",
    label: "代码变更",
    icon: Braces,
    tone: "text-rose-200 bg-rose-300/10",
    value: (s) => `+${s.additions}/-${s.deletions}`,
    hint: () => "新增 / 删除",
  },
];

const defaultStats: ReviewStats = {
  total: 0,
  avg_score: 0,
  low_score_count: 0,
  authors: 0,
  projects: 0,
  additions: 0,
  deletions: 0,
  score_threshold: 60,
  by_project_count: [],
  by_project_score: [],
  by_author_count: [],
  by_author_score: [],
  by_author_lines: [],
  by_project_lines: [],
  score_distribution: [],
  daily_trend: [],
  low_authors: [],
};

export function KpiGrid({ stats }: { stats: ReviewStats | null }) {
  const s: ReviewStats = stats ?? defaultStats;

  return (
    <div className="grid grid-cols-1 gap-3 min-[480px]:grid-cols-2 xl:grid-cols-5">
      {CARDS.map((card) => (
        <div key={card.label} className="panel panel-hover relative overflow-hidden rounded-2xl p-4 sm:p-5">
          <div className="absolute -top-10 -right-8 h-24 w-24 rounded-full bg-cyan-300/[0.035] blur-2xl" />
          <div className="relative flex items-start justify-between">
            <div className="text-[11px] font-semibold tracking-wide text-[var(--color-mute)]">
              {card.label}
            </div>
            <div className={`rounded-lg p-2 ${card.tone}`}>
              <card.icon size={15} strokeWidth={1.8} />
            </div>
          </div>
          <div className="mono relative mt-3 text-xl font-semibold tracking-tight text-[#edf8ff] sm:text-2xl">
            {card.value(s)}
          </div>
          <div className="relative mt-1 text-[10px] text-[#61758b]">
            {card.hint(s)}
          </div>
        </div>
      ))}
    </div>
  );
}
