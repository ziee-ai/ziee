/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_API_URL: string
  // Add more env variables as needed
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}

// Build-generated module manifest (vite-plugin-module-manifest). The cheap
// decision layer for smart module loading — see loader.ts.
declare module 'virtual:ziee-module-manifest' {
  import type { ModuleManifestEntry } from '@ziee/framework/module-system'
  export const manifest: ModuleManifestEntry[]
}
