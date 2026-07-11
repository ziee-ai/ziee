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
  Empty,
  Progress,
  Result,
  Spin,
  Tag,
  Text,
  Title,
} from '@/components/ui'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { DivScrollY } from '@/components/common/DivScrollY'
import { KnowledgeBaseDocumentsPanel } from '@/modules/knowledge-base/components/KnowledgeBaseDocumentsPanel'
import { KnowledgeBaseSearchPanel } from '@/modules/knowledge-base/components/KnowledgeBaseSearchPanel'
import { KnowledgeBaseFormDrawer } from '@/modules/knowledge-base/components/KnowledgeBaseFormDrawer'

/** Human label for the deployment retrieval mode line. */
const RETRIEVAL_LABEL: Record<string, string> = {
  hybrid_rerank: 'Hybrid + reranker',
  hybrid: 'Hybrid (semantic + keyword)',
  keyword_only: 'Keyword-only (no embedding model)',
}

export function KnowledgeBaseDetailPage() {
  const { kbId } = useParams<{ kbId: string }>()
  const navigate = useNavigate()
  const { kb, loading, retrievalInfo, usage } = Stores.KnowledgeBaseDetail
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
                ...(retrievalInfo
                  ? [{
                      key: 'retrieval',
                      label: 'Retrieval',
                      children: RETRIEVAL_LABEL[retrievalInfo.mode] ?? retrievalInfo.mode,
                    }]
                  : []),
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

          {/* Verify retrieval works on this KB, without opening a chat (DEC-40). */}
          {kbId && <KnowledgeBaseSearchPanel kbId={kbId} />}

          {/* The documents panel renders its OWN Card (title + count tag +
              Add-documents in the top-right extra) — checked against the
              surrounding Overview card + the LlmModelsSection/MyMemoriesSection
              title-with-extra precedent, not designed in isolation. */}
          {kbId && <KnowledgeBaseDocumentsPanel kbId={kbId} />}

          {/* Where this KB is used — scope legibility. */}
          <Card data-testid="kb-detail-used-in" title="Used in">
            {usage && (usage.conversations.length > 0 || usage.projects.length > 0) ? (
              <div className="flex flex-col gap-2">
                {usage.projects.length > 0 && (
                  <div className="flex items-center gap-2 flex-wrap">
                    <Text type="secondary" className="text-xs">Projects</Text>
                    {usage.projects.map(p => (
                      <Tag
                        key={p.id}
                        className="cursor-pointer focus-visible:outline focus-visible:outline-2"
                        data-testid={`kb-used-in-project-${p.id}`}
                        role="button"
                        tabIndex={0}
                        aria-label={`Open project ${p.label}`}
                        onClick={() => navigate(`/projects/${p.id}`)}
                        onKeyDown={e => {
                          if (e.key === 'Enter' || e.key === ' ') {
                            e.preventDefault()
                            navigate(`/projects/${p.id}`)
                          }
                        }}
                      >
                        {p.label}
                      </Tag>
                    ))}
                  </div>
                )}
                {usage.conversations.length > 0 && (
                  <div className="flex items-center gap-2 flex-wrap">
                    <Text type="secondary" className="text-xs">Chats</Text>
                    {usage.conversations.map(c => (
                      <Tag
                        key={c.id}
                        className="cursor-pointer focus-visible:outline focus-visible:outline-2"
                        data-testid={`kb-used-in-conversation-${c.id}`}
                        role="button"
                        tabIndex={0}
                        aria-label={`Open chat ${c.label}`}
                        onClick={() => navigate(`/chat/${c.id}`)}
                        onKeyDown={e => {
                          if (e.key === 'Enter' || e.key === ' ') {
                            e.preventDefault()
                            navigate(`/chat/${c.id}`)
                          }
                        }}
                      >
                        {c.label}
                      </Tag>
                    ))}
                  </div>
                )}
              </div>
            ) : (
              <Empty
                data-testid="kb-detail-used-in-empty"
                description="Not attached to any conversation or project yet."
              />
            )}
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
