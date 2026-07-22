import { lazy } from 'react'
import type { ComponentType, ReactNode } from 'react'
import type { LayoutDefinition } from '@/modules/router/types'

/**
 * The settings layout DEFINITION — deliberately light. Its `component` is a lazy
 * ref to {@link SettingsLayoutView} (the settings/app shell), so a module
 * referencing `SettingsLayoutDef` — auth (a CORE module, via /settings/sessions)
 * AND every `/settings/*` module — does NOT drag the whole shell
 * (HeaderBarContainer, LeftSidebar, SettingsPage, …) into the boot payload. The
 * shell loads (Suspense-wrapped by RouterComponent) only when a settings route
 * actually renders — the routes are all authenticated.
 */
const SettingsLayoutView = lazy(() => import('./SettingsLayoutView'))

export const SettingsLayoutDef: LayoutDefinition<undefined> = {
  component: SettingsLayoutView as unknown as ComponentType<{
    children: ReactNode
  }>,
  mergeOptions: () => undefined,
}
