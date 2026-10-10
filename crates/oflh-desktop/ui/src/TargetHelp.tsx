import { useEffect, useRef } from "react";
import { Info } from "lucide-react";
import { t } from "./i18n";

/** Target guidance stays available without taking space below the path field. */
export function TargetHelp() {
  const help = useRef<HTMLDetailsElement>(null);
  const hint = t("inspection.k_drag_a_file_or_folder_onto_this_window_hint");
  useEffect(() => {
    const closeOutside = (event: Event) => {
      if (help.current && !help.current.contains(event.target as Node))
        help.current.open = false;
    };
    document.addEventListener("pointerdown", closeOutside);
    document.addEventListener("focusin", closeOutside);
    return () => {
      document.removeEventListener("pointerdown", closeOutside);
      document.removeEventListener("focusin", closeOutside);
    };
  }, []);
  return (
    <>
      <span className="sr-only" id="target-path-description">
        {hint}
      </span>
      <details
        className="target-help"
        ref={help}
        onKeyDownCapture={(event) => {
          if (event.key === "Escape" && help.current?.open) {
            event.preventDefault();
            event.stopPropagation();
            help.current.open = false;
            help.current.querySelector("summary")?.focus();
          }
        }}
      >
        <summary
          aria-label={t("inspection.k_target_selection_help")}
          title={hint}
        >
          <Info size={15} aria-hidden="true" />
        </summary>
        <div role="note">{hint}</div>
      </details>
    </>
  );
}
