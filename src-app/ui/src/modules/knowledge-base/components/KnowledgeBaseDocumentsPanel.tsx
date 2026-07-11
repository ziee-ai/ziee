import { useEffect, useRef, useState } from 'react'
import { RotateCw, Trash2, Upload as UploadIcon } from 'lucide-react'
import {
  Button,
  Card,
  Empty,
  Flex,
  Spin,
  Tag,
  Text,
  Tooltip,
  Upload,
  message,
} from '@/components/ui'
import { Can } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { type KnowledgeBaseDocument, Permissions } from '@/api-client/types'
import { ListPagination } from '@/components/common/ListPagination'
import { FileCard } from '@/modules/file/components/FileCard'
import { docStatusBadge, docToFileEntity, isRetryable, partitionKbUploads } from '../docStatus'

interface Props {
  kbId: string
}

/** 100 MiB per file — mirrors the file module's cap (ProjectFilesManagePanel). */
const MAX_KB_FILE_SIZE = 100 * 1024 * 1024
/** Text-extractable types the KB accepts (the client `accept` is bypassable, so
 *  validate here too and itemize rejects). */
const KB_ACCEPTED_EXT = new Set([
  'pdf', 'txt', 'md', 'csv', 'json', 'docx', 'doc', 'rtf', 'odt', 'html',
])
/** Server default cap (admin-configurable) — shown as the counter approaches it. */
const KB_MAX_DOCUMENTS = 2000

/**
 * KB documents panel — mirrors the project knowledge-files panel
 * (`ProjectFilesManagePanel`): reuses `FileCard variant="row"` (thumbnails +
 * size/type subtitle), streams per-file UPLOAD PROGRESS rows, supports
 * drag-drop + multi-select bulk remove, and layers the KB-specific per-document
 * index-status badge + retry into each row's actions.
 */
