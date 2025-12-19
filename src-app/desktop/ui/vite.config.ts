import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'
import { formNamesPlugin } from './plugins/vite-plugin-form-names.js'
import { removeDataTestPlugin } from './plugins/vite-plugin-remove-data-test.js'

const host = process.env.TAURI_DEV_HOST

// https://vitejs.dev/config/
export default defineConfig(async () => {
  const isDev = process.env.NODE_ENV !== 'production'
  const isTest = process.env.NODE_ENV === 'test'

  return {
    plugins: [
      react({
        babel: {
          plugins: [['babel-plugin-react-compiler', {}]],
        },
      }),
      tailwindcss(),
      // Detect duplicate form names
      formNamesPlugin({
        srcDir: 'src',
      }),
      // Remove data-test-* attributes in production builds
      ...(isDev || isTest ? [] : [removeDataTestPlugin()]),
    ],

    // Vite options tailored for Tauri development
    clearScreen: false,
    root: 'src',

    resolve: {
      alias: {
        // Override getBaseURL for desktop - MUST come before @ alias
        // Calls Tauri backend for dynamic port instead of using window.location.origin
        '@/api-client/getBaseURL': path.resolve(
          __dirname,
          './src/modules/desktop-base/getBaseURL.ts',
        ),
        // Use desktop's own generated types (includes desktop endpoints)
        '@/api-client/types': path.resolve(
          __dirname,
          './src/api-client/types.ts',
        ),
        '@ziee/desktop': path.resolve(__dirname, './src'),
        '@': path.resolve(__dirname, '../../ui/src'),
        // Resolve @ziee/ui-core to core UI source files
        '@ziee/ui-core': path.resolve(__dirname, '../../ui/src'),
        // Force resolve packages from desktop UI's node_modules
        // This is needed because core UI files import these and resolver looks relative to their location
        'react-icons': path.resolve(__dirname, './node_modules/react-icons'),
        'react-markdown': path.resolve(
          __dirname,
          './node_modules/react-markdown',
        ),
        'react-use': path.resolve(__dirname, './node_modules/react-use'),
        'overlayscrollbars/overlayscrollbars.css': path.resolve(
          __dirname,
          './node_modules/overlayscrollbars/styles/overlayscrollbars.css',
        ),
        overlayscrollbars: path.resolve(
          __dirname,
          './node_modules/overlayscrollbars',
        ),
        'overlayscrollbars-react': path.resolve(
          __dirname,
          './node_modules/overlayscrollbars-react',
        ),
        mermaid: path.resolve(__dirname, './node_modules/mermaid'),
        katex: path.resolve(__dirname, './node_modules/katex'),
        'highlight.js': path.resolve(__dirname, './node_modules/highlight.js'),
        dayjs: path.resolve(__dirname, './node_modules/dayjs'),
        tinycolor2: path.resolve(__dirname, './node_modules/tinycolor2'),
        immer: path.resolve(__dirname, './node_modules/immer'),
      },
      // Ensure shared dependencies are resolved from desktop UI's node_modules
      // This is needed because core UI source files import these packages
      dedupe: [
        'react',
        'react-dom',
        'react-router-dom',
        'zustand',
        'antd',
        '@ant-design/icons',
        'i18next',
        'react-i18next',
        'react-icons',
        'react-markdown',
        'react-use',
        'dayjs',
        'immer',
        'tinycolor2',
        'overlayscrollbars',
        'overlayscrollbars-react',
      ],
    },

    // Tauri expects a fixed port
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
        // Tell vite to ignore watching `src-tauri`
        ignored: ['**/src-tauri/**', '**/.*/**', '**/node_modules/**'],
      },
      // Proxy API requests to backend (for development without Tauri)
      allowedHosts: true,
    },

    build: {
      outDir: '../dist',
    },
  }
})
