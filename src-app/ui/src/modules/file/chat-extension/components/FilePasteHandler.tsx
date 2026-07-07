import { useEffect, useRef } from 'react'
import { message } from '@/components/ui'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

// Maximum file size (100MB) — mirrors FileUploadArea / FileUploadButton.
const MAX_FILE_SIZE = 100 * 1024 * 1024

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
            `Pasted image ${f.name || ''} is too large. Maximum size is 100MB.`,
          ),
        )

      const files = collected.filter((f) => f.size <= MAX_FILE_SIZE)
      if (files.length > 0) {
        // `__state` (not the render-only proxy) — store access from a raw DOM
        // event listener, outside React render.
        Stores.File.__state.uploadFiles(files).catch((error: unknown) => {
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
