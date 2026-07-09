import { Download } from 'lucide-react'
import { Button, Popover, message } from '@/components/ui'
import { ApiClient } from '@/api-client'
import type { File as FileEntity } from '@/api-client/types'

interface ExportFormat {
  key: string
  label: string
  ext: string
  mime: string
}

const EXPORT_FORMATS: ExportFormat[] = [
  { key: 'md', label: 'Markdown (.md)', ext: 'md', mime: 'text/markdown' },
  {
    key: 'docx',
    label: 'Word (.docx)',
    ext: 'docx',
    mime: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
  },
  { key: 'pdf', label: 'PDF', ext: 'pdf', mime: 'application/pdf' },
  {
    key: 'odt',
    label: 'OpenDocument (.odt)',
    ext: 'odt',
    mime: 'application/vnd.oasis.opendocument.text',
  },
  { key: 'rtf', label: 'Rich Text (.rtf)', ext: 'rtf', mime: 'application/rtf' },
  { key: 'html', label: 'HTML', ext: 'html', mime: 'text/html' },
]

/**
 * "Export as…" for a file/deliverable — downloads the head content converted to
 * the chosen format via the server (pandoc + typst). Distinct from the plain
 * Download button, which returns the original bytes untouched.
 */
export function FileExportMenu({ file }: { file: FileEntity }) {
  const stem = file.filename.replace(/\.[^.]+$/, '') || file.filename

  const doExport = async (fmt: ExportFormat) => {
    try {
      const res = await ApiClient.File.export({
        file_id: file.id,
        format: fmt.key,
      })
      const blob =
        res instanceof Blob ? res : new Blob([res as BlobPart], { type: fmt.mime })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `${stem}.${fmt.ext}`
      a.click()
      URL.revokeObjectURL(url)
      message.success(`Exported as ${fmt.label}`)
    } catch (e) {
      console.error('[file-export] failed', fmt.key, e)
      message.error(`Failed to export as ${fmt.label}`)
    }
  }

  const menu = (
    <div className="flex flex-col">
      {EXPORT_FORMATS.map(f => (
        <div
          key={f.key}
          role="button"
          tabIndex={0}
          data-testid={`file-export-${f.key}`}
          className="cursor-pointer rounded-md px-3 py-1.5 text-start text-foreground text-sm whitespace-nowrap hover:bg-muted focus-visible:outline focus-visible:outline-2"
          onClick={() => doExport(f)}
          onKeyDown={e => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault()
              doExport(f)
            }
          }}
        >
          {f.label}
        </div>
      ))}
    </div>
  )

  return (
    <Popover content={menu} side="bottom" align="end" className="w-auto">
      <Button variant="ghost" size="icon" aria-label="Export" data-testid="file-export-menu">
        <Download className="size-3.5" />
      </Button>
    </Popover>
  )
}
