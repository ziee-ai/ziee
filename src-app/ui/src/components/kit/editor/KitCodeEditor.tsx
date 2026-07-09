import { forwardRef, useImperativeHandle, useRef } from 'react'
import CodeMirror from '@uiw/react-codemirror'
import type { CanvasEditorHandle } from './types'

interface KitCodeEditorProps {
  /** Initial source text (the file's head content). */
  initialText: string
  /** Fired on the first edit so the host can flag the canvas dirty. */
  onDirty?: () => void
}

/**
 * A plain-text code editor (CodeMirror) for `code` deliverables. Code has NO
 * round-trip transform — the file content IS the source, edited + saved as-is —
 * so unlike the markdown canvas there is no fidelity risk. Lazy-loaded via
 * `LazyCodeEditor`. Reads current text on Save via the shared imperative handle.
 */
export const KitCodeEditor = forwardRef<CanvasEditorHandle, KitCodeEditorProps>(
  function KitCodeEditor({ initialText, onDirty }, ref) {
    const textRef = useRef(initialText)
    useImperativeHandle(ref, () => ({ getContent: () => textRef.current }), [])
    return (
      <div className="h-full w-full overflow-auto" data-testid="canvas-code-editor">
        <CodeMirror
          value={initialText}
          height="100%"
          onChange={v => {
            textRef.current = v
            onDirty?.()
          }}
        />
      </div>
    )
  },
)
