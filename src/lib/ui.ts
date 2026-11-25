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
