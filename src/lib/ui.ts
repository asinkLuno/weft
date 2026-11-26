import type { DiskStatus } from "@/features/document/use-document";

export type DocumentTone = "synced" | "dirty" | "modified" | "deleted";

export function documentToneOf(
  dirty: boolean,
  diskStatus: DiskStatus,
): DocumentTone {
  if (diskStatus === "deleted") return "deleted";
  if (diskStatus === "modified") return "modified";
  if (dirty) return "dirty";
  return "synced";
}

export const DOCUMENT_TONE_STYLE: Record<
  DocumentTone,
  { dot: string; label: string }
> = {
  synced: {
    dot: "bg-base-content/30",
    label: "text-base-content/60",
  },
  dirty: {
    dot: "bg-primary",
    label: "text-primary font-medium",
  },
  modified: {
    dot: "bg-warning ring-1 ring-warning-content/30",
    label: "badge badge-xs badge-warning",
  },
  deleted: {
    dot: "bg-error ring-1 ring-error-content/30",
    label: "badge badge-xs badge-error",
  },
};

const INITIALS_SPLIT_RE = /[\s·._-]+/u;

export function entityInitials(id: string): string {
  const parts = id.trim().split(INITIALS_SPLIT_RE).filter(Boolean);
  if (parts.length > 1) {
    return parts
      .slice(0, 2)
      .map((part) => Array.from(part)[0])
      .join("")
      .toLocaleUpperCase();
  }
  return Array.from(parts[0] ?? "?")
    .slice(0, 2)
    .join("")
    .toLocaleUpperCase();
}

export function formatTimeRange(
  startDisplay?: string,
  endDisplay?: string,
): string | undefined {
  if (startDisplay && endDisplay && endDisplay !== startDisplay) {
    return `${startDisplay} — ${endDisplay}`;
  }
  return startDisplay ?? endDisplay;
}
