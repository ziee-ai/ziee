/**
 * Common imperative handle for canvas editors (markdown WYSIWYG, code). The host
 * reads the current content only on explicit Save via `getContent()`, so no
 * serialization runs per keystroke.
 */
export interface CanvasEditorHandle {
  getContent: () => string
}
