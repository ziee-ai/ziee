import { forwardRef, lazy, Suspense } from 'react'
import { Spin } from '@ziee/kit'
import type { CanvasEditorHandle } from './types'

/** Lazy boundary for the CodeMirror bundle — loads only when a code deliverable
 *  enters edit mode. */
const Inner = lazy(() =>
  import('./KitCodeEditor').then(m => ({ default: m.KitCodeEditor })),
)

interface LazyCodeEditorProps {
  initialText: string
  onDirty?: () => void
}

export const LazyCodeEditor = forwardRef<
  CanvasEditorHandle,
  LazyCodeEditorProps
>(function LazyCodeEditor(props, ref) {
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
