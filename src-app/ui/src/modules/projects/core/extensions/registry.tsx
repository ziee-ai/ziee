import type { PermissionExpr } from '@/core/permissions'
import type {
  AdvancedSettingsContribution,
  KnowledgeKindContribution,
  KnowledgeView,
  ProjectExtensionRegistration,
  ProjectSlotName,
} from '@/modules/projects/core/extensions/types'

/**
 * A rendered project-extension slot entry: the node plus the optional
 * permission that gates it. `<ProjectExtensionSlot>` filters on
 * `permission` reactively before rendering, so a user who lacks the grant
 * never sees the contribution (header + body).
 */
export interface RenderedSlotEntry {
  node: React.ReactNode
  permission?: PermissionExpr
}

/**
 * Lightweight registry for project-extension contributions.
 *
 * Mirrors `ChatExtensionRegistry` in spirit but stripped to just slots
 * + register + renderSlot. Project extensions don't stream, don't have
 * SSE, don't have per-conversation hooks — they contribute UI
 * components and that's it.
 *
 * Acid-test invariant: if no module registers via `register(...)`, the
 * registry returns empty arrays from `renderSlot` and the project page
 * renders fine (each slot host treats missing contributions as "no
 * knowledge of this kind yet").
 */
export class ProjectExtensionRegistry {
  /**
   * extension name → registration record. Keyed by name so re-registration
   * (e.g. HMR) silently replaces an entry instead of stacking duplicates.
   */
  private extensions: Map<string, ProjectExtensionRegistration> = new Map()

  register(registration: ProjectExtensionRegistration): void {
    if (this.extensions.has(registration.name)) {
      console.warn(
        `[ProjectExtensions] Re-registering "${registration.name}" — previous registration replaced (likely HMR).`,
      )
    }
    this.extensions.set(registration.name, registration)
    console.log(`[ProjectExtensions] Registered: ${registration.name}`)
  }

  /**
   * Iterate all `knowledge_kinds` contributions, sorted by `order`
   * (ascending; default 100). Returns the contribution objects so the
   * `<ProjectExtensionSlot>` component can select the right sub-component
   * based on the requested view.
   */
  knowledgeKinds(): Array<KnowledgeKindContribution & { extensionName: string }> {
    const out: Array<KnowledgeKindContribution & { extensionName: string }> = []
    for (const registration of this.extensions.values()) {
      const c = registration.slots.knowledge_kinds
      if (c) {
        out.push({ ...c, extensionName: registration.name })
      }
    }
    out.sort((a, b) => (a.order ?? 100) - (b.order ?? 100))
    return out
  }

  /**
   * Iterate all `advanced_settings` contributions, sorted by `order`.
   * Each contribution renders as its own self-contained card.
   */
  advancedSettings(): Array<
    AdvancedSettingsContribution & { extensionName: string }
  > {
    const out: Array<AdvancedSettingsContribution & { extensionName: string }> =
      []
    for (const registration of this.extensions.values()) {
      const c = registration.slots.advanced_settings
      if (c) {
        out.push({ ...c, extensionName: registration.name })
      }
    }
    out.sort((a, b) => (a.order ?? 100) - (b.order ?? 100))
    return out
  }

  /**
   * Generic slot renderer — returns the React components for a given
   * slot + view, in render order. Used by `<ProjectExtensionSlot>`.
   *
   * For `knowledge_kinds`, `view` selects `inlinePreview` or `managePanel`.
   * For `advanced_settings`, `view` is ignored (panels are self-contained).
   *
   * Each entry carries its contribution's optional `permission` so the
   * caller (`<ProjectExtensionSlot>`) can filter reactively — gating is a
   * render-time concern (needs the reactive auth store), so it is NOT done
   * here.
   */
  renderSlot(name: ProjectSlotName, view?: KnowledgeView): RenderedSlotEntry[] {
    if (name === 'knowledge_kinds') {
      if (!view) return []
      return this.knowledgeKinds().map((c, idx) => {
        const Component =
          view === 'inlinePreview' ? c.inlinePreview : c.managePanel
        return {
          node: <Component key={`${c.extensionName}-${view}-${idx}`} />,
          permission: c.permission,
        }
      })
    }
    if (name === 'advanced_settings') {
      return this.advancedSettings().map((c, idx) => {
        const Component = c.panel
        return {
          node: <Component key={`${c.extensionName}-panel-${idx}`} />,
          permission: c.permission,
        }
      })
    }
    return []
  }
}

/**
 * Process-wide singleton. Auto-discovery import.meta.glob runs once at
 * app boot and registers each sibling-module extension into this.
 */
export const projectExtensionRegistry = new ProjectExtensionRegistry()
