// Runs synchronously before the document can paint or React loads.
(() => {
  let theme = "vscode";
  try {
    const saved = localStorage.getItem("oflh-theme");
    if (["light", "rider", "vscode", "purple", "system"].includes(saved)) {
      theme = saved;
    }
  } catch {
    // Storage can be unavailable; match the application's first-use default.
  }
  document.documentElement.dataset.theme =
    theme === "system"
      ? matchMedia("(prefers-color-scheme: dark)").matches
        ? "vscode"
        : "light"
      : theme;
})();
