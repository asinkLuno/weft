import { GitBranch } from "lucide-react";
import { memo, useMemo } from "react";
import { EmptyState } from "@/components/empty-state";
import { type Lang, strings } from "@/i18n";
import type { WeftDoc } from "@/lib/document";
import { buildNarrativeView } from "./model";
import { NarrativeGantt } from "./narrative-gantt";

function NarrativeViewInner({
  doc,
  lang,
}: {
  doc: WeftDoc | null;
  lang: Lang;
}) {
  const t = strings[lang];
  const { byId, bounds, scale, entries } = useMemo(
    () => buildNarrativeView(doc),
    [doc],
  );
  if (entries.length === 0) {
    return <EmptyState icon={GitBranch}>{t.narrativeEmpty}</EmptyState>;
  }
  return (
    <div className="flex h-full flex-col gap-6 overflow-auto">
      {entries.map(([name, nar]) => (
        <NarrativeGantt
          key={name}
          name={name}
          nar={nar}
          byId={byId}
          bounds={bounds}
          scale={scale}
          t={t}
        />
      ))}
    </div>
  );
}

export const NarrativeView = memo(NarrativeViewInner);
