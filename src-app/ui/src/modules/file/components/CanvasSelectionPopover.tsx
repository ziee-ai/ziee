import { useEffect, useRef, useState } from 'react'
import { MessageSquareQuote, PencilLine } from 'lucide-react'
import { Button, message } from '@ziee/kit'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import {
  buildSelectionAskMessage,
  buildSelectionEditMessage,
} from '@/modules/file/utils/selectionEdit'
import { Chat } from '@/modules/chat/core/stores/chatBridge'

/**
 * Floating "selection → LLM" popover for the markdown canvas (ITEM-15 / ITEM-16).
 * When the user selects text inside the editor, a small toolbar appears with:
 *   - "Ask about this"  → quotes the excerpt into the chat composer as context
 *     (non-mutating, ITEM-15).
 *   - "Edit this section" → sends a targeted edit request (the model runs a
 *     scoped `edit_file` when the selection is unique, ITEM-16).
 * The message SHAPING is the unit-tested `selectionEdit` helpers; this component
 * captures the selection, positions the toolbar, and wires the result into the
 * conversation's composer (`Chat.$.TextStore`).
 */
export function CanvasSelectionPopover({
  containerRef,
  fileName,
  getDocText,
}: {
  /** The editor body element the selection must fall within. */
  containerRef: React.RefObject<HTMLElement | null>
  fileName: string
  /** Current document text (for the unique-`old_str` scoped-edit gate). */
  getDocText: () => string
}) {
  const [sel, setSel] = useState<{ text: string; top: number; left: number } | null>(null)
  const popRef = useRef<HTMLDivElement>(null)
  // Inject into THIS pane's composer, not the focused-pane bridge (audit #10): a
  // canvas viewer lives in its own pane's right panel, so "Ask/Edit in chat" from
  // pane B's viewer must target pane B's composer even when pane A is focused.
  const paneChat = (useChatPaneOrNull()?.store ?? Chat) as typeof Chat

  useEffect(() => {
    const onUp = () => {
      // Defer so the browser finishes updating the selection.
      requestAnimationFrame(() => {
        const s = window.getSelection()
        const text = s?.toString().trim() ?? ''
        const container = containerRef.current
        if (!s || s.rangeCount === 0 || !text || !container) {
          setSel(null)
          return
        }
        const range = s.getRangeAt(0)
        // Selection must be inside the editor body (not the popover or elsewhere).
        if (!container.contains(range.commonAncestorContainer)) {
          setSel(null)
          return
        }
        const rect = range.getBoundingClientRect()
        setSel({ text, top: rect.top - 8, left: rect.left + rect.width / 2 })
      })
    }
    document.addEventListener('mouseup', onUp)
    document.addEventListener('keyup', onUp)
    return () => {
      document.removeEventListener('mouseup', onUp)
      document.removeEventListener('keyup', onUp)
    }
  }, [containerRef])

  if (!sel) return null

  const composer = paneChat.$.TextStore
  const injectIntoComposer = (text: string) => {
    const existing = composer.getText()
    composer.setText(existing ? `${existing}\n\n${text}` : text)
  }

  const onAsk = () => {
    injectIntoComposer(buildSelectionAskMessage(sel.text, ''))
    message.success('Added the selection to the chat as context')
    setSel(null)
    window.getSelection()?.removeAllRanges()
  }

  const onEdit = () => {
    const instruction = window.prompt('How should the model edit this section?')
    if (instruction == null || !instruction.trim()) return
    const { message: msg } = buildSelectionEditMessage(
      fileName,
      sel.text,
      instruction.trim(),
      getDocText(),
    )
    injectIntoComposer(msg)
    message.success('Sent the scoped edit request to the chat')
    setSel(null)
    window.getSelection()?.removeAllRanges()
  }

  return (
    <div
      ref={popRef}
      data-testid="canvas-selection-popover"
      className="fixed z-50 flex -translate-x-1/2 -translate-y-full items-center gap-1 rounded-md border border-border bg-popover p-1 shadow-md"
      style={{ top: sel.top, left: sel.left }}
      // Keep the selection alive when the user mouses onto the toolbar.
      onMouseDown={(e) => e.preventDefault()}
    >
      <Button
        variant="ghost"
        size="default"
        aria-label="Ask about this"
        data-testid="canvas-selection-ask"
        onClick={onAsk}
      >
        <MessageSquareQuote className="size-3.5" /> Ask about this
      </Button>
      <Button
        variant="ghost"
        size="default"
        aria-label="Edit this section"
        data-testid="canvas-selection-edit"
        onClick={onEdit}
      >
        <PencilLine className="size-3.5" /> Edit this section
      </Button>
    </div>
  )
}
