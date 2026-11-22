import { defineConfig, mergeConfig } from "vite";
import base from "./vite.config";

// Fast Refresh is off under Playwright (server.hmr: false). A dropped
// /@react-refresh request on a loaded Windows runner otherwise throws
// "@vitejs/plugin-react can't detect preamble" and leaves a blank page
// (GUI E2E / Windows on 50b37f8 and 1934f28).
export default mergeConfig(
  base,
  defineConfig({
    server: { hmr: false },
  }),
);
