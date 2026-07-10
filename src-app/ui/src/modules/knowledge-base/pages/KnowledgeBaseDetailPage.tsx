import { useEffect, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import {
  ArrowLeft,
  Pencil,
} from 'lucide-react'
import {
  Button,
  Card,
  Descriptions,
  Progress,
  Result,
  Spin,
  Title,
} from '@/components/ui'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { DivScrollY } from '@/components/common/DivScrollY'
import { KnowledgeBaseDocumentsPanel } from '@/modules/knowledge-base/components/KnowledgeBaseDocumentsPanel'
import { KnowledgeBaseFormDrawer } from '@/modules/knowledge-base/components/KnowledgeBaseFormDrawer'

export function KnowledgeBaseDetailPage() {
  const { kbId } = useParams<{ kbId: string }>()
  const navigate = useNavigate()
  const { kb, loading } = Stores.KnowledgeBaseDetail
  const [editOpen, setEditOpen] = useState(false)

  useEffect(() => {
    if (kbId) void Stores.KnowledgeBaseDetail.load(kbId)
    return () => Stores.KnowledgeBaseDetail.reset()
  }, [kbId])

  if (loading && !kb) {
    return (
      <div className="flex justify-center py-12">
        <Spin label="Loading knowledge base" />
      </div>
    )
  }

  if (!kb) {
    return (
      <Result
        data-testid="kb-detail-not-found"
        status="error"
        title="Knowledge base not found"
        subtitle="It may have been deleted, or you don't have access."
        extra={
          <Button data-testid="kb-detail-back-button" onClick={() => navigate('/knowledge')}>
            Back to Knowledge
          </Button>
        }
      />
    )
  }

  const s = kb.indexing_summary
  const inProgress = s.indexing + s.pending
  const indexedPct = s.total > 0 ? Math.round((s.indexed / s.total) * 100) : 0

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <HeaderBarContainer>
        <div className="h-full flex items-center justify-between w-full gap-2">
          <div className="flex items-center gap-2 min-w-0">
            <Button
              data-testid="kb-detail-back"
              variant="ghost"
              size="icon"
              icon={<ArrowLeft />}
              aria-label="Back to Knowledge"
              onClick={() => navigate('/knowledge')}
            />
            <Title level={4} className="!m-0 !leading-tight truncate" data-testid="kb-detail-title">
              {kb.name}
            </Title>
          </div>
          <Can permission={Permissions.KnowledgeBaseManage}>
            <Button
              data-testid="kb-detail-edit"
              variant="outline"
              size="icon"
              icon={<Pencil />}
              aria-label="Edit knowledge base"
              onClick={() => setEditOpen(true)}
            />
          </Can>
        </div>
      </HeaderBarContainer>

      <DivScrollY nativeFlow className="flex-1">
        <div className="flex flex-col gap-3 max-w-4xl mx-auto p-4 w-full">
          <Card data-testid="kb-detail-overview" title="Overview">
            <Descriptions
              data-testid="kb-detail-overview-descriptions"
              column={2}
              items={[
                { key: 'documents', label: 'Documents', children: String(kb.document_count) },
                { key: 'indexed', label: 'Indexed', children: `${s.indexed} / ${s.total}` },
                ...(s.failed > 0
                  ? [{ key: 'failed', label: 'Failed', children: String(s.failed) }]
                  : []),
                ...(s.no_text > 0
                  ? [{ key: 'no_text', label: 'No extractable text', children: String(s.no_text) }]
                  : []),
                { key: 'description', label: 'Description', children: kb.description || '—' },
              ]}
            />
            {inProgress > 0 && (
              <div className="mt-3">
                <Progress
                  data-testid="kb-detail-indexing-progress"
                  aria-label="Indexing progress"
                  value={indexedPct}
                  showInfo
                  format={() => `${inProgress} indexing`}
                />
              </div>
            )}
          </Card>

          <Card data-testid="kb-detail-documents" title="Documents">
            {kbId && <KnowledgeBaseDocumentsPanel kbId={kbId} />}
          </Card>
        </div>
      </DivScrollY>

      <KnowledgeBaseFormDrawer
        open={editOpen}
        editing={kb}
        onClose={() => setEditOpen(false)}
      />
    </div>
  )
}
