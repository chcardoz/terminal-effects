import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const backend = process.env.TE_API_TARGET ? new URL(process.env.TE_API_TARGET) : null;
const proxy = backend
  ? Object.fromEntries(
      ["api", "media", "thumbnail", "filmstrip", "waveform", "frame"].map((route) => [
        `/${route}`,
        {
          target: backend.origin,
          changeOrigin: false,
          rewrite: (path: string) => `${backend.pathname.replace(/\/$/, "")}${path}`,
        },
      ]),
    )
  : undefined;

export default defineConfig({
  base: "./",
  plugins: [react()],
  server: { port: 5173, strictPort: true, proxy },
  build: {
    target: "chrome120",
    outDir: "dist",
    emptyOutDir: true,
    cssCodeSplit: false,
    rollupOptions: {
      output: {
        entryFileNames: "assets/app.js",
        chunkFileNames: "assets/[name].js",
        assetFileNames: (asset) =>
          asset.names.some((name) => name.endsWith(".css"))
            ? "assets/app.css"
            : "assets/[name][extname]",
      },
    },
  },
});
