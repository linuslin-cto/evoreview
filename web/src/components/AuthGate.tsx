"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { getToken } from "@/lib/api";
import { AppShell } from "@/components/AppShell";

export function AuthGate({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const [ready, setReady] = useState(false);

  useEffect(() => {
    if (!getToken()) {
      router.replace("/login");
      return;
    }
    setReady(true);
  }, [router]);

  if (!ready) {
    return (
      <div className="flex min-h-screen items-center justify-center tech-radial tech-grid">
        <div className="mono text-sm text-cyan-300">INITIALIZING CONSOLE…</div>
      </div>
    );
  }

  return <AppShell>{children}</AppShell>;
}
