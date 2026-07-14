import type { ReactNode, MouseEvent } from 'react'
import {
  Bold,
  Code,
  Heading1,
  Heading2,
  Italic,
  List,
  ListOrdered,
  Quote,
  Strikethrough,
} from 'lucide-react'
import { useEditorRef } from 'platejs/react'
import { Button } from '@ziee/kit'

/**
 * Formatting toolbar for the markdown canvas. Rendered INSIDE the `<Plate>`
 * context so `useEditorRef()` resolves the live editor. Each control is a
 * mousedown-preventDefault button so clicking it doesn't steal selection/focus
 * from the editor, then toggles the mark/block via Plate's transforms.
 */
export function MarkdownToolbar() {
  const editor = useEditorRef()

  const mark = (key: string) => (e: MouseEvent) => {
    e.preventDefault()
    editor.tf.toggleMark(key)
  }
  const block = (type: string) => (e: MouseEvent) => {
    e.preventDefault()
    editor.tf.toggleBlock(type)
  }

  const items: Array<{ id: string; label: string; icon: ReactNode; onClick: (e: MouseEvent) => void }> = [
    { id: 'bold', label: 'Bold', icon: <Bold className="size-3.5" />, onClick: mark('bold') },
    { id: 'italic', label: 'Italic', icon: <Italic className="size-3.5" />, onClick: mark('italic') },
    { id: 'strikethrough', label: 'Strikethrough', icon: <Strikethrough className="size-3.5" />, onClick: mark('strikethrough') },
    { id: 'code', label: 'Inline code', icon: <Code className="size-3.5" />, onClick: mark('code') },
    { id: 'h1', label: 'Heading 1', icon: <Heading1 className="size-3.5" />, onClick: block('h1') },
    { id: 'h2', label: 'Heading 2', icon: <Heading2 className="size-3.5" />, onClick: block('h2') },
    { id: 'blockquote', label: 'Quote', icon: <Quote className="size-3.5" />, onClick: block('blockquote') },
    { id: 'bulleted-list', label: 'Bulleted list', icon: <List className="size-3.5" />, onClick: block('ul') },
    { id: 'numbered-list', label: 'Numbered list', icon: <ListOrdered className="size-3.5" />, onClick: block('ol') },
  ]

  return (
    <div
      className="flex flex-wrap items-center gap-0.5 border-border border-b bg-muted/40 px-2 py-1"
      data-testid="canvas-markdown-toolbar"
    >
      {items.map(it => (
        <Button
          key={it.id}
          variant="ghost"
          size="icon"
          aria-label={it.label}
          data-testid={`canvas-toolbar-${it.id}`}
          onMouseDown={it.onClick}
        >
          {it.icon}
        </Button>
      ))}
    </div>
  )
}
