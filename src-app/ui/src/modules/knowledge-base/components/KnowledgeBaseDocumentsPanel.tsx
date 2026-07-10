import { FileText, RotateCw, Trash2, Upload as UploadIcon } from 'lucide-react'
import {
  Button,
  Empty,
  Flex,
  List,
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
import { docStatusBadge, isRetryable } from '../docStatus'

interface Props {
  kbId: string
}

export function KnowledgeBaseDocumentsPanel({ kbId }: Props) {
  const { documents, documentsLoading, uploading } = Stores.KnowledgeBaseDetail

  const handleFiles = async (files: File[]) => {
    if (files.length === 0) return
    try {
      const result = await Stores.KnowledgeBaseDetail.uploadAndAttach(kbId, files)
      const parts: string[] = []
      if (result.attached > 0) parts.push(`${result.attached} added`)
      if (result.skipped_duplicates > 0)
        parts.push(`${result.skipped_duplicates} already in this knowledge base`)
      message.success(parts.join(', ') || 'No documents added')
    } catch (err) {
      message.error(err instanceof Error ? err.message : 'Upload failed')
    }
  }

  const handleRemove = async (doc: KnowledgeBaseDocument) => {
    try {
      await Stores.KnowledgeBaseDetail.removeDocument(kbId, doc.file_id)
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

  return (
    <div className="flex flex-col gap-3">
      <Can permission={Permissions.KnowledgeBaseManage}>
        <Upload
          data-testid="kb-documents-upload"
          label="Add documents"
          multiple
          directory
          accept=".pdf,.txt,.md,.csv,.json,.docx,.doc,.rtf,.odt,.html"
          onFiles={files => void handleFiles(files)}
        >
          <Button
            data-testid="kb-documents-upload-button"
            icon={<UploadIcon />}
            loading={uploading}
          >
            Add documents
          </Button>
        </Upload>
      </Can>

      {documentsLoading && documents.length === 0 ? (
        <div className="flex justify-center py-8">
          <Spin label="Loading documents" />
        </div>
      ) : documents.length === 0 ? (
        <Empty
          data-testid="kb-documents-empty"
          icon={<FileText className="size-12" />}
          title="No documents yet"
          description="Add documents (or drop a folder) — the agent retrieves relevant passages from them."
        />
      ) : (
        <List
          data-testid="kb-documents-list"
          dataSource={documents}
          rowKey="file_id"
          renderItem={(doc: KnowledgeBaseDocument) => {
            const status = docStatusBadge(doc.index_status)
            return (
              <Flex align="center" justify="between" gap="small" className="w-full py-1">
                <Flex align="center" gap="small" className="min-w-0">
                  <FileText aria-hidden="true" className="shrink-0 size-4 text-muted-foreground" />
                  <Text ellipsis className="min-w-0">
                    {doc.filename}
                  </Text>
                </Flex>
                <Flex align="center" gap="small" className="shrink-0">
                  <Tag data-testid={`kb-document-status-${doc.file_id}`} tone={status.tone}>
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
              </Flex>
            )
          }}
        />
      )}
    </div>
  )
}
