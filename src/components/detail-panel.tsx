import { CalendarDays, Code2, type LucideIcon, X } from "lucide-react";
import { type ReactNode, useEffect, useRef } from "react";
import { entityInitials } from "@/lib/ui";

const CLOSE_ICON = <X className="size-4" />;
const LOCATE_ICON = <Code2 className="size-3.5" aria-hidden="true" />;
const CALENDAR_ICON = (
  <CalendarDays className="mt-0.5 size-3.5 shrink-0" aria-hidden="true" />
);

const AVATAR_SIZE_CLASS = {
  sm: "w-7 text-[10px]",
  md: "w-9 text-xs",
  lg: "w-10 text-xs",
} as const;

// 模块级共享单一 Escape 监听与栈：多面板分屏时按 Esc 只关闭最近打开的栈顶面板
const escapeCloseStack: Array<() => void> = [];

function handleGlobalEscape(event: KeyboardEvent) {
  if (event.defaultPrevented || event.key !== "Escape") return;
  const top = escapeCloseStack[escapeCloseStack.length - 1];
  if (top) {
    event.preventDefault();
    top();
  }
}

function registerEscapeHandler(handler: () => void): () => void {
  if (escapeCloseStack.length === 0) {
    window.addEventListener("keydown", handleGlobalEscape);
  }
  escapeCloseStack.push(handler);
  return () => {
    const idx = escapeCloseStack.lastIndexOf(handler);
    if (idx !== -1) escapeCloseStack.splice(idx, 1);
    if (escapeCloseStack.length === 0) {
      window.removeEventListener("keydown", handleGlobalEscape);
    }
  };
}

export function EntityAvatar({
  name,
  avatar,
  size = "md",
}: {
  name: string;
  avatar?: string;
  size?: keyof typeof AVATAR_SIZE_CLASS;
}) {
  return (
    <div className="avatar avatar-placeholder shrink-0">
      <div
        className={`rounded-full bg-neutral text-neutral-content ${AVATAR_SIZE_CLASS[size]}`}
      >
        {avatar ? (
          <img src={avatar} alt={name} className="object-cover" />
        ) : (
          <span className="font-medium">{entityInitials(name)}</span>
        )}
      </div>
    </div>
  );
}

export function DetailPanel({
  ariaLabel,
  title,
  leading,
  closeLabel,
  locateLabel,
  emptyLabel,
  isEmpty,
  extraActions,
  className = "",
  onClose,
  onLocate,
  children,
}: {
  ariaLabel: string;
  title: string;
  leading: ReactNode;
  closeLabel: string;
  locateLabel: string;
  emptyLabel: string;
  isEmpty: boolean;
  extraActions?: ReactNode;
  className?: string;
  onClose: () => void;
  onLocate: () => void;
  children: ReactNode;
}) {
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  useEffect(() => {
    return registerEscapeHandler(() => onCloseRef.current());
  }, []);

  return (
    <aside
      aria-label={ariaLabel}
      className={`flex h-full w-80 shrink-0 flex-col bg-base-100 ${className}`}
    >
      <header className="flex flex-col gap-2.5 border-b border-base-300 p-4">
        <div className="flex items-start gap-2.5">
          {leading}
          <h2 className="min-w-0 flex-1 self-center break-words text-base leading-5 font-semibold">
            {title}
          </h2>
          <button
            type="button"
            className="btn btn-ghost btn-xs btn-square shrink-0"
            aria-label={closeLabel}
            title={closeLabel}
            onClick={onClose}
          >
            {CLOSE_ICON}
          </button>
        </div>
        <div className="flex items-center justify-between gap-2">
          <button
            type="button"
            className="btn btn-outline btn-xs border-base-300"
            onClick={onLocate}
          >
            {LOCATE_ICON}
            {locateLabel}
          </button>
          {extraActions}
        </div>
      </header>

      <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-auto p-4 text-xs">
        {isEmpty ? (
          <p className="text-base-content/50">{emptyLabel}</p>
        ) : (
          children
        )}
      </div>
    </aside>
  );
}

export function DetailSection({
  title,
  icon: Icon,
  children,
}: {
  title: string;
  icon?: LucideIcon;
  children: ReactNode;
}) {
  return (
    <section className="flex flex-col gap-2 border-t border-base-300 pt-3">
      <h3 className="flex items-center gap-1.5 font-medium text-base-content/60">
        {Icon && <Icon className="size-3.5" aria-hidden="true" />}
        {title}
      </h3>
      {children}
    </section>
  );
}

export function DetailTimeRow({ value }: { value: string }) {
  return (
    <div className="flex items-start gap-1.5 text-base-content/70">
      {CALENDAR_ICON}
      <span className="break-words font-mono leading-5">{value}</span>
    </div>
  );
}

export function DetailBadgeList({
  items,
  title,
  onClick,
}: {
  items: ReadonlyArray<{ id: string; label: string }>;
  title?: string;
  onClick: (id: string) => void;
}) {
  return (
    <div className="flex flex-wrap gap-1.5">
      {items.map((item) => (
        <button
          key={item.id}
          type="button"
          className="badge badge-sm badge-outline h-auto cursor-pointer border-base-300 whitespace-normal break-words transition-colors hover:border-primary hover:text-primary"
          title={title}
          onClick={() => onClick(item.id)}
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}
