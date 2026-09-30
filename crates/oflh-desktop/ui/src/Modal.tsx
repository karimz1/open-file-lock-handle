import { useEffect, useRef, type ReactNode } from "react";
import { X } from "lucide-react";
import { t } from "./i18n";
export function Modal({
  title,
  children,
  close,
  danger = false,
}: {
  title: string;
  children: ReactNode;
  close: () => void;
  danger?: boolean;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current!;
    const previous = document.activeElement as HTMLElement | null;
    dialog.showModal();
    dialog.querySelector<HTMLElement>("[data-default-focus]")?.focus();
    return () => {
      dialog.close();
      previous?.focus();
    };
  }, []);
  return (
    <dialog
      ref={ref}
      className={danger ? "modal danger-modal" : "modal"}
      aria-labelledby="dialog-title"
      onCancel={(event) => {
        event.preventDefault();
        close();
      }}
    >
      <header>
        <h2 id="dialog-title">{title}</h2>
        <button
          className="icon-button"
          aria-label={t("Close dialog")}
          onClick={close}
        >
          <X size={17} />
        </button>
      </header>
      {children}
    </dialog>
  );
}
