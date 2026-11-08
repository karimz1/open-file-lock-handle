import { Copy, ExternalLink } from "lucide-react";
import type { Row } from "./api";
import { t, tValue } from "./i18n";

/** Query-matching paths, grouped without merging their access or lock evidence. */
export function MatchingFiles({
  row,
  current,
  copy,
  reveal,
  showAll,
}: {
  row: Row;
  current: boolean;
  copy: (field: string, reference: string) => void;
  reveal: (reference: string) => void;
  showAll: () => void;
}) {
  const matches = row.matching_files;
  if (!matches) return null;
  return (
    <section className="matching-files" aria-label={t("matches.k_title")}>
      <h4>
        {t("matches.k_title")} <span className="muted">({matches.total})</span>
      </h4>
      <ul className="matching-file-list">
        {matches.files.map((file) => (
          <li key={file.path.reference}>
            <p className="detail-path mono">
              {file.path.display || t("common.k_unavailable_2c9c1f79")}
            </p>
            <div className="inline-actions">
              <button
                disabled={!current}
                onClick={() => copy("path", file.path.reference)}
              >
                <Copy size={13} /> {t("selection.k_copy_path")}
              </button>
              <button
                disabled={!current}
                onClick={() => reveal(file.path.reference)}
              >
                <ExternalLink size={13} /> {t("common.k_reveal")}
              </button>
            </div>
            <details className="matching-file-observations">
              <summary>
                {t(
                  file.references === 1
                    ? "matches.k_reference_one"
                    : "matches.k_references",
                  { n: file.references },
                )}
              </summary>
              <ul>
                {file.observations.map((observation, index) => (
                  <li key={index}>
                    <p>
                      {tValue(observation.relation)} ·{" "}
                      {tValue(observation.access)}
                    </p>
                    {observation.evidence_label && (
                      <p className="evidence">
                        {tValue(observation.evidence_label)}
                      </p>
                    )}
                    {observation.evidence && (
                      <p className="detail-path">{observation.evidence}</p>
                    )}
                    {observation.deleted && (
                      <p className="danger-text">
                        {t("table.k_file_was_deleted")}
                      </p>
                    )}
                  </li>
                ))}
              </ul>
              {file.more_references > 0 && (
                <p className="hint">
                  {t("matches.k_more_references", { n: file.more_references })}
                </p>
              )}
            </details>
          </li>
        ))}
      </ul>
      {matches.total > matches.files.length && (
        <p className="hint">
          {t("matches.k_more_files", {
            n: matches.total - matches.files.length,
          })}
        </p>
      )}
      <button className="inline-link" onClick={showAll}>
        {t("matches.k_show_all")}
      </button>
    </section>
  );
}
