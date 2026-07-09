import { forwardRef, useImperativeHandle } from 'react'
import { serializeMd } from '@platejs/markdown'
import { MarkdownPlugin } from '@platejs/markdown'
import { BasicBlocksPlugin, BasicMarksPlugin } from '@platejs/basic-nodes/react'
import { Plate, PlateContent, usePlateEditor } from 'platejs/react'
import { markdownToEditor } from '@/modules/file/utils/markdownRoundtrip'

/**
 * Imperative handle so the host (the canvas edit-mode) reads the current
 * markdown only on explicit Save — not on every keystroke (which would re-run
 * the full markdown serializer per character).
 */
export interface KitMarkdownEditorHandle {
  getMarkdown: () => string
}

interface KitMarkdownEditorProps {
  /** Initial markdown source (the file's head content). */
  initialMarkdown: string
  /** Fired on the first edit so the host can flag the canvas dirty. */
  onDirty?: () => void
}

/**
 * A rich WYSIWYG markdown editor built on Plate, constrained to the GFM subset
 * the Streamdown viewer renders (headings, bold/italic/strike/code, lists,
 * blockquotes). The file's markdown stays canonical: we deserialize it in and
 * serialize it back out (see `markdownRoundtrip`). Lazy-loaded via
 * `LazyMarkdownEditor` so view-only users never pay the bundle cost.
 */
export const KitMarkdownEditor = forwardRef<
  KitMarkdownEditorHandle,
  KitMarkdownEditorProps
>(function KitMarkdownEditor({ initialMarkdown, onDirty }, ref) {
  const editor = usePlateEditor({
    plugins: [BasicBlocksPlugin, BasicMarksPlugin, MarkdownPlugin],
    value: markdownToEditor(initialMarkdown),
  })

  useImperativeHandle(ref, () => ({ getMarkdown: () => serializeMd(editor) }), [
    editor,
  ])

  return (
    <Plate editor={editor} onChange={() => onDirty?.()}>
      <PlateContent
        data-testid="canvas-markdown-editor"
        className="h-full w-full overflow-auto px-4 py-3 text-sm leading-relaxed outline-none focus-visible:outline-none [&_h1]:mt-4 [&_h1]:mb-2 [&_h1]:text-xl [&_h1]:font-semibold [&_h2]:mt-3 [&_h2]:mb-1.5 [&_h2]:text-lg [&_h2]:font-semibold [&_p]:my-2 [&_ul]:my-2 [&_ul]:list-disc [&_ul]:pl-6 [&_ol]:my-2 [&_ol]:list-decimal [&_ol]:pl-6 [&_blockquote]:border-l-2 [&_blockquote]:border-border [&_blockquote]:pl-3 [&_blockquote]:text-muted-foreground [&_code]:rounded [&_code]:bg-muted [&_code]:px-1 [&_code]:py-0.5 [&_code]:text-xs"
        placeholder="Start writing…"
      />
    </Plate>
  )
})
