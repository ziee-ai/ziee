import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { SandboxEnvironmentsSection } from './SandboxEnvironmentsSection'
import { SandboxResourceLimitsSection } from './SandboxResourceLimitsSection'

/**
 * Code Sandbox admin settings — single page combining the two operational
 * surfaces (rootfs environments + resource limits) under one route. Each
 * sub-concern is its own `<Card>` section, mirroring the codebase's
 * established pattern in `HardwareSettings` (OS / CPU / Memory / GPU cards
 * on one Hardware page).
 *
 * The two underlying stores (`SandboxEnvironments`, `SandboxResourceLimits`)
 * stay separate because their async shapes diverge sharply — environments
 * runs SSE subscriptions + per-flavor in-flight tracking; limits is a plain
 * singleton GET/PUT. Merging them would force the limits flow to import
 * machinery it doesn't need.
 *
 * Permission gating happens per-section (each section reads `Stores.Auth`
 * itself and renders a permission-denied alert or read-only form as
 * appropriate); the page-level container is just a heading + container.
 */
export function SandboxSettingsPage() {
  return (
    <SettingsPageContainer
      title="Code Sandbox"
      subtitle="Manage rootfs environments and the runtime resource caps applied to every execute_command."
    >
      <SandboxEnvironmentsSection />
      <SandboxResourceLimitsSection />
    </SettingsPageContainer>
  )
}
