import {
  BookOpen,
  FileText,
  FolderOpen,
  GitBranch,
  History,
  Waypoints,
} from "lucide-react";
import { memo } from "react";
import { WeftPrint } from "@/components/weft-art";
import { type Lang, strings } from "@/i18n";

const BOOK_ICON = <BookOpen className="size-3.5" />;
const WAYPOINTS_ICON = <Waypoints className="size-3.5" />;
const GIT_BRANCH_ICON = <GitBranch className="size-3.5" />;
const FOLDER_OPEN_ICON = <FolderOpen className="size-4" />;
const HISTORY_ICON = <History className="size-4 text-base-content/60" />;
const RECENT_FILE_ICON = (
  <FileText className="size-4 shrink-0 text-base-content/50" />
);
const EMPTY_FILE_ICON = (
  <FileText className="size-4 shrink-0 text-base-content/35" />
);

function splitPath(path: string): { name: string; dir: string } {
  const sep = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return sep < 0
    ? { name: path, dir: "" }
    : { name: path.slice(sep + 1), dir: path.slice(0, sep) || "/" };
}

export const WelcomeView = memo(function WelcomeView({
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
      className="grid h-full grid-cols-1 overflow-auto bg-base-100 text-base-content md:grid-cols-[minmax(360px,44%)_1fr] md:overflow-hidden"
      lang={lang === "zh" ? "zh-CN" : "en"}
    >
      <main className="flex min-h-0 min-w-0 flex-col p-8 md:overflow-y-auto md:p-10 lg:p-12">
        <div className="mx-auto my-auto flex w-full max-w-md flex-col gap-8">
          <header className="flex flex-col gap-5">
            <div className="flex flex-col gap-2">
              <h1 className="text-5xl font-black uppercase leading-none tracking-tighter md:text-6xl">
                WEFT
              </h1>
              <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-base-content/55">
                <span className="inline-flex items-center gap-1.5">
                  {BOOK_ICON}
                  {strings[lang].tab1}
                </span>
                <span className="inline-flex items-center gap-1.5">
                  {WAYPOINTS_ICON}
                  {strings[lang].tab2}
                </span>
                <span className="inline-flex items-center gap-1.5">
                  {GIT_BRANCH_ICON}
                  {strings[lang].tab4}
                </span>
              </div>
            </div>

            <button
              type="button"
              className="btn btn-primary self-start"
              onClick={onOpen}
              onMouseEnter={onPreload}
              onFocus={onPreload}
            >
              {FOLDER_OPEN_ICON}
              {strings[lang].open}
            </button>
          </header>

          <section className="flex min-w-0 flex-col gap-3 border-t border-base-300 pt-6">
            <header className="flex flex-col gap-0.5">
              <h2 className="flex items-center gap-2 text-sm font-semibold">
                {HISTORY_ICON}
                {strings[lang].recent}
              </h2>
              <p className="text-xs text-base-content/60">
                {strings[lang].recentDescription}
              </p>
            </header>
            {recents.length > 0 ? (
              <ul className="-mx-2 flex flex-col gap-0.5">
                {recents.map((p) => {
                  const { name, dir } = splitPath(p);
                  return (
                    <li key={p} className="min-w-0">
                      <button
                        type="button"
                        title={p}
                        className="btn btn-ghost h-auto w-full justify-start gap-3 px-2 py-2 text-left font-normal"
                        onClick={() => onRecent(p)}
                        onMouseEnter={onPreload}
                        onFocus={onPreload}
                      >
                        {RECENT_FILE_ICON}
                        <span className="flex min-w-0 flex-col items-start">
                          <strong className="max-w-full truncate text-sm font-medium">
                            {name}
                          </strong>
                          {dir ? (
                            <small className="max-w-full truncate font-mono text-xs font-normal text-base-content/50">
                              {dir}
                            </small>
                          ) : null}
                        </span>
                      </button>
                    </li>
                  );
                })}
              </ul>
            ) : (
              <div className="flex items-center gap-2.5 py-3 text-xs text-base-content/50">
                {EMPTY_FILE_ICON}
                <p>{strings[lang].recentEmpty}</p>
              </div>
            )}
          </section>
        </div>
      </main>

      <aside
        className="flex min-h-[320px] items-center justify-center overflow-hidden border-t border-base-300 bg-base-200 p-8 md:min-h-0 md:border-t-0 md:border-l md:p-12"
        aria-hidden="true"
      >
        <div className="aspect-[520/440] w-full max-w-[680px] md:aspect-auto md:h-full">
          <WeftPrint />
        </div>
      </aside>
    </div>
  );
});
