import { forwardRef, lazy, Suspense } from 'react'
import { Spin } from '@ziee/kit'
import type { CanvasEditorHandle } from './types'

/**
 * Lazy boundary for the Plate editor bundle (Plate + slate + remark). Mirrors
 * `LazyStreamdown`: view-only users never load it — it loads only when the
 * canvas enters edit mode. The imperative `getMarkdown()` ref is forwarded
 * through to the underlying editor.
 */
const Inner = lazy(() =>
  import('./KitMarkdownEditor').then(m => ({ default: m.KitMarkdownEditor })),
)

interface LazyMarkdownEditorProps {
  initialMarkdown: string
  onDirty?: () => void
}

export const LazyMarkdownEditor = forwardRef<
  CanvasEditorHandle,
  LazyMarkdownEditorProps
>(function LazyMarkdownEditor(props, ref) {
  return (
    <Suspense
      fallback={
        <div className="flex h-full items-center justify-center">
          <Spin label="Loading editor" />
        </div>
      }
    >
      <Inner {...props} ref={ref} />
    </Suspense>
  )
})
