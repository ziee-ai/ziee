// Pure mapping from a discovered model (GET /discover-models) to the
// Add-Remote-Model form's fields. Extracted from the drawer so the capability +
// display-name auto-fill is unit-testable (node:test) without React.

/** Structural subset of `DiscoveredModel` the mapping reads. */
export interface DiscoveredModelLike {
  id: string
  display_name?: string
  supports_vision?: boolean
  supports_tool_use?: boolean
  supports_embeddings?: boolean
  supports_chat: boolean
  context_length?: number
}

/** Flat form fields the drawer sets via `form.setValue` on pick. */
export interface DiscoveredFormFields {
  display_name: string
  vision: boolean
  tools: boolean
  text_embedding: boolean
  chat: boolean
  context_length?: number
}

export function mapDiscoveredModelToForm(m: DiscoveredModelLike): DiscoveredFormFields {
  return {
    // Fall back to the id when the catalog/live model has no label.
    display_name: m.display_name || m.id,
    vision: Boolean(m.supports_vision),
    tools: Boolean(m.supports_tool_use),
    text_embedding: Boolean(m.supports_embeddings),
    // `supports_chat` is a required bool on DiscoveredModel; preserve it verbatim.
    chat: m.supports_chat,
    context_length: m.context_length,
  }
}
