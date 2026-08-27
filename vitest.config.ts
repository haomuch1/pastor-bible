import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// The frontend had no tests until 2026-08-27, and it shipped a history sidebar
// with no delete control on it for a whole session while the command underneath
// was written, registered and covered by a passing cargo test. A test that
// renders the component is the only kind that would have caught that.
//
// jsdom does no layout: every getBoundingClientRect is zero, so a test here
// cannot tell a visible button from a collapsed one. What it can prove is that
// the control exists, is reachable by its accessible name, and does what it
// says. The pixels are measured in a real browser and recorded in HANDOFF.
export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
    include: ["src/**/*.test.tsx", "src/**/*.test.ts"],
    restoreMocks: true,
  },
});
