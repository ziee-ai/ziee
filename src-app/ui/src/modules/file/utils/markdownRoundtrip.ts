// Markdown ⟷ Plate round-trip for the artifact canvas.
//
// The file's markdown stays canonical: on open we deserialize it into Plate's
// value model; on save we serialize back to GFM markdown. We use HEADLESS slate
// editors (no React) so this is usable from the round-trip unit tests and from
// the editor component alike. Constructs the editor does not model are preserved
// by @platejs/markdown's remark pipeline rather than dropped.

import { deserializeMd, MarkdownPlugin, serializeMd } from '@platejs/markdown'
import { BaseBasicBlocksPlugin, BaseBasicMarksPlugin } from '@platejs/basic-nodes'
import { createSlateEditor, type Value } from 'platejs'

/**
 * The HEADLESS plugin set defining which GFM constructs the round-trip
 * understands (headings, bold/italic/strike/code marks, lists, blockquotes) —
 * deliberately constrained to what the read-only Streamdown viewer renders so
 * what you edit matches what you render. The React editor component mirrors this
 * with the rendering (`Base*` → non-`Base*`) plugin variants.
 */
const roundtripPlugins = [
  BaseBasicBlocksPlugin,
  BaseBasicMarksPlugin,
  MarkdownPlugin,
]

function headlessEditor() {
  return createSlateEditor({ plugins: roundtripPlugins })
}

/** Parse markdown source into a Plate value for editing. */
export function markdownToEditor(md: string): Value {
  return deserializeMd(headlessEditor(), md ?? '')
}

/** Serialize a Plate value back to canonical GFM markdown. */
export function editorToMarkdown(value: Value): string {
  const editor = headlessEditor()
  editor.children = value
  return serializeMd(editor)
}

/**
 * Normalize markdown by round-tripping it through the editor model. Used on save
 * so repeated saves of unchanged content produce byte-identical output (stable,
 * minimal diffs) and so the stored form matches what the editor will re-open.
 */
export function normalizeMarkdown(md: string): string {
  return editorToMarkdown(markdownToEditor(md))
}
