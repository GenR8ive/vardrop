import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Panel dev sunucusu: /api çağrılarını vardrop-server'a yönlendirir.
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      "/api": {
        target: "http://127.0.0.1:8080",
        changeOrigin: true,
      },
    },
  },
  build: { outDir: "dist", sourcemap: false },
});
