import { useEffect, useRef, useState } from "react";
import { ChevronDown, Info, Timer } from "lucide-react";
import { t } from "./i18n";
const intervals = [0, 5, 10, 15, 30, 60];

/** App-rendered interval list avoids OS-native popup colors and font sizes. */
export function AutoRefresh({
  value,
  onChange,
}: {
  value: number;
  onChange: (value: number) => void;
}) {
  const selector = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const help = useRef<HTMLDetailsElement>(null);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(intervals.indexOf(value));
  const label = (seconds: number) =>
    seconds === 0 ? t("common.k_off") : `${seconds}s`;
  const close = (restoreFocus = false) => {
    setOpen(false);
    if (restoreFocus) trigger.current?.focus();
  };
  const choose = (index: number) => {
    onChange(intervals[index]);
    close(true);
  };
  useEffect(() => {
    if (open) list.current?.focus();
  }, [open]);
  useEffect(() => {
    const outside = (event: Event) => {
      const target = event.target as Node;
      if (!selector.current?.contains(target)) setOpen(false);
      if (help.current && !help.current.contains(target))
        help.current.open = false;
    };
    document.addEventListener("pointerdown", outside);
    document.addEventListener("focusin", outside);
    return () => {
      document.removeEventListener("pointerdown", outside);
      document.removeEventListener("focusin", outside);
    };
  }, []);
  return (
    <div
      className="auto-refresh"
      onKeyDownCapture={(event) => {
        if (event.key === "Escape" && (open || help.current?.open)) {
          event.preventDefault();
          event.stopPropagation();
          if (open) close(true);
          if (help.current?.open) {
            help.current.open = false;
            help.current.querySelector("summary")?.focus();
          }
        }
      }}
    >
      <div className="auto-refresh-control">
        <Timer size={14} />
        <span>{t("app.k_auto")}</span>
        <div className="refresh-selector" ref={selector}>
          <button
            ref={trigger}
            className="refresh-selector-trigger"
            aria-label={t("status.k_automatic_refresh_interval")}
            aria-haspopup="listbox"
            aria-expanded={open}
            aria-controls={open ? "refresh-intervals" : undefined}
            onClick={() => {
              setActive(intervals.indexOf(value));
              setOpen(!open);
            }}
            onKeyDown={(event) => {
              if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
                event.preventDefault();
                event.stopPropagation();
                setActive(
                  event.key === "Home"
                    ? 0
                    : event.key === "End"
                      ? intervals.length - 1
                      : intervals.indexOf(value),
                );
                setOpen(true);
              }
            }}
          >
            {label(value)}
            <ChevronDown size={14} />
          </button>
          {open && (
            <div
              id="refresh-intervals"
              ref={list}
              role="listbox"
              className="refresh-intervals"
              tabIndex={-1}
              aria-label={t("status.k_automatic_refresh_interval")}
              aria-activedescendant={`refresh-interval-${active}`}
              onKeyDown={(event) => {
                if (
                  ["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)
                ) {
                  event.preventDefault();
                  event.stopPropagation();
                  setActive((current) =>
                    event.key === "Home"
                      ? 0
                      : event.key === "End"
                        ? intervals.length - 1
                        : (current +
                            (event.key === "ArrowDown" ? 1 : -1) +
                            intervals.length) %
                          intervals.length,
                  );
                } else if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  event.stopPropagation();
                  choose(active);
                } else if (event.key === "Tab") close(true);
                else {
                  const match = intervals.findIndex((seconds) =>
                    label(seconds)
                      .toLowerCase()
                      .startsWith(event.key.toLowerCase()),
                  );
                  if (event.key.length === 1 && match >= 0) {
                    event.preventDefault();
                    event.stopPropagation();
                    setActive(match);
                  }
                }
              }}
            >
              {intervals.map((seconds, index) => (
                <div
                  key={seconds}
                  id={`refresh-interval-${index}`}
                  role="option"
                  aria-selected={seconds === value}
                  className={active === index ? "highlighted" : ""}
                  onPointerMove={() => setActive(index)}
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => choose(index)}
                >
                  {label(seconds)}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
      <details className="auto-refresh-info" ref={help}>
        <summary aria-label={t("status.k_automatic_refresh_information")}>
          <Info size={15} />
        </summary>
        <div role="note">
          <strong>{t("status.k_automatic_refresh")}</strong>
          <p>
            {t("inspection.k_off_by_default_choose_an_interval_to_re_fb2f1749")}
          </p>
          <p>{t("status.k_it_can_help_with_changing_processes_or_f5538b6f")}</p>
        </div>
      </details>
    </div>
  );
}
