import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

export function EmptyState({
  icon: Icon,
  action,
  children,
}: {
  icon?: LucideIcon;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex h-full min-h-40 flex-col items-center justify-center gap-3 text-center text-sm text-base-content/50">
      {Icon && (
        <Icon className="size-7 text-base-content/30" aria-hidden="true" />
      )}
      <p>{children}</p>
      {action}
    </div>
  );
}
