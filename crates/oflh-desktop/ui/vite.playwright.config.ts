import { defineConfig, mergeConfig } from "vite";
import base from "./vite.config";

// Playwright drives one bundled development build served by `vite preview`,
// not the dev server. The dev server answers every source module with its own
// request (hundreds per page load, for every test), and on a loaded Windows
// runner single requests get dropped: first "/@react-refresh" (#218), then
// "net::ERR_NO_BUFFER_SPACE" for src/inputMode.ts, each leaving a blank page.
// A bundle needs a handful of requests per page load. Development mode keeps
// React's development build, import.meta.env.DEV and, with minify off, the
// component names the render-count tests read.
export default mergeConfig(
  base,
  defineConfig({
    // A build always runs with NODE_ENV=production, which selects React's
    // production build and turns import.meta.env.DEV off. Keep both as under
    // the dev server, so developer options and React's checks stay tested.
    define: {
      "process.env.NODE_ENV": JSON.stringify("development"),
      "import.meta.env.DEV": "true",
      "import.meta.env.PROD": "false",
    },
    build: {
      outDir: "playwright-bundle",
      emptyOutDir: true,
      minify: false,
      sourcemap: true,
    },
  }),
);
