import {
  BookOpen,
  FileText,
  FolderOpen,
  GitBranch,
  History,
  Waypoints,
} from "lucide-react";
import { WeftPrint } from "@/components/weft-art";
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
    <div
      className="h-full overflow-auto bg-base-200 text-base-content"
      lang={lang === "zh" ? "zh-CN" : "en"}
    >
      <main className="mx-auto flex min-h-full w-full max-w-6xl flex-col gap-5 p-6">
        <div className="grid flex-1 grid-cols-1 gap-6 lg:grid-cols-2">
          <div className="flex min-w-0 flex-col gap-4">
            <button
              type="button"
              className="btn btn-primary btn-md self-start"
              onClick={onOpen}
              onMouseEnter={onPreload}
              onFocus={onPreload}
            >
              <FolderOpen className="size-4" />
              {strings[lang].open}
            </button>

            <section className="card card-sm card-border min-w-0 flex-1 bg-base-100">
              <div className="card-body gap-3">
                <header className="flex flex-col gap-0.5">
                  <h2 className="card-title flex items-center gap-2 text-sm font-semibold">
                    <History className="size-4 text-base-content/60" />
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
                          className="btn btn-ghost h-auto w-full justify-start gap-3 px-2 py-2.5 text-left font-normal"
                          onClick={() => onRecent(p)}
                          onMouseEnter={onPreload}
                          onFocus={onPreload}
                        >
                          <FileText className="size-4 shrink-0 text-base-content/50" />
                          <span className="flex min-w-0 flex-col items-start">
                            <strong className="max-w-full truncate text-sm font-medium">
                              {p.split(PATH_SEP_RE).pop()}
                            </strong>
                            <small className="max-w-full truncate font-mono text-xs font-normal text-base-content/50">
                              {p}
                            </small>
                          </span>
                        </button>
                      </li>
                    ))}
                  </ul>
                ) : (
                  <div className="flex items-center gap-2.5 py-4 text-xs text-base-content/50">
                    <FileText className="size-4 shrink-0 text-base-content/35" />
                    <p>{strings[lang].recentEmpty}</p>
                  </div>
                )}
              </div>
            </section>
          </div>

          <aside
            className="flex min-h-[340px] flex-col gap-4 rounded-box bg-primary p-6 text-primary-content"
            aria-hidden="true"
          >
            <span className="text-7xl font-black uppercase leading-none tracking-tighter lg:text-8xl">
              weft
            </span>
            <div className="min-h-0 flex-1">
              <WeftPrint />
            </div>
            <div className="flex flex-wrap justify-between gap-3 border-t border-primary-content/25 pt-3 text-xs text-primary-content/80">
              <span className="inline-flex items-center gap-1.5">
                <BookOpen className="size-3.5" />
                {strings[lang].tab1}
              </span>
              <span className="inline-flex items-center gap-1.5">
                <Waypoints className="size-3.5" />
                {strings[lang].tab2}
              </span>
              <span className="inline-flex items-center gap-1.5">
                <GitBranch className="size-3.5" />
                {strings[lang].tab4}
              </span>
            </div>
          </aside>
        </div>

        <p className="text-xs text-base-content/50">{strings[lang].fileHint}</p>
      </main>
    </div>
  );
}
