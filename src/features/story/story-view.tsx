import { BookOpen, CalendarDays, UserRound } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { type Lang, strings } from "@/i18n";
import type { WeftDoc } from "@/lib/document";
import "./story.css";
export function StoryView({ doc, lang }: { doc: WeftDoc; lang: Lang }) {
  const t = strings[lang];
  const story = doc.story;
  if (!story) return null;
  return (
    <div className="story-view @container mx-auto w-full max-w-5xl py-4">
      <header className="story-header mb-10 space-y-5 border-b pb-8">
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <BookOpen className="size-4" aria-hidden="true" />
          <span>{t.tab1}</span>
        </div>
        <h1 className="break-words text-3xl font-semibold tracking-tight @min-[640px]:text-4xl">
          {story.title}
        </h1>
        {story.author && (
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <UserRound className="size-4 shrink-0" aria-hidden="true" />
            <span className="break-words">
              {t.author}：{story.author}
            </span>
          </div>
        )}
        {story.description && (
          <p className="max-w-3xl whitespace-pre-wrap break-words text-base leading-8 text-muted-foreground">
            {story.description}
          </p>
        )}
      </header>
      <div className="grid items-start gap-6 @min-[640px]:grid-cols-[minmax(0,1fr)_minmax(0,2fr)]">
        {(story.date?.name || story.date?.description) && (
          <Card className="min-w-0 gap-5">
            <CardHeader className="border-b">
              <CardTitle>
                <h2 className="flex items-center gap-2 text-sm">
                  <CalendarDays
                    className="size-4 text-muted-foreground"
                    aria-hidden="true"
                  />
                  {t.calendar}
                </h2>
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-2">
              {story.date.name && (
                <p className="break-words font-medium">{story.date.name}</p>
              )}
              {story.date.description && (
                <p className="whitespace-pre-wrap break-words text-sm leading-6 text-muted-foreground">
                  {story.date.description}
                </p>
              )}
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  );
}
