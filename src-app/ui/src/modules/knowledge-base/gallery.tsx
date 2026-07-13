/**
 * Dev-gallery seed for the `knowledge-base` module — the `/knowledge` list, the
 * `/knowledge/:kbId` detail page (documents + retrieval + usage), and the
 * create/edit knowledge-base drawer overlay. Auto-discovered by the gallery's
 * runtime registry (`@/dev/gallery/support`); never imported by `module.tsx`, so
 * it is dev-only and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { lazyBound } from '@/dev/gallery/support'
import type { IndexingSummary, KnowledgeBase } from '@/api-client/types'

const summary = (over: Partial<IndexingSummary> = {}): IndexingSummary => ({
  total: 12,
  indexed: 10,
  indexing: 1,
  pending: 1,
  failed: 0,
  no_text: 0,
  ...over,
})

const KB_1 = 'kb000000-0000-0000-0000-000000000001'
const KB_2 = 'kb000000-0000-0000-0000-000000000002'

const knowledgeBases: KnowledgeBase[] = [
  {
    id: KB_1,
    name: 'Lab protocols',
    description: 'SOPs, assay methods, and reagent prep guides.',
    document_count: 12,
    indexing_summary: summary(),
    created_at: '2025-12-01T00:00:00.000Z',
    updated_at: '2026-01-05T00:00:00.000Z',
  },
  {
    id: KB_2,
    name: 'Grant references',
    description: 'Background literature for the R01 resubmission.',
    document_count: 34,
    indexing_summary: summary({ total: 34, indexed: 34, indexing: 0, pending: 0 }),
    created_at: '2025-11-10T00:00:00.000Z',
    updated_at: '2026-01-04T00:00:00.000Z',
  },
]

const byId = (id: string): KnowledgeBase =>
  knowledgeBases.find(k => k.id === id) ?? { ...knowledgeBases[0], id }

export const gallery: ModuleGallery = {
  cassette: {
    'KnowledgeBase.list': knowledgeBases,
    'KnowledgeBase.get': ctx => byId(ctx.params.id),
    'KnowledgeBase.listDocuments': [
      {
        file_id: 'f0000000-0000-0000-0000-000000000001',
        filename: 'rna-extraction-sop.pdf',
        mime_type: 'application/pdf',
        file_size: 348_160,
        has_thumbnail: true,
        preview_page_count: 4,
        chunk_count: 22,
        index_status: 'indexed',
        added_at: '2025-12-01T00:00:00.000Z',
      },
      {
        file_id: 'f0000000-0000-0000-0000-000000000002',
        filename: 'qpcr-assay-methods.docx',
        mime_type:
          'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
        file_size: 51_200,
        has_thumbnail: false,
        preview_page_count: 2,
        chunk_count: 8,
        index_status: 'indexing',
        added_at: '2025-12-20T00:00:00.000Z',
      },
      {
        file_id: 'f0000000-0000-0000-0000-000000000003',
        filename: 'scanned-gel-image.pdf',
        mime_type: 'application/pdf',
        file_size: 1_048_576,
        has_thumbnail: false,
        preview_page_count: 1,
        chunk_count: 0,
        index_status: 'no_text',
        added_at: '2026-01-02T00:00:00.000Z',
      },
    ],
    'KnowledgeBase.retrievalInfo': {
      embedding_configured: true,
      mode: 'hybrid_rerank',
      rerank_enabled: true,
    },
    'KnowledgeBase.usage': {
      conversations: [
        { id: 'c0000000-0000-0000-0000-000000000001', label: 'Protocol Q&A' },
      ],
      projects: [
        { id: 'p0000000-0000-0000-0000-000000000001', label: 'R01 resubmission' },
      ],
    },
  },
  overlays: [
    {
      // Prop-driven overlay (open/editing/onClose props, no store open action) →
      // rendered open via bound props (lazyBound), mirroring the file module's
      // prop-driven surfaces.
      slug: 'overlay-knowledge-base-form-drawer',
      surface: 'modules/knowledge-base/components/KnowledgeBaseFormDrawer',
      title: 'Knowledge base — create/edit (drawer)',
      component: lazyBound(
        () => import('@/modules/knowledge-base/components/KnowledgeBaseFormDrawer'),
        'KnowledgeBaseFormDrawer',
        { open: true, editing: null, onClose: () => {} },
      ),
    },
  ],
}
