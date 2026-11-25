import {
  BookOpen,
  FileText,
  FolderOpen,
  GitBranch,
  History,
  Waypoints,
} from "lucide-react";
import { WeftMark, WeftPrint } from "@/components/weft-art";
import { type Lang, strings } from "@/i18n";

const PATH_SEP_RE = /[\\/]/;

export function WelcomeView({
  lang,
  recents,
  onOpen,
  onRecent,
  onPreload,
}: {
  lang: Lang;
  recents: string[];
  onOpen: () => void;
  onRecent: (path: string) => void;
  onPreload?: () => void;
}) {
  return (
    <main
      className="mx-auto flex min-h-full w-full max-w-[1440px] flex-col gap-6 p-6"
      lang={lang === "zh" ? "zh-CN" : "en"}
    >
      <header className="flex items-center gap-6">
        <div className="flex items-center gap-2 text-primary">
          <WeftMark />
          <span className="text-3xl font-semibold tracking-tight">weft</span>
        </div>
        <span className="border-l border-base-300 pl-6 text-xs text-base-content/60">
          {strings[lang].workspace}
        </span>
      </header>

      <div className="grid flex-1 grid-cols-1 items-start gap-8 lg:grid-cols-2">
        <section className="flex flex-col items-start gap-6 lg:self-center">
          <h1 className="whitespace-pre-line text-4xl font-bold tracking-tight text-primary lg:text-5xl">
            {strings[lang].welcomeTitle}
          </h1>
          <p className="max-w-prose text-sm leading-7 text-base-content/70">
            {strings[lang].welcomeDescription}
          </p>
          <button
            type="button"
            className="btn btn-primary btn-lg"
            onClick={onOpen}
            onMouseEnter={onPreload}
            onFocus={onPreload}
          >
            <FolderOpen />
            {strings[lang].open}
          </button>
        </section>

        <aside
          className="flex min-h-[420px] flex-col gap-4 rounded-box bg-primary p-6 text-primary-content lg:row-span-2 lg:min-h-full"
          aria-hidden="true"
        >
          <span className="text-[7rem] font-black uppercase leading-none tracking-tighter lg:text-[9rem]">
            weft
          </span>
          <div className="min-h-0 flex-1">
            <WeftPrint />
          </div>
          <div className="flex flex-wrap justify-between gap-3 border-t border-primary-content/30 pt-4 text-xs">
            <span className="inline-flex items-center gap-2">
              <BookOpen className="size-4" />
              {strings[lang].tab1}
            </span>
            <span className="inline-flex items-center gap-2">
              <Waypoints className="size-4" />
              {strings[lang].tab2}
            </span>
            <span className="inline-flex items-center gap-2">
              <GitBranch className="size-4" />
              {strings[lang].tab4}
            </span>
          </div>
        </aside>

        <section className="card card-border w-full min-w-0 bg-base-100">
          <div className="card-body gap-4">
            <header className="flex flex-col gap-1">
              <h2 className="card-title flex items-center gap-2">
                <History className="size-4" />
                {strings[lang].recent}
              </h2>
              <p className="text-xs text-base-content/60">
                {strings[lang].recentDescription}
              </p>
            </header>
            {recents.length > 0 ? (
              <ul className="flex flex-col divide-y divide-base-300">
                {recents.map((p) => (
                  <li key={p} className="min-w-0">
                    <button
                      type="button"
                      className="btn btn-ghost h-auto w-full justify-start gap-3 py-3 text-left font-normal"
                      onClick={() => onRecent(p)}
                      onMouseEnter={onPreload}
                      onFocus={onPreload}
                    >
                      <FileText className="size-5 shrink-0 text-primary" />
                      <span className="flex min-w-0 flex-col items-start">
                        <strong className="max-w-full truncate text-sm font-medium">
                          {p.split(PATH_SEP_RE).pop()}
                        </strong>
                        <small className="max-w-full truncate text-xs font-normal text-base-content/60">
                          {p}
                        </small>
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            ) : (
              <div className="flex items-center gap-3 py-6 text-xs text-base-content/60">
                <FileText className="size-5 shrink-0 text-primary" />
                <p>{strings[lang].recentEmpty}</p>
              </div>
            )}
          </div>
        </section>
      </div>

      <p className="text-xs text-base-content/60">{strings[lang].fileHint}</p>
    </main>
  );
}