export function KnowledgeBaseDocumentsPanel({ kbId }: Props) {
  const {
    kb,
    documents,
    documentsLoading,
    documentsPage,
    documentsPageSize,
    uploadingFiles,
    selectedFileIds,
  } = Stores.KnowledgeBaseDetail

  const rootRef = useRef<HTMLDivElement>(null)
  const [isDragging, setIsDragging] = useState(false)
  const count = kb?.document_count ?? documents.length

  const handleFiles = async (files: File[]) => {
    if (files.length === 0) return
    // Client-side per-file validation with an ITEMIZED reject report (which
    // files, and why) — never a vague "some failed". The server also enforces
    // size + MIME-sniff as defense-in-depth.
    const { accepted, rejected } = partitionKbUploads(
      files,
      MAX_KB_FILE_SIZE,
      KB_ACCEPTED_EXT,
    )
    if (rejected.length > 0) {
      const shown = rejected
        .slice(0, 4)
        .map(r => `${r.name} (${r.reason === 'too-large' ? 'too large' : 'unsupported type'})`)
        .join(', ')
      message.error(
        `Skipped ${rejected.length} file${rejected.length === 1 ? '' : 's'}: ${shown}` +
          (rejected.length > 4 ? ` +${rejected.length - 4} more` : ''),
      )
    }
    if (accepted.length === 0) return
    try {
      const result = await Stores.KnowledgeBaseDetail.uploadAndAttach(kbId, accepted)
      const parts: string[] = []
      if (result.attached > 0) parts.push(`${result.attached} added`)
      if (result.skipped_duplicates > 0)
        parts.push(`${result.skipped_duplicates} already in this knowledge base`)
      if (parts.length > 0) message.success(parts.join(', '))
    } catch (err) {
      message.error(err instanceof Error ? err.message : 'Upload failed')
    }
  }

  const handleRetryAll = async () => {
    try {
      await Stores.KnowledgeBaseDetail.retryAllFailed(kbId)
      message.success('Re-indexing failed documents')
    } catch (err) {
      message.error(err instanceof Error ? err.message : 'Failed to re-index')
    }
  }
  const handleRef = useRef(handleFiles)
  handleRef.current = handleFiles

  const handleRemove = async (doc: KnowledgeBaseDocument) => {
    try {
      await Stores.KnowledgeBaseDetail.removeDocument(kbId, doc.file_id)
    } catch (err) {
      message.error(err instanceof Error ? err.message : 'Failed to remove')
    }
  }

  const handleBatchRemove = async () => {
    const n = selectedFileIds.size
    if (n === 0) return
    try {
      await Stores.KnowledgeBaseDetail.batchRemove(kbId)
      message.success(`Removed ${n} document${n === 1 ? '' : 's'}`)
    } catch (err) {
      message.error(err instanceof Error ? err.message : 'Failed to remove')
    }
  }

  const handleReindex = async (doc: KnowledgeBaseDocument) => {
    try {
      await Stores.KnowledgeBaseDetail.reindexDocument(kbId, doc.file_id)
      message.success('Re-indexing started')
    } catch (err) {
      message.error(err instanceof Error ? err.message : 'Failed to re-index')
    }
  }

  // Drag-and-drop onto the panel container (the KB detail page is the
  // management surface, so the drop target is this panel — not a dialog).
  useEffect(() => {
    const el = rootRef.current
    if (!el) return
    let dragCounter = 0
    const hasFiles = (e: DragEvent) =>
      Array.from(e.dataTransfer?.types ?? []).includes('Files')
    const onEnter = (e: DragEvent) => {
      if (!hasFiles(e)) return
      e.preventDefault()
      dragCounter += 1
      if (dragCounter === 1) setIsDragging(true)
    }
    const onLeave = (e: DragEvent) => {
      if (!hasFiles(e)) return
      e.preventDefault()
      dragCounter = Math.max(0, dragCounter - 1)
      if (dragCounter === 0) setIsDragging(false)
    }
    const onOver = (e: DragEvent) => {
      if (hasFiles(e)) e.preventDefault()
    }
    const onDrop = (e: DragEvent) => {
      if (!hasFiles(e)) return
      e.preventDefault()
      dragCounter = 0
      setIsDragging(false)
      void handleRef.current(Array.from(e.dataTransfer?.files ?? []))
    }
    el.addEventListener('dragenter', onEnter)
    el.addEventListener('dragleave', onLeave)
    el.addEventListener('dragover', onOver)
    el.addEventListener('drop', onDrop)
    return () => {
      el.removeEventListener('dragenter', onEnter)
      el.removeEventListener('dragleave', onLeave)
      el.removeEventListener('dragover', onOver)
      el.removeEventListener('drop', onDrop)
    }
  }, [])

  const uploadingRows = Array.from(uploadingFiles.values())

  return (
    <Card
      data-testid="kb-detail-documents"
      title={
        <Flex align="center" gap="small">
          <span>Documents</span>
          <Tag
            variant="outline"
            tone={
              count >= KB_MAX_DOCUMENTS
                ? 'error'
                : count >= KB_MAX_DOCUMENTS - 50
                  ? 'warning'
                  : undefined
            }
            data-testid="kb-documents-count"
          >
            {count}
            {count >= KB_MAX_DOCUMENTS - 50 ? ` / ${KB_MAX_DOCUMENTS}` : ''} document
            {count === 1 ? '' : 's'}
          </Tag>
        </Flex>
      }
      extra={
        <Can permission={Permissions.KnowledgeBaseManage}>
          <Upload
            data-testid="kb-documents-upload"
            label="Add documents"
            multiple
            directory
            accept=".pdf,.txt,.md,.csv,.json,.docx,.doc,.rtf,.odt,.html"
            onFiles={files => void handleFiles(files)}
            className="!border-0 !p-0 !gap-0 !rounded-none"
          >
            <Button data-testid="kb-documents-upload-button" icon={<UploadIcon />}>
              Add documents
            </Button>
          </Upload>
        </Can>
      }
    >
      <div ref={rootRef} className="relative flex flex-col gap-3">
      {/* Honesty-at-scale: scanned/no-text advisory + retry-all-failed. */}
      {kb && (kb.indexing_summary.no_text > 0 || kb.indexing_summary.failed > 0) && (
        <div className="flex items-center justify-between gap-2 flex-wrap">
          {kb.indexing_summary.no_text > 0 ? (
            <Text
              type="secondary"
              className="text-xs"
              data-testid="kb-documents-no-text-advisory"
            >
              {kb.indexing_summary.no_text} document
              {kb.indexing_summary.no_text === 1 ? '' : 's'} have no extractable text
              (scanned?) — not retrievable.
            </Text>
          ) : (
            <span />
          )}
          {kb.indexing_summary.failed > 0 && (
            <Can permission={Permissions.KnowledgeBaseManage}>
              <Button
                size="default"
                variant="outline"
                icon={<RotateCw />}
                data-testid="kb-documents-retry-all"
                onClick={() => void handleRetryAll()}
              >
                Retry {kb.indexing_summary.failed} failed
              </Button>
            </Can>
          )}
        </div>
      )}

      {/* Multi-select bulk actions. */}
      {selectedFileIds.size > 0 && (
        <div className="flex items-center justify-between gap-2 px-3 py-2 rounded bg-primary/10 border border-primary/30">
          <Text>{selectedFileIds.size} selected</Text>
          <Flex align="center" gap="small">
            <Button
              size="default"
              variant="outline"
              data-testid="kb-documents-clear-selection"
              onClick={() => Stores.KnowledgeBaseDetail.deselectAll()}
            >
              Clear
            </Button>
            <Button
              size="default"
              variant="ghost"
              icon={<Trash2 />}
              data-testid="kb-documents-remove-selected"
              onClick={() => void handleBatchRemove()}
            >
              Remove selected
            </Button>
          </Flex>
        </div>
      )}

      {/* Per-file upload progress rows (thumbnail slot shows a % ring). */}
      {uploadingRows.length > 0 && (
        <div className="flex flex-col gap-2">
          {uploadingRows.map(progress => (
            <FileCard
              key={progress.id}
              uploadProgress={progress}
              variant="row"
              onRemove={() =>
                Stores.KnowledgeBaseDetail.dismissUploadingFile(progress.id)
              }
            />
          ))}
        </div>
      )}

      {documentsLoading && documents.length === 0 ? (
        <div className="flex justify-center py-8">
          <Spin label="Loading documents" />
        </div>
      ) : documents.length === 0 && uploadingRows.length === 0 ? (
        <Empty
          data-testid="kb-documents-empty"
          title="No documents yet"
          description="Add documents (or drop a folder) — the agent retrieves relevant passages from them."
        />
      ) : (
        <div className="flex flex-col gap-2" data-testid="kb-documents-list">
          {documents.map(doc => {
            const status = docStatusBadge(doc.index_status)
            return (
              <FileCard
                key={doc.file_id}
                file={docToFileEntity(doc)}
                variant="row"
                selectable
                selected={selectedFileIds.has(doc.file_id)}
                onSelectChange={() =>
                  Stores.KnowledgeBaseDetail.toggleSelection(doc.file_id)
                }
                subtitle={
                  <>
                    {doc.mime_type ?? 'file'}
                    {doc.chunk_count > 0
                      ? ` · ${doc.chunk_count} chunk${doc.chunk_count === 1 ? '' : 's'}`
                      : ''}
                  </>
                }
                actions={
                  <Flex align="center" gap="small">
                    <Tag
                      data-testid={`kb-document-status-${doc.file_id}`}
                      tone={status.tone}
                    >
                      {status.label}
                    </Tag>
                    <Can permission={Permissions.KnowledgeBaseManage}>
                      {isRetryable(doc.index_status) && (
                        <Tooltip content="Re-index">
                          <Button
                            data-testid={`kb-document-reindex-${doc.file_id}`}
                            variant="ghost"
                            size="icon"
                            icon={<RotateCw />}
                            aria-label={`Re-index ${doc.filename}`}
                            onClick={() => void handleReindex(doc)}
                          />
                        </Tooltip>
                      )}
                      <Tooltip content="Remove">
                        <Button
                          data-testid={`kb-document-remove-${doc.file_id}`}
                          variant="ghost"
                          size="icon"
                          icon={<Trash2 />}
                          aria-label={`Remove ${doc.filename}`}
                          onClick={() => void handleRemove(doc)}
                        />
                      </Tooltip>
                    </Can>
                  </Flex>
                }
              />
            )
          })}
        </div>
      )}

      {/* Numbered server-side pagination (discrete pages + page-size changer),
          mirroring the users/memories settings pages via `ListPagination` — NOT
          infinite scroll. A KB holds up to 2000 documents. */}
      {count > 0 && (
        <ListPagination
          data-testid="kb-documents-pagination"
          current={documentsPage}
          total={count}
          pageSize={documentsPageSize}
          itemNoun="documents"
          onChange={p =>
            void Stores.KnowledgeBaseDetail.loadDocumentsPage(
              kbId,
              p,
              documentsPageSize,
            )
          }
          onPageSizeChange={s =>
            void Stores.KnowledgeBaseDetail.loadDocumentsPage(kbId, 1, s)
          }
        />
      )}

      {/* Drag overlay covering the panel. */}
      {isDragging && (
        <div
          className="absolute inset-0 z-10 flex flex-col items-center justify-center gap-2 rounded-lg border-2 border-dashed border-primary bg-primary/10 text-primary font-medium pointer-events-none"
          data-testid="kb-documents-drop-overlay"
        >
          <UploadIcon size={32} />
          <span>Drop files to add to this knowledge base</span>
        </div>
      )}
      </div>
    </Card>
  )
}
