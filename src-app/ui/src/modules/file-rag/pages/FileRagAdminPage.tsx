import { Alert, Spinner } from '@/components/ui'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { EnableSection } from '../components/sections/EnableSection'
import { EmbeddingSection } from '../components/sections/EmbeddingSection'
import { RerankSection } from '../components/sections/RerankSection'
import { ChunkingSection } from '../components/sections/ChunkingSection'
import { FullTextSection } from '../components/sections/FullTextSection'
import { MaintenanceSection } from '../components/sections/MaintenanceSection'

/**
 * Deployment-wide Document-RAG (file_rag) admin settings. Stacked section
 * cards, each with its own form so a save is scoped to one concern.
 *
 * Card order:
 *   1. Document search — master enable + shared retrieval top-K
 *   2. Embedding       — the vector arm: model picker + cosine cutoff + re-embed
 *   3. Chunking        — window size / overlap / per-file cap
 *   4. Full-text       — lexical arm tuning (works with no embedder)
 *   5. Maintenance     — backfill existing files
 *
 * Default is ON (FTS from day one); the vector arm activates once an embedding
 * model is configured under "Embedding".
 */
export function FileRagAdminPage() {
  const { settings, loading, error } = Stores.FileRagAdmin
  return (
    <SettingsPageContainer
      title="Document RAG"
      subtitle="Deployment-wide document retrieval: master toggle, embedding model, chunking, full-text tuning, and backfill. On by default — full-text search works immediately; semantic search activates when you set an embedding model."
    >
      {/* Surface load failures (the per-section cards render nothing
          until settings arrive, so without this the page body is blank
          on error). */}
      {error && !settings && (
        <Alert
          tone="error"
          className="mb-4"
          data-testid="file-rag-admin-load-error"
          title="Failed to load Document RAG settings"
          description={error}
        />
      )}
      {/* Spinner while the first load is in flight so the body isn't blank. */}
      {loading && !settings && (
        <div className="flex justify-center py-8">
          <Spinner label="Loading Document RAG settings" />
        </div>
      )}
      <EnableSection />
      <EmbeddingSection />
      <RerankSection />
      <ChunkingSection />
      <FullTextSection />
      <MaintenanceSection />
    </SettingsPageContainer>
  )
}
