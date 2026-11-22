import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
export default defineConfig({
  // Fast Refresh is off under Playwright (OFLH_PLAYWRIGHT=1). A dropped
  // /@react-refresh request on a loaded Windows runner otherwise throws
  // "@vitejs/plugin-react can't detect preamble" and leaves a blank page
  // (GUI E2E / Windows on 50b37f8 and 1934f28).
  plugins: [
    react({ fastRefresh: process.env.OFLH_PLAYWRIGHT !== "1" }),
    tailwindcss(),
  ],
  clearScreen: false,
});
