import { useRef } from "react";
import { t } from "./i18n";

const MIN_WIDTH = 160;
const MAX_WIDTH = 480;
export const clampSidebarWidth = (
  width: number,
  viewport = window.innerWidth,
) =>
  Math.round(Math.max(MIN_WIDTH, Math.min(width, MAX_WIDTH, viewport * 0.45)));

export function readSidebarWidth(): number | null {
  try {
    const saved = Number(localStorage.getItem("oflh-sidebar-width"));
    return Number.isFinite(saved) && saved > 0
      ? clampSidebarWidth(saved)
      : null;
  } catch {
    return null;
  }
}

/** Resize navigation with pointer capture or arrow keys; retain collapse separately. */
export function SidebarResize({
  width,
  onChange,
}: {
  width: number | null;
  onChange: (width: number | null) => void;
}) {
  const drag = useRef<{ x: number; width: number } | null>(null);
  const save = (value: number | null) => {
    onChange(value);
    try {
      if (value === null) localStorage.removeItem("oflh-sidebar-width");
      else localStorage.setItem("oflh-sidebar-width", String(value));
    } catch {
      /* Keep the width for this session. */
    }
  };
  return (
    <div
      className="sidebar-resize"
      role="separator"
      tabIndex={0}
      aria-label={t("navigation.k_resize_sidebar")}
      aria-orientation="vertical"
      aria-valuemin={MIN_WIDTH}
      aria-valuemax={clampSidebarWidth(MAX_WIDTH)}
      aria-valuenow={width ?? undefined}
      onDoubleClick={() => save(null)}
      onPointerDown={(event) => {
        if (event.button !== 0) return;
        event.preventDefault();
        const actual =
          event.currentTarget.previousElementSibling?.getBoundingClientRect()
            .width;
        drag.current = { x: event.clientX, width: actual ?? width ?? 208 };
        event.currentTarget.setPointerCapture(event.pointerId);
      }}
      onPointerMove={(event) => {
        if (drag.current)
          save(
            clampSidebarWidth(
              drag.current.width + event.clientX - drag.current.x,
            ),
          );
      }}
      onPointerUp={(event) => {
        drag.current = null;
        if (event.currentTarget.hasPointerCapture(event.pointerId))
          event.currentTarget.releasePointerCapture(event.pointerId);
      }}
      onLostPointerCapture={() => {
        drag.current = null;
      }}
      onKeyDown={(event) => {
        if (event.key === "Home") {
          event.preventDefault();
          save(null);
        }
        if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
        event.preventDefault();
        const actual =
          event.currentTarget.previousElementSibling?.getBoundingClientRect()
            .width;
        save(
          clampSidebarWidth(
            (actual ?? width ?? 208) + (event.key === "ArrowRight" ? 16 : -16),
          ),
        );
      }}
    />
  );
}
