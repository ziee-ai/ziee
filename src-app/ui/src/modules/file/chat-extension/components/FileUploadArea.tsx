import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { message } from '@/components/ui'
import { Stores } from '@/core/stores'

// Maximum file size (100MB)
const MAX_FILE_SIZE = 100 * 1024 * 1024

/**
 * FileUploadArea Component
 *
 * Drag-and-drop file upload for the chat composer. Slot-mounted (no children):
 * the file extension registers this into an input-area slot, and it locates the
 * composer root via the `[data-chat-composer]` marker on ChatInput, attaches
 * drag listeners to it, and portals a "Drop files to upload" overlay while a
 * file drag is in progress. This keeps the chat module decoupled from the file
 * module (chat never imports file — the file extension registers into chat).
 */
export function FileUploadArea() {
  const sentinelRef = useRef<HTMLSpanElement>(null)
  const [host, setHost] = useState<HTMLElement | null>(null)
  const [dragging, setDragging] = useState(false)
  // Depth counter so dragenter/leave bubbling from child nodes doesn't
  // flicker the overlay (only hide once the pointer truly leaves the area).
  const depth = useRef(0)

  useEffect(() => {
    const el = sentinelRef.current?.closest<HTMLElement>('[data-chat-composer]')
    if (!el) return
    setHost(el)

    const onEnter = (e: DragEvent) => {
      e.preventDefault()
      depth.current += 1
      if (e.dataTransfer?.types?.includes('Files')) setDragging(true)
    }
    const onOver = (e: DragEvent) => e.preventDefault()
    const onLeave = (e: DragEvent) => {
      e.preventDefault()
      depth.current -= 1
      if (depth.current <= 0) {
        depth.current = 0
        setDragging(false)
      }
    }
    const onDrop = (e: DragEvent) => {
      e.preventDefault()
      depth.current = 0
      setDragging(false)

      const dropped = Array.from(e.dataTransfer?.files ?? [])
      if (dropped.length === 0) return

      // Validate file size — surface a toast for each oversized file.
      dropped
        .filter((f) => f.size > MAX_FILE_SIZE)
        .forEach((f) =>
          message.error(`File ${f.name} is too large. Maximum size is 100MB.`),
        )

      const files = dropped.filter((f) => f.size <= MAX_FILE_SIZE)
      if (files.length > 0) {
        // `__state` (not the render-only proxy) — store access from a raw DOM
        // event listener, outside React render.
        Stores.File.uploadFiles(files).catch((error: unknown) => {
          console.error('Upload failed:', error)
          message.error('Failed to upload files')
        })
      }
    }

    el.addEventListener('dragenter', onEnter)
    el.addEventListener('dragover', onOver)
    el.addEventListener('dragleave', onLeave)
    el.addEventListener('drop', onDrop)
    return () => {
      el.removeEventListener('dragenter', onEnter)
      el.removeEventListener('dragover', onOver)
      el.removeEventListener('dragleave', onLeave)
      el.removeEventListener('drop', onDrop)
    }
  }, [])

  return (
    <>
      <span ref={sentinelRef} className="hidden" aria-hidden="true" />
      {dragging && host &&
        createPortal(
          <div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center rounded-lg border-2 border-dashed border-primary bg-accent/80">
            <span className="text-sm font-medium text-primary">
              Drop files to upload
            </span>
          </div>,
          host,
        )}
    </>
  )
}
