import { BookOpen, CalendarDays, UserRound } from "lucide-react";
import { memo } from "react";
import { type Lang, strings } from "@/i18n";
import type { WeftDoc } from "@/lib/document";

export const StoryView = memo(function StoryView({
  doc,
  lang,
}: {
  doc: WeftDoc;
  lang: Lang;
}) {
  const t = strings[lang];
  const story = doc.story;
  if (!story) return null;
  return (
    <div className="mx-auto w-full max-w-5xl py-4">
      <header className="mb-10 flex flex-col gap-5 border-b border-base-300 pb-8">
        <div className="flex items-center gap-2 text-sm text-base-content/60">
          <BookOpen className="size-4" aria-hidden="true" />
          <span>{t.tab1}</span>
        </div>
        <h1 className="break-words text-4xl font-bold tracking-tight text-primary">
          {story.title}
        </h1>
        {story.author && (
          <div className="flex items-center gap-2 text-sm text-base-content/60">
            <UserRound className="size-4 shrink-0" aria-hidden="true" />
            <span className="break-words">
              {t.author}：{story.author}
            </span>
          </div>
        )}
        {story.description && (
          <p className="max-w-3xl whitespace-pre-wrap break-words text-base leading-8 text-base-content/70">
            {story.description}
          </p>
        )}
      </header>
      <div className="grid items-start gap-6 md:grid-cols-[minmax(0,1fr)_minmax(0,2fr)]">
        {(story.date?.name || story.date?.description) && (
          <div className="card card-border min-w-0 bg-base-100">
            <div className="card-body gap-2">
              <h2 className="card-title flex items-center gap-2 text-sm">
                <CalendarDays
                  className="size-4 text-base-content/60"
                  aria-hidden="true"
                />
                {t.calendar}
              </h2>
              {story.date.name && (
                <p className="break-words font-medium">{story.date.name}</p>
              )}
              {story.date.description && (
                <p className="whitespace-pre-wrap break-words text-sm leading-6 text-base-content/70">
                  {story.date.description}
                </p>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
});
