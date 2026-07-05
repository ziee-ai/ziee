/**
 * Gallery coverage registry — the ENFORCED gate.
 *
 * `GALLERY_COVERAGE` maps EVERY generated `GallerySurface` (see
 * galleryCoverage.generated.ts) to how the gallery covers it. Because the object
 * is `satisfies Record<GallerySurface, Coverage>`, a surface with no entry is a
 * COMPILE error, and a stale key (deleted file) is also a compile error.
 *
 * Coverage kinds:
 *   - page(id)    — this surface IS a seeded gallery page (see ALL_PAGES).
 *   - story(id)   — covered by a kit story section (see stories/).
 *   - via(id)     — rendered inside another covered surface (its page/story).
 *   - allow(why)  — genuinely non-visual (provider/context/pure-logic/null
 *                   render) — no visual entry needed; the reason is reviewed.
 *   - pending(why)— tracked TODO: accounted for, not yet given a visual entry.
 *
 * The tsc gate guarantees every surface is at least `pending`; the parity test
 * (gen-gallery-coverage.mjs --check) lists pending surfaces so they stay visible.
 */
import type { GallerySurface } from './galleryCoverage.generated'

export type Coverage =
  | { kind: 'page'; pageId: string }
  | { kind: 'story'; storyId: string }
  | { kind: 'via'; surface: string }
  | { kind: 'allow'; reason: string }
  | { kind: 'pending'; reason: string }

export const page = (pageId: string): Coverage => ({ kind: 'page', pageId })
export const story = (storyId: string): Coverage => ({ kind: 'story', storyId })
export const via = (surface: string): Coverage => ({ kind: 'via', surface })
export const allow = (reason: string): Coverage => ({ kind: 'allow', reason })
export const pending = (reason: string): Coverage => ({ kind: 'pending', reason })

// Keep this object total over GallerySurface. The `// <<< scaffold-insert >>>`
// marker is where `gen:gallery-coverage --scaffold` appends missing surfaces as
// `pending(...)`; refine each into page/story/via/allow as coverage lands.
export const GALLERY_COVERAGE = {
  "modules/auth/AuthGuard": allow('non-visual — auth redirect guard'),
  "modules/hardware/HardwareMonitorButton": via('rendered in the app-layout header'),
  "modules/host-mount/conversation-extension/components/ConversationMountsControl": pending('interaction-only — conversation host-mount control (needs open-state entry)'),
  "modules/host-mount/pages/HostMountPolicyPage": page("modules/host-mount/pages/HostMountPolicyPage"),
  "modules/host-mount/project-extension/components/ProjectMountsPanel": via('rendered within the host-mount project extension panel'),
  "modules/host-mount/project-extension/extension": allow('non-visual — project-extension registration'),
  "modules/layouts/app-layout/components/Drawer": pending('interaction-only — desktop drawer primitive (needs open-state entry)'),
  "modules/layouts/app-layout/components/HeaderBarContainer": via('rendered within the app-layout chrome'),
  "modules/layouts/app-layout/components/LeftSidebar": via('rendered within the app-layout chrome'),
  "modules/layouts/app-layout/components/SidebarHeaderSpacer": via('rendered within the app-layout chrome'),
  "modules/layouts/app-layout/components/SidebarToggleButton": via('rendered within the app-layout chrome'),
  "modules/llm-provider/components/ProviderGroupAssignmentCard": via('rendered within the llm-provider settings page'),
  "modules/memory/pages/MemoryCombinedPage": page("modules/memory/pages/MemoryCombinedPage"),
  "modules/remote-access/pages/RemoteAccessPage": page("modules/remote-access/pages/RemoteAccessPage"),
  "modules/settings/SettingsPage": via('desktop settings layout shell (renders each settings page as an outlet)'),
  "modules/tunnel-auth/MagicLinkPage": page("modules/tunnel-auth/MagicLinkPage"),
  "modules/tunnel-auth/PhoneAuthPage": page("modules/tunnel-auth/PhoneAuthPage"),
  "modules/updater/components/UpdateBanner": via('rendered as a slot banner in the app-layout'),
  "modules/updater/pages/AboutPage": page("modules/updater/pages/AboutPage"),
  // <<< scaffold-insert >>>
} satisfies Record<GallerySurface, Coverage>

// ── Rollup counts (used by COVERAGE.md + the coverage report) ────────────────
export function coverageSummary() {
  const counts: Record<Coverage['kind'], number> = {
    page: 0,
    story: 0,
    via: 0,
    allow: 0,
    pending: 0,
  }
  for (const v of Object.values(GALLERY_COVERAGE) as Coverage[]) counts[v.kind]++
  const total = Object.keys(GALLERY_COVERAGE).length
  const covered = total - counts.pending
  return { total, covered, ...counts }
}
