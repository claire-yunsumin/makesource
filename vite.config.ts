import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Tauri는 고정 포트(1420)를 기대한다. TAURI_DEV_HOST가 있으면 모바일/원격 개발용 호스트로 노출.
const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  // Tauri 개발 시 Vite 콘솔을 지우지 않아 Rust 로그가 유지되도록 함
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // src-tauri 변경은 Rust 쪽에서 감시하므로 Vite watcher에서 제외
      ignored: ["**/src-tauri/**"],
    },
  },
  test: {
    environment: "node",
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
  },
});
