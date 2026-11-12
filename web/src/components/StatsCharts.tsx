"use client";

import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { ReviewStats } from "@/lib/types";

const tooltipStyle = {
  background: "#0b1524",
  border: "1px solid rgba(133,169,205,0.18)",
  borderRadius: 10,
  color: "#d8e2ef",
  fontSize: 12,
  boxShadow: "0 18px 40px rgba(0,0,0,.28)",
};

function ChartCard({
  title,
  children,
  dense,
}: {
  title: string;
  children: React.ReactNode;
  dense?: boolean;
}) {
  return (
    <div className="panel panel-hover rounded-2xl p-4 sm:p-5">
      <div className="mb-1 text-sm font-semibold text-[#e8f1fb]">{title}</div>
      <div className="mb-4 text-[10px] uppercase tracking-[0.12em] text-[#566a81]">Live analytics</div>
      <div className={dense ? "h-64 sm:h-72" : "h-52 sm:h-56 md:h-60"}>{children}</div>
    </div>
  );
}

export function StatsCharts({
  stats,
  dense = false,
}: {
  stats: ReviewStats | null;
  dense?: boolean;
}) {
  if (!stats) {
    return (
      <div className="panel rounded-2xl p-8 text-center text-sm text-[var(--color-mute)]">
        暂无统计数据
      </div>
    );
  }

  const scoreColors = ["#ff6b88", "#f5b942", "#4da3ff", "#50d8a0"];
  const axisTick = { fill: "#6f8298", fontSize: 10 };
  const grid = "rgba(133,169,205,0.09)";

  return (
    <div className={`grid gap-3 ${dense ? "xl:grid-cols-2" : "lg:grid-cols-2"}`}>
      <div className={dense ? "xl:col-span-2" : "lg:col-span-2"}>
        <ChartCard title="审查趋势" dense={dense}>
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={stats.daily_trend}>
              <defs>
                <linearGradient id="countGradient" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor="#5ee7f2" stopOpacity={0.3} />
                  <stop offset="100%" stopColor="#5ee7f2" stopOpacity={0.01} />
                </linearGradient>
              </defs>
              <CartesianGrid stroke={grid} vertical={false} />
              <XAxis dataKey="date" tick={axisTick} tickLine={false} axisLine={false} />
              <YAxis allowDecimals={false} tick={axisTick} tickLine={false} axisLine={false} width={28} />
              <Tooltip contentStyle={tooltipStyle} cursor={{ stroke: "rgba(94,231,242,.18)" }} />
              <Area
                type="monotone"
                dataKey="count"
                name="审查次数"
                stroke="#5ee7f2"
                strokeWidth={2}
                fill="url(#countGradient)"
                activeDot={{ r: 4, fill: "#5ee7f2", stroke: "#0b1524", strokeWidth: 2 }}
              />
            </AreaChart>
          </ResponsiveContainer>
        </ChartCard>
      </div>

      <ChartCard title="项目审查量" dense={dense}>
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={stats.by_project_count.slice(0, 8)} layout="vertical" margin={{ left: 4 }}>
            <CartesianGrid stroke={grid} horizontal={false} />
            <XAxis type="number" allowDecimals={false} tick={axisTick} tickLine={false} axisLine={false} />
            <YAxis dataKey="name" type="category" width={82} tick={axisTick} tickLine={false} axisLine={false} />
            <Tooltip contentStyle={tooltipStyle} />
            <Bar dataKey="value" name="审查次数" fill="#5ee7f2" radius={[0, 5, 5, 0]} barSize={12} />
          </BarChart>
        </ResponsiveContainer>
      </ChartCard>

      <ChartCard title="得分区间分布" dense={dense}>
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={stats.score_distribution}>
            <CartesianGrid stroke={grid} vertical={false} />
            <XAxis dataKey="name" tick={axisTick} tickLine={false} axisLine={false} />
            <YAxis allowDecimals={false} tick={axisTick} tickLine={false} axisLine={false} width={28} />
            <Tooltip contentStyle={tooltipStyle} />
            <Bar dataKey="value" name="记录数" radius={[5, 5, 0, 0]} barSize={34}>
              {stats.score_distribution.map((_, i) => (
                <Cell key={i} fill={scoreColors[i % scoreColors.length]} />
              ))}
            </Bar>
          </BarChart>
        </ResponsiveContainer>
      </ChartCard>

      <ChartCard title="开发者平均得分" dense={dense}>
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={stats.by_author_score.slice(0, 8)}>
            <CartesianGrid stroke={grid} vertical={false} />
            <XAxis dataKey="name" tick={axisTick} tickLine={false} axisLine={false} />
            <YAxis domain={[0, 100]} tick={axisTick} tickLine={false} axisLine={false} width={28} />
            <Tooltip contentStyle={tooltipStyle} />
            <Bar dataKey="value" name="平均分" fill="#4da3ff" radius={[5, 5, 0, 0]} barSize={22} />
          </BarChart>
        </ResponsiveContainer>
      </ChartCard>

      <div className="panel panel-hover rounded-2xl p-4 sm:p-5">
        <div className="mb-1 text-sm font-semibold text-[#e8f1fb]">风险关注成员</div>
        <div className="mb-4 text-[10px] uppercase tracking-[0.12em] text-[#566a81]">Below threshold</div>
        <div className="space-y-2">
          {stats.low_authors.length === 0 ? (
            <div className="flex h-48 items-center justify-center rounded-xl border border-dashed border-white/[0.08] text-xs text-[#64778e]">
              当前区间无低分记录
            </div>
          ) : (
            stats.low_authors.slice(0, 6).map((item, index) => (
              <div key={item.name} className="flex items-center gap-3 rounded-xl border border-white/[0.05] bg-white/[0.02] px-3 py-2.5">
                <span className="mono w-5 text-[10px] text-[#53677d]">{String(index + 1).padStart(2, "0")}</span>
                <span className="min-w-0 flex-1 truncate text-xs font-medium text-[#dce6f1]">{item.name}</span>
                <span className="rounded-md bg-rose-300/10 px-2 py-1 mono text-[10px] text-rose-200">{item.count} 次</span>
                <span className="mono text-[10px] text-[#75889e]">AVG {item.avg}</span>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
