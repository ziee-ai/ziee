import { useState } from 'react'
import { Search } from 'lucide-react'
import { Button, Card, Flex, Input, Tag, Text } from '@ziee/kit'
import { Stores } from '@ziee/framework/stores'
import { ApiClient } from '@/api-client'
import type { KnowledgeSearchHit } from '@/api-client/types'
import { KnowledgeBaseDetail } from '@/modules/knowledge-base/stores/knowledgeBaseDetail'
import { FilePreviewDrawer } from '@/modules/file/stores/filePreviewDrawer'
import { PdfHighlight as PdfHighlightStore } from '@/modules/file/stores/pdfHighlight'

/**
 * Detail-page "test retrieval" box (FB-8 / DEC-40) — runs the same retrieval
 * the agent uses (`POST /knowledge-bases/{id}/search`) so a user can verify
 * "is my 2019 paper actually retrievable?" without opening a chat. Each hit
 * opens the source document at the cited page with an exact-passage highlight
 * (reuses the global `PdfHighlight` mechanism the chat `kb_source` panel uses).
 */
export function KnowledgeBaseSearchPanel({ kbId }: { kbId: string }) {
  const { searching, searchResults } = KnowledgeBaseDetail
  const [q, setQ] = useState('')

  const run = () => void KnowledgeBaseDetail.searchKb(kbId, q)

  const openHit = async (h: KnowledgeSearchHit) => {
    const file = await Stores.File.getFileEntityById(h.file_id)
    if (file.mime_type === 'application/pdf') {
      try {
        const res = await ApiClient.File.getTextRects({
          file_id: h.file_id,
          page: h.page_number,
          start: h.char_start,
          end: h.char_end,
        })
        PdfHighlightStore.setTarget(h.file_id, { page: h.page_number, rects: res.rects })
      } catch {
        PdfHighlightStore.setTarget(h.file_id, { page: h.page_number, rects: [] })
      }
    } else {
      // Non-PDF: drive find-in-document to the passage prefix (scroll+highlight).
      Stores.File.setFileFindQuery(h.file_id, (h.content ?? '').trim().slice(0, 60))
    }
    FilePreviewDrawer.openPreview(file)
  }

  const inc = searchResults?.indexing_incomplete
  const incomplete = !!inc && inc.searchable < inc.total

  return (
    <Card data-testid="kb-detail-search" title="Test retrieval">
      <Flex gap="small" align="center">
        <Input
          data-testid="kb-search-input"
          value={q}
          onChange={e => setQ(e.target.value)}
          onKeyDown={e => {
            if (e.key === 'Enter') run()
          }}
          placeholder="Search this knowledge base the way the agent would…"
        />
        <Button
          data-testid="kb-search-button"
          icon={<Search />}
          loading={searching}
          onClick={run}
        >
          Search
        </Button>
      </Flex>

      {searchResults && (
        <div className="mt-3">
          <Text type="secondary" className="text-xs block mb-1">
            {searchResults.hits.length} passage
            {searchResults.hits.length === 1 ? '' : 's'} · {searchResults.mode.toLowerCase()}
          </Text>
          {incomplete && (
            <Text type="warning" className="text-xs block mb-1" data-testid="kb-search-incomplete">
              Corpus not fully indexed: {inc!.searchable} of {inc!.total} documents searchable
              — results may be partial.
            </Text>
          )}
          {searchResults.hits.length === 0 ? (
            <Text type="secondary" className="text-xs block" data-testid="kb-search-empty">
              No passages matched this query.
            </Text>
          ) : (
            <ul className="space-y-2" data-testid="kb-search-hits">
              {searchResults.hits.map((h, i) => (
                <li key={`${h.file_id}-${h.char_start}-${i}`} className="rounded-md border border-border p-2">
                  <div className="flex items-center gap-2 mb-1">
                    <Tag
                      variant="outline"
                      tone="info"
                      className="m-0"
                      data-testid={`kb-search-hit-source-${i}`}
                    >
                      {h.filename || 'document'} · p{h.page_number}
                    </Tag>
                    <Text type="secondary" className="text-xs">
                      score {h.score.toFixed(3)}
                    </Text>
                    <Button
                      size="default"
                      variant="link"
                      className="ms-auto"
                      data-testid={`kb-search-open-${i}`}
                      onClick={() => void openHit(h)}
                    >
                      Open source
                    </Button>
                  </div>
                  <Text className="text-xs block [overflow-wrap:anywhere] line-clamp-3">
                    {h.content}
                  </Text>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </Card>
  )
}
