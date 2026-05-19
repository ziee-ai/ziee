import { Stores } from '@/core/stores'
import { AnnotationDrawer } from './AnnotationDrawer'

export function AnnotationDrawerPortal() {
  const { openAnnotation, setOpenAnnotation } = Stores.Chat.McpStore
  return (
    <AnnotationDrawer
      annotation={openAnnotation}
      onClose={() => setOpenAnnotation(null)}
    />
  )
}
