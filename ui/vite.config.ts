// @ts-ignore
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'node:path'

const host = process.env.TAURI_DEV_HOST

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],

  // Vite options tailored for Tauri development
  clearScreen: false,
  root: 'src',

  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },

  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: [
        '../../src-web/**',
        '**/.*/**',
        '**/node_modules/**',
        '**/dist/**',
      ],
    },
    proxy: {
      '/api/': {
        target: 'http://localhost:3000/',
        changeOrigin: true,
      },
    },
    allowedHosts: true
  },

  build: {
    outDir: '../../dist/ui',
  },
}))
