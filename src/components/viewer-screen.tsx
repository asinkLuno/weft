import { useTranslation } from "react-i18next";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

// Codes the backend can send (src-tauri/src/validate.rs); each needs a
// problems.<code> entry in the locales, which this union makes the compiler check.
export type ProblemCode =
  | "parse.syntax"
  | "structure.multiDoc"
  | "structure.topLevel"
  | "structure.sectionMapping"
  | "structure.missingKey"
  | "structure.unknownKey";

export type Problem = {
  code: ProblemCode;
  line: number;
  col: number;
  detail?: string;
};

export function ViewerScreen({
  path,
  problems,
}: {
  path: string;
  problems: Problem[];
}) {
  const { t } = useTranslation();

  return (
    <Card>
      <CardHeader>
        <CardTitle className="font-mono text-sm">{path}</CardTitle>
      </CardHeader>
      <CardContent>
        {problems.length === 0 ? (
          <p className="text-sm text-muted-foreground">{t("problems.none")}</p>
        ) : (
          <>
            <p className="mb-3 text-xs text-muted-foreground">
              {t("problems.title", { count: problems.length })}
            </p>
            <ul className="space-y-3">
              {problems.map((problem) => (
                <li key={`${problem.code}:${problem.line}:${problem.col}`}>
                  <div className="flex gap-2 text-sm">
                    <span className="shrink-0 pt-px font-mono text-destructive text-xs tabular-nums">
                      {problem.line}:{problem.col}
                    </span>
                    <span>{t(`problems.${problem.code}`)}</span>
                  </div>
                  {/* The parser's own words, verbatim and untranslated. */}
                  {problem.detail && (
                    <pre className="mt-1.5 rounded-sm bg-muted px-2 py-1.5 font-mono text-xs whitespace-pre-wrap">
                      {problem.detail}
                    </pre>
                  )}
                </li>
              ))}
            </ul>
          </>
        )}
      </CardContent>
    </Card>
  );
}
