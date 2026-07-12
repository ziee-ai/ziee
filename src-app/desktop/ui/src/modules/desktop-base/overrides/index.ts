/**
 * Desktop UI Override registrations — auto-discovery barrel.
 *
 * `registerDesktopOverrides()` is invoked SYNCHRONOUSLY from `main.tsx` before
 * `ReactDOM.render`, so every element-level desktop override (a `<Seam>` declared
 * in a core web component) is in the registry before the core component that
 * reads its seam first renders — the same pre-render window as
 * `Stores.AppMode.setMultiUserMode(false)`.
 *
 * Each sibling file under this dir owns ONE seam and exports a `register()`; they
 * are glob-discovered here (mirroring `desktop-loader.ts`) so a new conversion is
 * a single dropped file — no shared-registry edit. The `seam` codemod emits one
 * such file per migrated shadow.
 */
export function registerDesktopOverrides(): void {
  const files = import.meta.glob<{ register?: () => void }>(
    ['./*.tsx', './*.ts', '!./index.ts'],
    { eager: true },
  )
  for (const [path, mod] of Object.entries(files)) {
    if (typeof mod.register === 'function') {
      mod.register()
    } else {
      console.warn(`[Desktop overrides] ${path} has no register() export`)
    }
  }
}
