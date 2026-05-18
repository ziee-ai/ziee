import { Streamdown } from 'streamdown'
import type { Annotation } from '@/api-client/types'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { useStreamdownComponents } from '@/modules/chat/core/utils/useStreamdownComponents'

export interface AnnotationDrawerProps {
  annotation: Annotation | null
  onClose: () => void
}

/**
 * Renders the drawer content for any annotation type.
 *
 * Currently renders all annotation types as markdown via Streamdown.
 *
 * TODO: Branch on `annotation_type` for type-specific rendering, e.g.:
 *   - "image"  → <img> tag
 *   - "audio"  → audio player
 *   - "file"   → download link
 */
export function AnnotationDrawer({ annotation, onClose }: AnnotationDrawerProps) {
  const components = useStreamdownComponents(annotation?.id ?? '')
  return (
    <Drawer
      title={annotation?.label ?? annotation?.annotation_type ?? 'Reference'}
      open={!!annotation}
      onClose={onClose}
      mask={false}
      size={480}
    >
      {annotation && (
        <Streamdown shikiTheme={['github-light', 'github-dark']} components={components}>
          {annotation.content}
        </Streamdown>
      )}
    </Drawer>
  )
}
