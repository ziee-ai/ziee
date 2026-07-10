/**
 * Shapes a "selection → LLM" request from the canvas. The model edits a
 * deliverable via the built-in `files_mcp::edit_file` (unique `old_str`→`new_str`).
 * A scoped edit is only safe when the selected text occurs EXACTLY ONCE in the
 * document — otherwise `edit_file` would reject an ambiguous `old_str`, so we
 * degrade to an instruction-only message referencing the excerpt. Pure +
 * unit-tested; the UI wires the returned message into the chat composer.
 */

/** True iff `selection` is a non-empty substring occurring exactly once in `docText`. */
export function isUniqueSelection(docText: string, selection: string): boolean {
  if (!selection) return false
  const first = docText.indexOf(selection)
  if (first === -1) return false
  return docText.indexOf(selection, first + 1) === -1
}

function blockquote(text: string): string {
  return text
    .split('\n')
    .map(l => `> ${l}`)
    .join('\n')
}

/** Non-mutating "ask about this" — quotes the excerpt + the user's question. */
export function buildSelectionAskMessage(selection: string, question: string): string {
  return `Regarding this excerpt from the document:\n\n${blockquote(selection)}\n\n${question}`
}

/**
 * Mutating "edit this section". When the selection is unique, returns `oldStr`
 * so the model can do a targeted `edit_file`; otherwise `oldStr` is undefined and
 * the message asks for the change without an exact anchor.
 */
export function buildSelectionEditMessage(
  fileName: string,
  selection: string,
  instruction: string,
  docText: string,
): { message: string; oldStr?: string } {
  const unique = isUniqueSelection(docText, selection)
  const message = unique
    ? `In the deliverable "${fileName}", edit exactly this section:\n\n${blockquote(selection)}\n\nInstruction: ${instruction}`
    : `In the deliverable "${fileName}", regarding this section (it appears more than once, so locate it in context):\n\n${blockquote(selection)}\n\nInstruction: ${instruction}`
  return unique ? { message, oldStr: selection } : { message }
}
