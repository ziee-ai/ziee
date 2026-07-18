import { defineSyncpack } from '@ziee/config/syncpack'
export default defineSyncpack({
  source: ['package.json', 'src-app/ui/package.json', 'src-app/desktop/ui/package.json'],
  versionGroups: [
    { label: 'Desktop-only Tauri plugins (only desktop has them; do not require matching versions elsewhere)', dependencies: ['@tauri-apps/plugin-dialog', '@tauri-apps/plugin-notification'], packages: ['@ziee/desktop-ui'] },
    { label: "Test/db tooling only used by core UI's setup scripts (desktop now uses pg too for its test fixture)", dependencies: ['bcryptjs', '@types/bcryptjs', 'dotenv', 'uuid'], packages: ['@ziee/ui-core'] },
  ],
})
