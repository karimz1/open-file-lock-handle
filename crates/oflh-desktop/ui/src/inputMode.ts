/** Programmatic focus keeps keyboard and safe dialog defaults usable, but should
 * not look like a selected action when a menu or dialog opens from a pointer. */
export function trackInputMode(): () => void {
  const root = document.documentElement;
  root.dataset.inputMode = "pointer";
  const pointer = () => {
    root.dataset.inputMode = "pointer";
  };
  const keyboard = (event: KeyboardEvent) => {
    if (!["Control", "Meta", "Alt", "Shift"].includes(event.key))
      root.dataset.inputMode = "keyboard";
  };
  document.addEventListener("pointerdown", pointer, true);
  document.addEventListener("keydown", keyboard, true);
  return () => {
    document.removeEventListener("pointerdown", pointer, true);
    document.removeEventListener("keydown", keyboard, true);
  };
}
