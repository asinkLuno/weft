import {
  BookOpen,
  FileText,
  FolderOpen,
  GitBranch,
  History,
  Waypoints,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { WeftMark, WeftPrint } from "@/components/weft-art";
import { type Lang, strings } from "@/i18n";
import "./welcome.css";

export function WelcomeView({
  lang,
  recents,
  onOpen,
  onRecent,
}: {
  lang: Lang;
  recents: string[];
  onOpen: () => void;
  onRecent: (path: string) => void;
}) {
  return (
    <main className="welcome" lang={lang === "zh" ? "zh-CN" : "en"}>
      <header className="welcome-masthead">
        <div className="brand">
          <WeftMark />
          <span>weft</span>
        </div>
        <span className="welcome-workspace">{strings[lang].workspace}</span>
      </header>
      <div className="welcome-layout">
        <section className="welcome-intro">
          <h1>{strings[lang].welcomeTitle}</h1>
          <p>{strings[lang].welcomeDescription}</p>
          <Button size="lg" onClick={onOpen}>
            <FolderOpen />
            {strings[lang].open}
          </Button>
        </section>
        <div className="welcome-print" aria-hidden="true">
          <span className="welcome-wordmark">weft</span>
          <WeftPrint />
          <div className="welcome-print-legend">
            <span>
              <BookOpen />
              {strings[lang].tab1}
            </span>
            <span>
              <Waypoints />
              {strings[lang].tab2}
            </span>
            <span>
              <GitBranch />
              {strings[lang].tab4}
            </span>
          </div>
        </div>
        <Card className="recent-card">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <History className="size-4" />
              {strings[lang].recent}
            </CardTitle>
            <CardDescription>{strings[lang].recentDescription}</CardDescription>
          </CardHeader>
          <CardContent>
            {recents.length > 0 ? (
              <ul className="recent-list">
                {recents.map((p) => (
                  <li key={p}>
                    <Button
                      variant="ghost"
                      className="recent-file"
                      onClick={() => onRecent(p)}
                    >
                      <FileText className="size-5 text-primary" />
                      <span>
                        <strong>{p.split(/[\\/]/).pop()}</strong>
                        <small>{p}</small>
                      </span>
                    </Button>
                  </li>
                ))}
              </ul>
            ) : (
              <div className="recent-empty">
                <FileText className="size-5" aria-hidden="true" />
                <p>{strings[lang].recentEmpty}</p>
              </div>
            )}
          </CardContent>
        </Card>
      </div>
      <p className="welcome-note">{strings[lang].fileHint}</p>
    </main>
  );
}
