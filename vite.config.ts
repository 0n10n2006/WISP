/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const host = process.env.TAURI_DEV_HOST;

// Preact's React-compatible layer: the same components and hooks at a
// fraction of React DOM's size, so every Wisp window loads and runs lighter.
const preactAliases = [
  { find: /^react-dom\/test-utils$/, replacement: "preact/test-utils" },
  { find: /^react-dom\/client$/, replacement: "preact/compat/client" },
  { find: /^react-dom$/, replacement: "preact/compat" },
  { find: /^react\/jsx-runtime$/, replacement: "preact/jsx-runtime" },
  { find: /^react\/jsx-dev-runtime$/, replacement: "preact/jsx-dev-runtime" },
  { find: /^react$/, replacement: "preact/compat" },
];

export default defineConfig({
  plugins: [react()],
  resolve: { alias: preactAliases },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    watch: { ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    target: "es2022",
    sourcemap: false,
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./tests/setup.ts"],
    // Tests render through Preact too, so they exercise what ships.
    server: { deps: { inline: [/zustand/, /@testing-library\/preact/] } },
    include: ["tests/unit/**/*.test.{ts,tsx}", "tests/integration/**/*.test.{ts,tsx}"],
  },
});
