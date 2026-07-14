import { createContext, useContext } from 'react'

/**
 * Per-pane scope for citation-highlight state (ITEM-49).
 *
 * The `PdfHighlight` target and the file find-query are keyed by `fileId`, which
 * COLLIDES when two split panes open a citation into the SAME document —
 * last-writer-wins clobbers the other pane's highlight, and one pane's unmount
 * cleanup wipes the sibling's. This scope, provided by whoever opens the doc in a
 * pane (the KB `kb_source` panel, which knows its pane id), is folded into the
 * storage key so each pane holds an INDEPENDENT highlight for the same fileId.
 *
 * Default `null` = no pane scope (the single-pane route, file attachments in a
 * message, the standalone KB search panel) → the key is the bare `fileId`, so the
 * stored/read keys are byte-identical to before the split.
 *
 * Lives in the FILE module so the leaf viewers (`pdfjs-body`, `FindableRegion`)
 * read it WITHOUT importing chat; the chat-side opener imports it to provide the
 * pane id (a kb→file dependency, the correct direction).
 */
export const FileHighlightScopeContext = createContext<string | null>(null)

/** The current pane scope for citation highlights (null outside a pane). */
export function useFileHighlightScope(): string | null {
  return useContext(FileHighlightScopeContext)
}

/** Compose the (scope, fileId) storage key. No scope → the bare fileId. */
export function scopedHighlightKey(
  scope: string | null | undefined,
  fileId: string,
): string {
  return scope ? `${scope}::${fileId}` : fileId
}
