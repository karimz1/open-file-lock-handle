import {
  Children,
  isValidElement,
  useEffect,
  useRef,
  type ReactNode,
} from "react";
import { X } from "lucide-react";
import { t } from "./i18n";
export function Modal({
  title,
  children,
  close,
  danger = false,
  className = "",
}: {
  title: string;
  children: ReactNode;
  close: () => void;
  danger?: boolean;
  className?: string;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  // The body scrolls in small windows; the title and buttons stay in view.
  const body: ReactNode[] = [];
  const actions: ReactNode[] = [];
  for (const child of Children.toArray(children)) {
    const name = isValidElement<{ className?: unknown }>(child)
      ? child.props.className
      : undefined;
    const isActions =
      typeof name === "string" && name.split(" ").includes("modal-actions");
    (isActions ? actions : body).push(child);
  }
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
      className={`modal${danger ? " danger-modal" : ""}${className ? ` ${className}` : ""}`}
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
          aria-label={t("app.k_close_dialog")}
          onClick={close}
        >
          <X size={17} />
        </button>
      </header>
      <div className="modal-body">{body}</div>
      {actions}
    </dialog>
  );
}
