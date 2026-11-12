"use client";

import {
  ArrowRight,
  Braces,
  GitPullRequest,
  LockKeyhole,
  Radar,
  ShieldCheck,
  User,
} from "lucide-react";
import Image from "next/image";
import { useRouter } from "next/navigation";
import { FormEvent, useState } from "react";
import { api, setSession } from "@/lib/api";

export default function LoginPage() {
  const router = useRouter();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [remember, setRemember] = useState(true);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setLoading(true);
    setError("");
    try {
      const data = await api<{ token: string; username: string }>("/api/auth/login", {
        method: "POST",
        body: JSON.stringify({ username, password }),
      });
      setSession(data.token, data.username, remember);
      router.replace("/reviews");
    } catch (err) {
      console.error("login failed", err);
      setError("登录失败，请检查用户名和密码");
    } finally {
      setLoading(false);
    }
  }

  const capabilities = [
    { icon: GitPullRequest, title: "多平台接入", hint: "GitLab · GitHub · Gitea" },
    { icon: Braces, title: "深度审查", hint: "Diff · Agentic context" },
    { icon: ShieldCheck, title: "安全闭环", hint: "Review · Alert · Audit" },
  ];

  return (
    <div className="relative min-h-screen overflow-hidden tech-radial">
      <div className="pointer-events-none absolute inset-0 tech-grid" />
      <div className="pointer-events-none absolute inset-0 scanlines opacity-20" />

      <div className="relative z-10 mx-auto grid min-h-screen max-w-7xl items-center gap-12 px-5 py-10 lg:grid-cols-[1.15fr_.85fr] lg:px-12">
        <section className="hidden lg:block">
          <div className="mb-10 flex items-center gap-4">
            <Image
              src="/xiezhi-logo.png"
              alt="獬豸"
              width={64}
              height={64}
              className="rounded-2xl ring-1 ring-cyan-300/20"
              priority
            />
            <div>
              <div className="font-[family-name:var(--font-orbitron)] text-xl font-semibold tracking-[0.24em] text-[#d6fcff]">
                XIEZHI
              </div>
              <div className="mt-1 text-xs uppercase tracking-[0.18em] text-[#687c92]">
                AI code intelligence platform
              </div>
            </div>
          </div>

          <div className="max-w-2xl">
            <div className="mb-4 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-cyan-300/70">
              <Radar size={14} />
              Autonomous review network
            </div>
            <h1 className="text-4xl font-semibold leading-tight tracking-tight text-[#f0f7ff] xl:text-5xl">
              让每一次代码变更
              <span className="block bg-gradient-to-r from-cyan-200 to-blue-300 bg-clip-text text-transparent">
                都经过智能质量防线
              </span>
            </h1>
            <p className="mt-5 max-w-xl text-sm leading-7 text-[#7d90a6]">
              自动接收 Git Webhook，通过大模型完成上下文审查、风险识别与质量评分，并沉淀为可追踪的工程洞察。
            </p>
          </div>

          <div className="mt-10 grid max-w-2xl grid-cols-3 gap-3">
            {capabilities.map(({ icon: Icon, title, hint }) => (
              <div key={title} className="panel panel-hover rounded-2xl p-4">
                <Icon size={17} className="mb-4 text-cyan-200" />
                <div className="text-xs font-semibold text-[#dce6f2]">{title}</div>
                <div className="mt-1 text-[10px] text-[#61758b]">{hint}</div>
              </div>
            ))}
          </div>
        </section>

        <section className="mx-auto w-full max-w-md">
          <div className="panel rounded-3xl p-6 sm:p-8">
            <div className="mb-7 lg:hidden">
              <Image src="/xiezhi-logo.png" alt="獬豸" width={52} height={52} className="mb-4 rounded-xl" priority />
            </div>
            <div className="mb-7">
              <div className="text-[10px] font-semibold uppercase tracking-[0.16em] text-cyan-300/70">
                Secure console access
              </div>
              <h2 className="mt-2 text-2xl font-semibold tracking-tight text-[#f0f7ff]">登录控制台</h2>
              <p className="mt-2 text-xs text-[#70849a]">使用管理员凭据进入代码审查工作台</p>
            </div>

            <form className="space-y-4" onSubmit={onSubmit}>
              <label className="field-label">
                用户名
                <div className="relative">
                  <User size={15} className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-[#5e7288]" />
                  <input
                    className="field pl-10"
                    value={username}
                    onChange={(event) => setUsername(event.target.value)}
                    placeholder="operator"
                    autoComplete="username"
                    required
                  />
                </div>
              </label>
              <label className="field-label">
                密码
                <div className="relative">
                  <LockKeyhole size={15} className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-[#5e7288]" />
                  <input
                    className="field pl-10"
                    type="password"
                    value={password}
                    onChange={(event) => setPassword(event.target.value)}
                    placeholder="••••••••"
                    autoComplete="current-password"
                    required
                  />
                </div>
              </label>
              <label className="flex cursor-pointer items-center gap-2 text-xs text-[#71849a]">
                <input
                  className="accent-cyan-300"
                  type="checkbox"
                  checked={remember}
                  onChange={(event) => setRemember(event.target.checked)}
                />
                记住登录状态
              </label>
              {error && (
                <div role="alert" className="rounded-lg border border-rose-300/15 bg-rose-300/[0.06] px-3 py-2 text-xs text-rose-200">
                  {error}
                </div>
              )}
              <button type="submit" className="tech-button w-full" disabled={loading}>
                {loading ? "正在验证身份…" : "进入工作台"}
                {!loading && <ArrowRight size={15} />}
              </button>
            </form>

            <div className="mt-6 flex items-center justify-center gap-2 border-t border-white/[0.06] pt-5 text-[10px] text-[#52667d]">
              <ShieldCheck size={12} />
              HMAC signed session · 30 days
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}
