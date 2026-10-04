import { useEffect, useState } from "react";
import { Copy, ExternalLink, FileSearch } from "lucide-react";
import { api, type Status } from "./api";
import { t } from "./i18n";
import { Modal } from "./Modal";

type SystemInfo = Awaited<ReturnType<typeof api.systemInfo>>;
const operatingSystems: Record<string, string> = {
  linux: "Linux",
  windows: "Windows",
  macos: "macOS",
};

/** Build details come from Rust; browser platform identifiers can be misleading. */
export function AboutDialog({
  status,
  close,
  report,
}: {
  status: Status;
  close: () => void;
  report: (error: unknown) => void;
}) {
  const [system, setSystem] = useState<SystemInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    let active = true;
    void api
      .systemInfo()
      .then((result) => {
        if (active) setSystem(result);
      })
      .catch(() => {})
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, []);
  const systemLabel = system
    ? `${operatingSystems[system.os] ?? system.os} ${system.arch}`
    : "";
  const copy = async () => {
    try {
      await api.copyDiagnostic(
        [
          "OFLH Desktop",
          `Version: ${status.version}`,
          ...(status.commit ? [`Commit: ${status.commit}`] : []),
          ...(systemLabel ? [`OS: ${systemLabel}`] : []),
          "License: MIT",
          "https://github.com/karimz1/open-file-lock-handle",
        ].join("\n"),
      );
      setCopied(true);
    } catch (error) {
      report(error);
    }
  };
  return (
    <Modal title={t("settings.k_about_oflh")} close={close}>
      <div className="about-dialog-heading">
        <FileSearch size={28} aria-hidden="true" />
        <strong>OFLH Desktop</strong>
      </div>
      <dl className="about-dialog-details">
        <dt>{t("about.k_version")}</dt>
        <dd>
          <button
            className="inline-link"
            onClick={() => void api.openInstalledRelease().catch(report)}
          >
            {status.version}
          </button>
        </dd>
        {status.commit && (
          <>
            <dt>{t("about.k_commit")}</dt>
            <dd className="mono">{status.commit}</dd>
          </>
        )}
        <dt>{t("about.k_system")}</dt>
        <dd>
          {systemLabel ||
            t(loading ? "app.k_loading" : "common.k_unavailable_2c9c1f79")}
        </dd>
        <dt>{t("about.k_license")}</dt>
        <dd>MIT</dd>
      </dl>
      <p>
        {t("app.k_independent_project_by")}{" "}
        <button
          className="inline-link"
          onClick={() => void api.openProfile().catch(report)}
        >
          Karim Zouine
        </button>
      </p>
      <div className="inline-actions about-dialog-links">
        <button
          className="inline-link"
          onClick={() => void api.openProject().catch(report)}
        >
          {t("app.k_view_project_on_github")}
          <ExternalLink size={13} />
        </button>
        {!/^\d+\.\d+\.\d+$/.test(status.version) && status.build_url && (
          <button
            className="inline-link"
            onClick={() => void api.openBuild().catch(report)}
          >
            {t("app.k_view_rc_pipeline")}
            <ExternalLink size={13} />
          </button>
        )}
      </div>
      {copied && (
        <p className="muted" role="status">
          {t("about.k_copied")}
        </p>
      )}
      <div className="modal-actions">
        <button disabled={loading} onClick={() => void copy()}>
          <Copy size={14} />
          {t("selection.k_copy")}
        </button>
        <button data-default-focus onClick={close}>
          {t("common.k_close")}
        </button>
      </div>
    </Modal>
  );
}
