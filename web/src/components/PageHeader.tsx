import type { LucideIcon } from "lucide-react";

export function PageHeader({
  eyebrow,
  title,
  description,
  icon: Icon,
  action,
}: {
  eyebrow: string;
  title: string;
  description: string;
  icon: LucideIcon;
  action?: React.ReactNode;
}) {
  return (
    <div className="mb-5 flex flex-col justify-between gap-4 sm:flex-row sm:items-end">
      <div>
        <div className="mb-2 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-[0.16em] text-cyan-300/70">
          <Icon size={13} />
          {eyebrow}
        </div>
        <h2 className="text-xl font-semibold tracking-tight text-[#f1f7ff] sm:text-2xl">{title}</h2>
        <p className="mt-1.5 max-w-2xl text-xs leading-5 text-[var(--color-mute)] sm:text-sm">{description}</p>
      </div>
      {action}
    </div>
  );
}
