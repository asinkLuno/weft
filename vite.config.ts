import path from "node:path";
import process from "node:process";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "./src"),
    },
  },
  build: {
    rolldownOptions: {
      // Tailwind 的 CSS 生成走 JS 回调，必然占满构建时间；这条提示只是噪音
      checks: { bundlerTimings: false },
      output: {
        codeSplitting: {
          groups: [
            {
              name: "zrender",
              test: /[\\/]node_modules[\\/]zrender[\\/]/,
              priority: 20,
            },
            {
              name: "echarts",
              test: /[\\/]node_modules[\\/]echarts[\\/]/,
              priority: 10,
            },
          ],
        },
      },
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
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
      // 3. tell Vite to ignore watching `src-tauri`
      // 4. 本应用会监听/编辑 .weft 文档与 .rn 插件，变更（含原子写临时文件）不能触发 vite full-reload（会冲掉前端状态退回欢迎页）
      ignored: ["**/src-tauri/**", "**/*.{weft,weft-tmp,rn}"],
    },
  },
}));
