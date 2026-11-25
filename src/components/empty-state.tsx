import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

export function EmptyState({
  icon: Icon,
  children,
}: {
  icon?: LucideIcon;
  children: ReactNode;
}) {
  return (
    <div className="flex h-full min-h-40 flex-col items-center justify-center gap-4 text-center text-sm text-base-content/60">
      {Icon && <Icon className="size-8 text-primary" aria-hidden="true" />}
      <p>{children}</p>
    </div>
  );
}
