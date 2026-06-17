import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { EnableSection } from '../components/sections/EnableSection'
import { EmbeddingSection } from '../components/sections/EmbeddingSection'
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
  return (
    <SettingsPageContainer
      title="Document RAG (admin)"
      subtitle="Deployment-wide document retrieval: master toggle, embedding model, chunking, full-text tuning, and backfill. On by default — full-text search works immediately; semantic search activates when you set an embedding model."
    >
      <EnableSection />
      <EmbeddingSection />
      <ChunkingSection />
      <FullTextSection />
      <MaintenanceSection />
    </SettingsPageContainer>
  )
}
