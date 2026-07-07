import { FileText } from 'lucide-react'
import { Card, Empty, ScrollArea, Spinner, Tag, Text, Title } from '@/components/ui'
import type { OfficeApp, OpenDoc } from '@/api-client/types'
import { Stores } from '@/core/stores'

/**
 * The "Open Office documents" right-panel (registered as the `office-bridge`
 * panel renderer). Lists the user's currently-open Word/Excel/PowerPoint
 * documents grouped by application, each row showing name + folder path and
 * saved/active status tags.
 *
 * Reads the LIVE list from `Stores.OfficeBridge` (kept fresh by the store's
 * `sync:office_document` refetch); falls back to the serialized tab `snapshot`
 * before the store's first fetch resolves (e.g. right after the tool-result card
 * opens the panel, or on a rehydrated conversation).
 */
const APP_LABELS: Record<OfficeApp, string> = {
  word: 'Word',
  excel: 'Excel',
  power_point: 'PowerPoint',
}
const APP_ORDER: OfficeApp[] = ['word', 'excel', 'power_point']

export function OpenDocumentsPanel({ documents: snapshot = [] }: { documents?: OpenDoc[] }) {
  const { documents: live, loading } = Stores.OfficeBridge
  const documents = live.length > 0 ? live : snapshot

  // First load with nothing to show yet → spinner (a refetch that already has
  // documents keeps the list visible instead of flashing to a spinner).
  if (loading && documents.length === 0) {
    return (
      <div
        className="flex h-full items-center justify-center p-6"
        data-testid="office-docs-panel-loading"
      >
        <Spinner label="Loading open Office documents" />
      </div>
    )
  }

  if (documents.length === 0) {
    return (
      <div
        className="flex h-full items-center justify-center p-6"
        data-testid="office-docs-panel-empty"
      >
        <Empty data-testid="office-docs-empty" description="No open Office documents" />
      </div>
    )
  }

  const groups = APP_ORDER.map(app => ({
    app,
    docs: documents.filter(d => d.app === app),
  })).filter(g => g.docs.length > 0)

  return (
    <ScrollArea className="h-full">
      <div className="flex flex-col gap-4 p-3" data-testid="office-docs-panel">
        <Title level={5} className="!mb-0">
          Open Office documents
        </Title>
        {groups.map(group => (
          <div key={group.app} className="flex flex-col gap-2">
            <Text strong className="text-xs uppercase text-muted-foreground">
              {APP_LABELS[group.app]} ({group.docs.length})
            </Text>
            {group.docs.map((doc, i) => (
              <Card
                key={`${group.app}-${i}`}
                size="sm"
                data-testid={`office-doc-card-${group.app}-${i}`}
              >
                <div className="flex items-start gap-2">
                  <FileText className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
                  <div className="min-w-0 flex-1">
                    <Text strong className="block truncate text-sm">
                      {doc.name}
                    </Text>
                    {doc.path && (
                      <Text
                        type="secondary"
                        className="block truncate text-xs [overflow-wrap:anywhere]"
                      >
                        {doc.path}
                      </Text>
                    )}
                    <div className="mt-1 flex flex-wrap gap-1">
                      {doc.active && (
                        <Tag
                          variant="outline"
                          tone="info"
                          data-testid={`office-doc-active-${group.app}-${i}`}
                        >
                          Active
                        </Tag>
                      )}
                      <Tag
                        variant="outline"
                        tone={doc.saved ? 'success' : 'warning'}
                        data-testid={`office-doc-saved-${group.app}-${i}`}
                      >
                        {doc.saved ? 'Saved' : 'Unsaved'}
                      </Tag>
                    </div>
                  </div>
                </div>
              </Card>
            ))}
          </div>
        ))}
      </div>
    </ScrollArea>
  )
}
