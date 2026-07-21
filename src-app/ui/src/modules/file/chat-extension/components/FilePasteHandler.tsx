import { useEffect, useRef } from 'react'
import { message } from '@ziee/kit'
import { Stores } from '@ziee/framework/stores'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { composerPaneKey } from '@/modules/file/stores/file'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import {
  MAX_FILE_UPLOAD_BYTES as MAX_FILE_SIZE,
  MAX_FILE_UPLOAD_LABEL,
} from '@/modules/file/constants'

/**
 * FilePasteHandler (ITEM-8 — paste image from clipboard)
 *
 * Slot-mounted (no children), mirroring FileUploadArea: it locates the composer
 * root via the `[data-chat-composer]` marker on ChatInput and attaches a
 * `paste` listener there. When the clipboard carries image files/items, they
 * are routed through the SAME upload path as drag-drop and the attach button
 * (`Stores.File.uploadFiles`), so limits/permissions/behavior are identical.
 *
 * Plain-text paste is left untouched (not intercepted, no preventDefault), so
 * pasting text into the textarea behaves exactly as before. Chat stays
 * file-agnostic — the file extension registers this into a chat input slot.
 */
export function FilePasteHandler() {
  const sentinelRef = useRef<HTMLSpanElement>(null)
  const canUpload = usePermission(Permissions.FilesUpload)
  // Keep the permission flag in a ref so the DOM listener (bound once) reads the
  // current value without re-binding.
  const canUploadRef = useRef(canUpload)
  canUploadRef.current = canUpload
  // This pane's composer buffer key (ITEM-32), likewise in a ref so the
  // bound-once paste listener uploads into THIS pane's attachments.
  const paneKeyRef = useRef(composerPaneKey(useChatPaneOrNull()?.paneId))
  paneKeyRef.current = composerPaneKey(useChatPaneOrNull()?.paneId)

  useEffect(() => {
    const el = sentinelRef.current?.closest<HTMLElement>('[data-chat-composer]')
    if (!el) return

    const onPaste = (e: ClipboardEvent) => {
      if (!canUploadRef.current) return
      const dt = e.clipboardData
      if (!dt) return

      // Collect image files from clipboard items. `getAsFile()` yields a File
      // for image items (e.g. a screenshot); `dt.files` is the fallback for
      // browsers that surface pasted images there.
      const collected: File[] = []
      for (const item of Array.from(dt.items ?? [])) {
        if (item.kind === 'file' && item.type.startsWith('image/')) {
          const file = item.getAsFile()
          if (file) collected.push(file)
        }
      }
      if (collected.length === 0) {
        for (const file of Array.from(dt.files ?? [])) {
          if (file.type.startsWith('image/')) collected.push(file)
        }
      }

      // No image payload → leave the event alone (plain-text paste proceeds).
      if (collected.length === 0) return

      // We're handling an image paste — stop it from also landing as text/URL
      // in the textarea.
      e.preventDefault()

      collected
        .filter((f) => f.size > MAX_FILE_SIZE)
        .forEach((f) =>
          message.error(
            f.name
              ? `Pasted image ${f.name} is too large. Maximum size is ${MAX_FILE_UPLOAD_LABEL}.`
              : `Pasted image is too large. Maximum size is ${MAX_FILE_UPLOAD_LABEL}.`,
          ),
        )

      const files = collected.filter((f) => f.size <= MAX_FILE_SIZE)
      if (files.length > 0) {
        // uploadFiles is an action — callable directly from a raw DOM event
        // listener (actions are hook-free, safe outside React render).
        Stores.File.uploadFiles(paneKeyRef.current, files).catch((error: unknown) => {
          console.error('Paste upload failed:', error)
          message.error('Failed to upload pasted image')
        })
      }
    }

    el.addEventListener('paste', onPaste)
    return () => el.removeEventListener('paste', onPaste)
  }, [])

  return <span ref={sentinelRef} className="hidden" aria-hidden="true" />
}
