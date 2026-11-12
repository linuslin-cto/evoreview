"use client";



import {

  Activity,

  ChevronRight,

  FileSearch,

  LayoutDashboard,

  LogOut,

  Settings2,

  ShieldCheck,

  Users,

} from "lucide-react";

import Image from "next/image";

import Link from "next/link";

import { usePathname, useRouter } from "next/navigation";

import { clearSession, getUsername } from "@/lib/api";



const NAV = [

  { href: "/reviews", label: "审查总览", hint: "Review feed", icon: LayoutDashboard },

  { href: "/members", label: "成员洞察", hint: "Team signals", icon: Users },

  { href: "/deep", label: "深度审查", hint: "Review detail", icon: FileSearch },

  { href: "/reports", label: "报告中心", hint: "Intelligence", icon: Activity },

  { href: "/system", label: "系统状态", hint: "Operations", icon: Settings2 },

];



export function AppShell({ children }: { children: React.ReactNode }) {

  const pathname = usePathname();

  const router = useRouter();

  const username = getUsername() || "operator";

  const current = NAV.find((item) => pathname.startsWith(item.href)) || NAV[0];



  return (

    <div className="relative min-h-[100dvh] overflow-x-hidden tech-radial">

      <div className="pointer-events-none fixed inset-0 tech-grid opacity-70" />

      <div className="pointer-events-none fixed inset-0 scanlines opacity-20" />



      <aside className="fixed inset-y-0 left-0 z-30 hidden w-[var(--sidebar-width)] border-r border-white/[0.06] bg-[#070d17]/90 px-4 py-5 backdrop-blur-2xl lg:flex lg:flex-col">

        <div className="flex items-center gap-3 px-2">

          <Image

            src="/xiezhi-logo.png"

            alt="獬豸"

            width={42}

            height={42}

            className="rounded-xl ring-1 ring-cyan-300/20"

          />

          <div className="min-w-0">

            <div className="font-[family-name:var(--font-orbitron)] text-sm font-semibold tracking-[0.2em] text-[#c8fbff]">

              XIEZHI

            </div>

            <div className="mt-0.5 text-[10px] uppercase tracking-[0.16em] text-[var(--color-mute)]">

              Code intelligence

            </div>

          </div>

        </div>



        <div className="mx-2 mt-7 mb-3 text-[10px] font-semibold uppercase tracking-[0.16em] text-[#52657d]">

          Workspace

        </div>

        <nav className="flex flex-1 flex-col gap-1">

          {NAV.map((item) => {

            const active = pathname.startsWith(item.href);

            const Icon = item.icon;

            return (

              <Link

                key={item.href}

                href={item.href}

                aria-current={active ? "page" : undefined}

                className={`group relative flex items-center gap-3 rounded-xl px-3 py-3 transition ${

                  active

                    ? "bg-gradient-to-r from-cyan-300/[0.12] to-blue-400/[0.04] text-cyan-100"

                    : "text-[var(--color-mute)] hover:bg-white/[0.035] hover:text-[var(--color-fog)]"

                }`}

              >

                {active && (

                  <span className="absolute inset-y-2 left-0 w-0.5 rounded-full bg-cyan-300 shadow-[0_0_12px_rgba(94,231,242,.9)]" />

                )}

                <Icon

                  size={17}

                  strokeWidth={1.8}

                  className={active ? "text-cyan-200" : "text-[#657991] group-hover:text-[#9eb0c4]"}

                />

                <span className="min-w-0 flex-1">

                  <span className="block text-[13px] font-semibold">{item.label}</span>

                  <span className="mt-0.5 block text-[10px] text-[#5d7188]">{item.hint}</span>

                </span>

                {active && <ChevronRight size={13} className="text-cyan-300/70" />}

              </Link>

            );

          })}

        </nav>



        <div className="rounded-xl border border-white/[0.06] bg-white/[0.025] p-3">

          <div className="flex items-center gap-2">

            <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-cyan-300/10 text-xs font-bold text-cyan-200">

              {username.slice(0, 1).toUpperCase()}

            </div>

            <div className="min-w-0 flex-1">

              <div className="truncate text-xs font-semibold text-[#dce6f2]">{username}</div>

              <div className="mt-0.5 flex items-center gap-1.5 text-[10px] text-[var(--color-mute)]">

                <span className="status-dot" />

                Operator online

              </div>

            </div>

            <button

              type="button"

              aria-label="退出登录"

              className="rounded-lg p-2 text-[#60748b] transition hover:bg-white/5 hover:text-rose-300"

              onClick={() => {

                clearSession();

                router.replace("/login");

              }}

            >

              <LogOut size={15} />

            </button>

          </div>

        </div>

      </aside>



      <div className="relative z-10 min-h-[100dvh] lg:pl-[var(--sidebar-width)]">

        <header className="sticky top-0 z-20 border-b border-white/[0.06] bg-[#050a12]/80 backdrop-blur-2xl">

          <div className="mx-auto flex h-[var(--header-height)] max-w-[var(--content-max-width)] items-center justify-between gap-3 px-4 sm:gap-4 sm:px-6 lg:px-8">

            <div className="flex min-w-0 items-center gap-3">

              <Image

                src="/xiezhi-logo.png"

                alt="獬豸"

                width={34}

                height={34}

                className="rounded-lg lg:hidden"

              />

              <div className="min-w-0">

                <div className="flex items-center gap-2">

                  <h1 className="truncate text-base font-semibold text-[#edf6ff] sm:text-lg">

                    {current.label}

                  </h1>

                  <span className="hidden rounded-md border border-white/[0.06] bg-white/[0.025] px-2 py-0.5 mono text-[9px] uppercase tracking-[0.14em] text-[#71849b] md:inline">

                    {current.hint}

                  </span>

                </div>

                <p className="mt-0.5 hidden truncate text-[11px] text-[var(--color-mute)] sm:block">

                  AI 驱动的代码质量分析与风险洞察

                </p>

              </div>

            </div>

            <div className="flex shrink-0 items-center gap-2">

              <div className="hidden items-center gap-2 rounded-xl border border-mint/15 bg-mint/5 px-3 py-2 text-[10px] font-semibold uppercase tracking-[0.1em] text-mint md:flex">

                <ShieldCheck size={14} />

                Protected

              </div>

              <button

                type="button"

                aria-label="退出登录"

                className="rounded-xl border border-white/[0.06] bg-white/[0.025] p-2.5 text-[#71849b] transition hover:border-rose-300/20 hover:text-rose-200 lg:hidden"

                onClick={() => {

                  clearSession();

                  router.replace("/login");

                }}

              >

                <LogOut size={16} />

              </button>

            </div>

          </div>

        </header>



        <main className="app-main">{children}</main>

      </div>



      <nav className="fixed inset-x-3 bottom-[max(0.75rem,env(safe-area-inset-bottom))] z-40 grid grid-cols-5 rounded-2xl border border-white/[0.08] bg-[#09111e]/92 p-1.5 shadow-2xl backdrop-blur-2xl lg:hidden">

        {NAV.map((item) => {

          const active = pathname.startsWith(item.href);

          const Icon = item.icon;

          return (

            <Link

              key={item.href}

              href={item.href}

              aria-current={active ? "page" : undefined}

              className={`flex min-h-[52px] flex-col items-center justify-center gap-1 rounded-xl px-1 py-2 text-[9px] transition ${

                active ? "bg-cyan-300/10 text-cyan-200" : "text-[#63768c]"

              }`}

            >

              <Icon size={16} strokeWidth={active ? 2.2 : 1.7} />

              <span className="max-w-full truncate px-0.5">{item.label.slice(0, 4)}</span>

            </Link>

          );

        })}

      </nav>

    </div>

  );

}


