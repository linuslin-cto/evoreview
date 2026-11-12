import type { Metadata } from "next";
import { IBM_Plex_Sans, JetBrains_Mono, Orbitron } from "next/font/google";
import "./globals.css";

const orbitron = Orbitron({
  variable: "--font-orbitron",
  subsets: ["latin"],
  weight: ["500", "600", "700"],
});

const plex = IBM_Plex_Sans({
  variable: "--font-plex",
  subsets: ["latin"],
  weight: ["400", "500", "600", "700"],
});

const jetbrains = JetBrains_Mono({
  variable: "--font-jetbrains",
  subsets: ["latin"],
  weight: ["400", "500", "600"],
});

export const metadata: Metadata = {
  title: "獬豸 · AI 代码审查工作台",
  description: "基于大模型的自动化代码审查控制台",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html
      lang="zh-CN"
      className={`${orbitron.variable} ${plex.variable} ${jetbrains.variable} h-full dark`}
    >
      <body className="min-h-full antialiased">{children}</body>
    </html>
  );
}
