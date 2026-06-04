import { Fragment, createContext, useContext } from 'react'
import type {
  KnowledgeView,
  ProjectSlotName,
} from '@/modules/projects/core/extensions/types'
import { projectExtensionRegistry } from '@/modules/projects/core/extensions/registry'

/**
 * Renders all extension components registered for a given project slot.
 *
 * For `knowledge_kinds`, `view` selects which sub-component each
 * contribution provides:
 *   - "inlinePreview" → the project detail page's knowledge card
 *   - "managePanel" → the knowledge drawer
 *
 * Extensions read `Stores.ProjectDetail.project` directly for context
 * (mirrors how chat extensions read `Stores.Chat.conversation`). No
 * props are passed to slot components.
 */
interface ProjectExtensionSlotProps {
  name: ProjectSlotName
  /** Required for `knowledge_kinds` (selects inlinePreview vs managePanel),
   *  ignored for `advanced_settings` (panels are self-contained). */
  view?: KnowledgeView
  className?: string
  fallback?: React.ReactNode
}

export function ProjectExtensionSlot({
  name,
  view,
  className,
  fallback,
}: ProjectExtensionSlotProps) {
  const renderers = projectExtensionRegistry.renderSlot(name, view)
  if (renderers.length === 0) {
    return fallback ? <>{fallback}</> : null
  }
  const dataAttr = view ? `${name}:${view}` : name
  return (
    <div className={className} data-project-extension-slot={dataAttr}>
      {renderers.map((node, idx) => (
        <Fragment key={`${name}-${view ?? 'default'}-${idx}`}>{node}</Fragment>
      ))}
    </div>
  )
}

/**
 * React context exposing a callback that opens the project knowledge
 * drawer. Inline-preview components can call `useOpenManageDrawer()` to
 * trigger drawer-open without coupling to the projects module's internal
 * drawer state — the host (`<ProjectKnowledgeSection>`) provides the
 * implementation.
 */
const DrawerOpenerContext = createContext<(() => void) | null>(null)

export function DrawerOpenerProvider({
  open,
  children,
}: {
  open: () => void
  children: React.ReactNode
}) {
  return (
    <DrawerOpenerContext.Provider value={open}>
      {children}
    </DrawerOpenerContext.Provider>
  )
}

/**
 * Returns a callback that opens the manage drawer, or a no-op if used
 * outside `<DrawerOpenerProvider>`. The no-op behavior keeps slot
 * components renderable in isolation (e.g. Storybook, unit tests).
 */
export function useOpenManageDrawer(): () => void {
  const open = useContext(DrawerOpenerContext)
  return open ?? (() => {})
}
