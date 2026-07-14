import { useState } from 'react'
import { Download, Import, ShieldCheck } from 'lucide-react'
import { Button, Card, Space, Spin, Text, Empty, Dropdown, ErrorState } from '@ziee/kit'
import { message } from '@ziee/kit'
import { Permissions } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { Stores } from '@/core/stores'
import { CitationCard } from '../components/CitationCard'
import { ImportCitationsModal } from '../components/ImportCitationsModal'

const EXPORT_FORMATS: { key: string; label: string; ext: string; mime: string }[] = [
  { key: 'text', label: 'Formatted (CSL style)', ext: 'txt', mime: 'text/plain' },
  { key: 'bibtex', label: 'BibTeX (.bib)', ext: 'bib', mime: 'application/x-bibtex' },
  { key: 'ris', label: 'RIS (.ris)', ext: 'ris', mime: 'application/x-research-info-systems' },
  { key: 'csljson', label: 'CSL-JSON (.json)', ext: 'json', mime: 'application/json' },
]

function download(content: string, filename: string, mime: string) {
  const blob = new Blob([content], { type: `${mime};charset=utf-8` })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

export function CitationsSettingsPage() {
  const { entries, loading, importing, verifying, error } = Stores.Citations
  // Import / Delete require `citations::manage`; Verify-all + Export are `use`.
  const canManage = usePermission(Permissions.CitationsManage)
  const [importOpen, setImportOpen] = useState(false)

  const handleVerifyAll = async () => {
    try {
      const report = await Stores.Citations.verifyAll()
      const verified = report.results.filter(
        r => r.verification_status === 'verified',
      ).length
      const bad = report.results.length - verified
      message.info(`Verified ${verified}; ${bad} need attention.`)
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Verify failed')
    }
  }

  const handleExport = async (format: string) => {
    try {
      const out = await Stores.Citations.exportLibrary(format)
      const fmt = EXPORT_FORMATS.find(f => f.key === format)
      download(out, `citations.${fmt?.ext ?? 'txt'}`, fmt?.mime ?? 'text/plain')
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Export failed')
    }
  }

  return (
    <SettingsPageContainer
      title="Citations"
      subtitle="Your verified bibliography library. Import references, verify they resolve to real records, and export in a citation style."
    >
      <Card data-testid="cite-settings-card">
        <Space className="mb-3" wrap>
          {canManage && (
            <Button
              variant="outline"
              icon={<Import />}
              loading={importing}
              onClick={() => setImportOpen(true)}
              data-testid="cite-settings-import-button"
            >
              Import
            </Button>
          )}
          <Button
            icon={<ShieldCheck />}
            loading={verifying}
            disabled={entries.length === 0 || !canManage}
            onClick={handleVerifyAll}
            data-testid="cite-settings-verify-all-button"
          >
            Verify all
          </Button>
          <Dropdown
            disabled={entries.length === 0}
            items={EXPORT_FORMATS.map(f => ({ key: f.key, label: f.label }))}
            onSelect={(key) => void handleExport(key)}
            data-testid="cite-settings-export-dropdown"
          >
            <Button icon={<Download />} data-testid="cite-settings-export-button">Export</Button>
          </Dropdown>
          <Text type="secondary">{entries.length} reference(s)</Text>
        </Space>

        {loading ? (
          <Spin label="Loading" />
        ) : entries.length === 0 ? (
          error ? (
            <ErrorState
              resource="citations"
              description="Your bibliography couldn't be loaded. Check your connection and try again."
              details={error}
              onRetry={() => void Stores.Citations.load()}
              data-testid="cite-settings-error"
            />
          ) : (
            <Empty data-testid="cite-settings-empty" />
          )
        ) : (
          <div>
            {entries.map(e => (
              <CitationCard key={e.id} entry={e} canManage={canManage} />
            ))}
          </div>
        )}
      </Card>

      <ImportCitationsModal
        open={importOpen}
        onClose={() => setImportOpen(false)}
      />
    </SettingsPageContainer>
  )
}
