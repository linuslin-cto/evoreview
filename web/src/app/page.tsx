"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import { getToken } from "@/lib/api";

export default function HomePage() {
  const router = useRouter();
  useEffect(() => {
    router.replace(getToken() ? "/reviews" : "/login");
  }, [router]);
  return (
    <div className="flex min-h-screen items-center justify-center tech-radial tech-grid">
      <div className="mono text-cyan-300">BOOTING XIEZHI…</div>
    </div>
  );
}
