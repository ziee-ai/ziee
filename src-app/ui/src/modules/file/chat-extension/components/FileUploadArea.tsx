import { useRef, useState } from 'react'
import { message } from '@/components/ui'
import { Stores } from '@/core/stores'

// Maximum file size (100MB)
const MAX_FILE_SIZE = 100 * 1024 * 1024

export interface FileUploadAreaProps {
  children: React.ReactNode
}

/**
 * FileUploadArea Component
 * Drag-and-drop overlay for file uploads
 * Wraps the chat input area to accept dropped files. Clicks pass through
 * to the children (drag-and-drop only — no file dialog). The overlay is a
 * plain Tailwind layer shown only while a file drag is in progress.
 */
export function FileUploadArea({ children }: FileUploadAreaProps) {
  // Access file extension store directly via Stores.Chat (reactive via store proxy)
  const { uploadFiles } = Stores.File
  const [dragging, setDragging] = useState(false)
  // Depth counter so dragenter/leave bubbling from child nodes doesn't
  // flicker the overlay (only hide once the pointer truly leaves the area).
  const depth = useRef(0)

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault()
    depth.current = 0
    setDragging(false)

    const dropped = Array.from(e.dataTransfer.files)
    if (dropped.length === 0) return

    // Validate file size — surface a toast for each oversized file.
    dropped
      .filter((f) => f.size > MAX_FILE_SIZE)
      .forEach((f) =>
        message.error(`File ${f.name} is too large. Maximum size is 100MB.`),
      )

    const files = dropped.filter((f) => f.size <= MAX_FILE_SIZE)
    if (files.length > 0) {
      uploadFiles(files).catch((error: any) => {
        console.error('Upload failed:', error)
        message.error('Failed to upload files')
      })
    }
  }

  return (
    <div
      className="relative"
      onDragEnter={(e) => {
        e.preventDefault()
        depth.current += 1
        if (e.dataTransfer.types?.includes('Files')) setDragging(true)
      }}
      onDragOver={(e) => e.preventDefault()}
      onDragLeave={(e) => {
        e.preventDefault()
        depth.current -= 1
        if (depth.current <= 0) {
          depth.current = 0
          setDragging(false)
        }
      }}
      onDrop={handleDrop}
    >
      {children}
      {dragging && (
        <div className="pointer-events-none absolute inset-0 z-10 flex min-h-[200px] items-center justify-center rounded-md border-2 border-dashed border-primary bg-accent">
          <span className="text-sm font-medium text-primary">
            Drop files to upload
          </span>
        </div>
      )}
    </div>
  )
}
