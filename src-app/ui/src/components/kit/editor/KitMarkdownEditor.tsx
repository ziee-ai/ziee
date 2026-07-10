import { forwardRef, useImperativeHandle } from 'react'
import { serializeMd } from '@platejs/markdown'
import { MarkdownPlugin } from '@platejs/markdown'
import { BasicBlocksPlugin, BasicMarksPlugin } from '@platejs/basic-nodes/react'
import { ListPlugin } from '@platejs/list/react'
import { ImagePlugin } from '@platejs/media/react'
import { Plate, PlateContent, usePlateEditor } from 'platejs/react'
import { ApiClient } from '@/api-client'
import { markdownToEditor } from '@/modules/file/utils/markdownRoundtrip'
import type { CanvasEditorHandle } from './types'
import { MarkdownToolbar } from './MarkdownToolbar'
import { CanvasImageElement } from './CanvasImageElement'

/**
 * Upload a pasted/dropped image to the file store and return its raw-bytes URL.
 * Plate's ImagePlugin calls this on paste/drop (a data-URL or ArrayBuffer), then
 * inserts an `img` node with the returned URL — which serializes to markdown
 * `![](/api/files/{id}/raw)` on Save (ITEM-21). The upload rides the app's
 * JWT-authed ApiClient (same path as the file drop-zone).
 */
async function uploadCanvasImage(dataUrl: ArrayBuffer | string): Promise<string> {
  const blob =
    typeof dataUrl === 'string'
      ? await (await fetch(dataUrl)).blob()
      : new Blob([dataUrl], { type: 'image/png' })
  const ext = (blob.type.split('/')[1] || 'png').replace(/[^a-z0-9]/gi, '')
  const file = new File([blob], `pasted-image-${Date.now()}.${ext}`, {
    type: blob.type || 'image/png',
  })
  const fd = new FormData()
  fd.append('file', file)
  const uploaded = await ApiClient.File.upload(fd, {})
  return `/api/files/${(uploaded as { id: string }).id}/raw`
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
  CanvasEditorHandle,
  KitMarkdownEditorProps
>(function KitMarkdownEditor({ initialMarkdown, onDirty }, ref) {
  const editor = usePlateEditor({
    plugins: [
      BasicBlocksPlugin,
      BasicMarksPlugin,
      ListPlugin,
      // Paste/drop an image → uploadCanvasImage → an `img` node that serializes
      // to a markdown `![](…)` link (ITEM-21). Rendered by CanvasImageElement.
      ImagePlugin.configure({
        options: { uploadImage: uploadCanvasImage },
        render: { node: CanvasImageElement },
      }),
      MarkdownPlugin,
    ],
    value: markdownToEditor(initialMarkdown),
  })

  useImperativeHandle(ref, () => ({ getContent: () => serializeMd(editor) }), [
    editor,
  ])

  return (
    <Plate
      editor={editor}
      onChange={() => {
        // Plate/Slate fire onChange on cursor/selection moves too. Only flag the
        // canvas dirty on a REAL content mutation — otherwise merely clicking into
        // the editor arms the unsaved-changes guard + enables Save with no edit.
        if (editor.operations.some((op) => op.type !== 'set_selection')) onDirty?.()
      }}
    >
      <div className="flex h-full flex-col">
        <MarkdownToolbar />
        <PlateContent
          aria-label="Markdown document editor"
          data-testid="canvas-markdown-editor"
          className="min-h-0 w-full flex-1 overflow-auto px-4 py-3 text-sm leading-relaxed outline-none focus-visible:outline-none [&_h1]:mt-4 [&_h1]:mb-2 [&_h1]:text-xl [&_h1]:font-semibold [&_h2]:mt-3 [&_h2]:mb-1.5 [&_h2]:text-lg [&_h2]:font-semibold [&_p]:my-2 [&_ul]:my-2 [&_ul]:list-disc [&_ul]:ps-6 [&_ol]:my-2 [&_ol]:list-decimal [&_ol]:ps-6 [&_blockquote]:border-border [&_blockquote]:border-s-2 [&_blockquote]:ps-3 [&_blockquote]:text-muted-foreground [&_code]:rounded [&_code]:bg-muted [&_code]:px-1 [&_code]:py-0.5 [&_code]:text-xs"
          placeholder="Start writing…"
        />
      </div>
    </Plate>
  )
})
