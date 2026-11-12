"use client";

import { BellRing, Bot, Cpu, GitBranch, ServerCog } from "lucide-react";
import { useEffect, useState } from "react";
import { AuthGate } from "@/components/AuthGate";
import { PageHeader } from "@/components/PageHeader";
import { api } from "@/lib/api";
import type { SystemInfo } from "@/lib/types";

function SystemContent() {
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    api<SystemInfo>("/api/system")
      .then(setInfo)
      .catch((err) => setError(err instanceof Error ? err.message : "加载失败"));
  }, []);

  if (error) {
    return <div className="panel rounded-2xl p-6 text-rose-300">{error}</div>;
  }
  if (!info) {
    return (
      <div className="panel rounded-2xl p-6 mono text-cyan-300">SCANNING SYSTEM…</div>
    );
  }

  const metrics = [
    { label: "LLM 供应商", value: info.llm_provider, icon: Bot },
    { label: "审查策略", value: info.review_strategy, icon: Cpu },
    { label: "审查风格", value: info.review_style, icon: GitBranch },
    { label: "Push Review", value: info.push_review_enabled ? "开启" : "关闭", icon: BellRing },
  ];

  const rows = [
    ["GitLab Token", info.tokens.gitlab],
    ["GitHub Token", info.tokens.github],
    ["Gitea Token", info.tokens.gitea],
    ["钉钉", info.notify.dingtalk ? "开启" : "关闭"],
    ["企业微信", info.notify.wecom ? "开启" : "关闭"],
    ["飞书", info.notify.feishu ? "开启" : "关闭"],
    ["自定义 Webhook", info.notify.extra_webhook ? "开启" : "关闭"],
    ["仅保护分支审查", info.protected_branches_only ? "是" : "否"],
    ["支持扩展名", info.supported_extensions],
    ["REVIEW_MAX_TOKENS", info.review_max_tokens],
    ["REPO_CACHE_DIR", info.repo_cache_dir],
    ["低分阈值", String(info.score_threshold)],
    ["日报 Cron", info.report_crontab],
  ];

  return (
    <div className="space-y-5">
      <PageHeader
        eyebrow="Runtime observability"
        title="系统运行状态"
        description="查看当前审查策略、模型配置、平台连接与最近应用日志。敏感凭据仅显示脱敏状态。"
        icon={ServerCog}
      />
      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
        {metrics.map(({ label, value, icon: Icon }) => (
          <div key={label} className="panel panel-hover rounded-2xl p-4 sm:p-5">
            <div className="flex items-center justify-between">
              <div className="text-[11px] font-semibold text-[var(--color-mute)]">{label}</div>
              <div className="rounded-lg bg-cyan-300/[0.08] p-2 text-cyan-200"><Icon size={15} /></div>
            </div>
            <div className="mono mt-4 text-lg text-[#edf7ff]">{value}</div>
          </div>
        ))}
      </div>

      <div className="panel overflow-hidden rounded-2xl">
        <div className="border-b border-white/[0.06] px-5 py-4">
          <div className="text-sm font-semibold text-[#e6effa]">配置快照</div>
          <div className="mt-0.5 text-[10px] text-[#61758b]">只读运行时配置</div>
        </div>
        <table className="data-table w-full text-sm">
          <tbody>
            {rows.map(([k, v]) => (
              <tr key={k} className="border-t border-white/[0.045]">
                <td className="w-48 px-5 py-3 text-xs text-[var(--color-mute)]">{k}</td>
                <td className="px-5 py-3 mono text-xs text-[#c8d6e5]">{v}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="panel rounded-2xl p-4 sm:p-5">
        <div className="mb-1 flex items-center gap-2 text-sm font-semibold text-[#e6effa]">
          <span className="status-dot" />
          实时日志尾部
        </div>
        <div className="mb-4 text-[10px] uppercase tracking-[0.12em] text-[#566a81]">Application log stream</div>
        <pre className="max-h-[420px] overflow-auto rounded-xl border border-white/[0.06] bg-[#03070d] p-4 mono text-[11px] leading-5 text-[#9fb0c2]">
          {info.log_tail || "（日志为空）"}
        </pre>
      </div>
    </div>
  );
}

export default function SystemPage() {
  return (
    <AuthGate>
      <SystemContent />
    </AuthGate>
  );
}
