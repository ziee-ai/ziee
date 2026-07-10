import { forwardRef, useImperativeHandle, useRef } from 'react'
import CodeMirror, { EditorView } from '@uiw/react-codemirror'
import type { CanvasEditorHandle } from './types'

// Give the CodeMirror editable region an accessible name (its `role=textbox`
// otherwise has none — flagged by the a11y-name runtime gate).
const A11Y_LABEL = EditorView.contentAttributes.of({
  'aria-label': 'Code document editor',
})

// Bind CodeMirror to the app's semantic color tokens so the plain-text canvas is
// theme-aware AND contrast-correct in BOTH themes. Without this the editor
// inherits the app foreground (near-white in one theme) onto its own light
// surface → a WCAG-AA contrast failure flagged by the runtime-health gate.
const CANVAS_THEME = EditorView.theme({
  '&': { backgroundColor: 'var(--background)', color: 'var(--foreground)' },
  '.cm-scroller': { backgroundColor: 'var(--background)' },
  '.cm-content': {
    color: 'var(--foreground)',
    caretColor: 'var(--foreground)',
  },
  '.cm-line': { color: 'var(--foreground)' },
  '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--foreground)' },
  '.cm-gutters': {
    backgroundColor: 'var(--muted)',
    color: 'var(--muted-foreground)',
    border: 'none',
  },
  '.cm-activeLine': { backgroundColor: 'var(--accent)' },
  '.cm-activeLineGutter': { backgroundColor: 'var(--accent)' },
  '.cm-selectionBackground, &.cm-focused .cm-selectionBackground, ::selection': {
    backgroundColor: 'var(--accent)',
  },
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
          // Disable the bundled light theme so CANVAS_THEME (bound to the app's
          // semantic tokens) fully controls the surface in BOTH themes —
          // otherwise the built-in white background survives under dark theme
          // (near-white text on white → a WCAG-AA contrast failure).
          theme="none"
          extensions={[A11Y_LABEL, CANVAS_THEME]}
          onChange={v => {
            textRef.current = v
            onDirty?.()
          }}
        />
      </div>
    )
  },
)
