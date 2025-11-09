import type { LayoutDefinition } from '@/modules/router/types'
import { BlankLayout as BlankLayoutComponent } from './BlankLayout'

/**
 * BlankLayout - Simple layout with no chrome
 *
 * This layout has no layout options, just renders children.
 */
export const BlankLayout: LayoutDefinition<undefined> = {
  component: BlankLayoutComponent as any, // Cast to match LayoutDefinition signature
  mergeOptions: () => undefined,
}

// Re-export component
export { BlankLayoutComponent }
