import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [
    react({
      babel: {
        plugins: [['babel-plugin-react-compiler', {}]],
      },
    }),
  ],

  resolve: {
    alias: {
      // Resolve @ to core UI src (core UI uses this extensively)
      '@': path.resolve(__dirname, '../../ui/src'),
      // Resolve @ziee/ui-core to source files
      '@ziee/ui-core': path.resolve(__dirname, '../../ui/src'),
      // Override getBaseURL for desktop - calls Tauri backend for dynamic port
      '@ziee/ui-core/src/api-client/getBaseURL': path.resolve(
        __dirname,
        './src/modules/desktop-base/getBaseURL.ts'
      ),
    },
  },

  // Prevent vite from obscuring rust errors
  clearScreen: false,

  // Tauri expects a fixed port
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Tell vite to ignore watching `src-tauri`
      ignored: ['**/src-tauri/**'],
    },
  },
})
