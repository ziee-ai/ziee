/// <reference types="vite/client" />

// Vite's import.meta.glob types
interface ImportMeta {
  glob: <T = unknown>(
    pattern: string | string[],
    options?: {
      eager?: boolean
      import?: string
      as?: string
    },
  ) => Record<string, T>
}
