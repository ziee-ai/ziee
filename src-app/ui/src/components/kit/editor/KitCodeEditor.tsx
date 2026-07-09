import { forwardRef, useImperativeHandle, useRef } from 'react'
import CodeMirror, { EditorView } from '@uiw/react-codemirror'
import type { CanvasEditorHandle } from './types'

// Give the CodeMirror editable region an accessible name (its `role=textbox`
// otherwise has none — flagged by the a11y-name runtime gate).
const A11Y_LABEL = EditorView.contentAttributes.of({
  'aria-label': 'Code document editor',
})

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
          extensions={[A11Y_LABEL]}
          onChange={v => {
            textRef.current = v
            onDirty?.()
          }}
        />
      </div>
    )
  },
)
