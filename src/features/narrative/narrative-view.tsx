import { memo } from "react";
import { type Lang, strings } from "@/i18n";
import type { WeftDoc } from "@/lib/document";
import { buildNarrativeView } from "./model";
import { NarrativeGantt } from "./narrative-gantt";
import "./narrative.css";

function NarrativeViewInner({
  doc,
  lang,
}: {
  doc: WeftDoc | null;
  lang: Lang;
}) {
  const t = strings[lang];
  const { byId, bounds, scale, entries } = buildNarrativeView(doc);
  if (entries.length === 0) {
    return <div className="view-empty">{t.narrativeEmpty}</div>;
  }
  return (
    <div className="narrative-view flex h-full flex-col gap-8 overflow-auto">
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
