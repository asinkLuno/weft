import { useEffect } from "react";
import { toast } from "sonner";
import { formatDocumentError, type Lang } from "@/i18n";

export function DocumentErrorToast({
  error,
  lang,
}: {
  error: unknown;
  lang: Lang;
}) {
  useEffect(() => {
    if (error === null) return;
    const id = toast.error(
      <div className="whitespace-pre-line">
        {formatDocumentError(error, lang)}
      </div>,
      { duration: Infinity, closeButton: true },
    );
    return () => {
      toast.dismiss(id);
    };
  }, [error, lang]);
  return null;
}
