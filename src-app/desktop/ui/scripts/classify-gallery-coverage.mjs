/**
 * One-shot helper: (re)classify coverage.ts entries from `pending()` into honest
 * coverage kinds by directory/name heuristics + the route-element (page) map.
 * The result is HAND-MAINTAINED thereafter; re-run only to re-bootstrap.
 *
 * Kinds assigned:
 *   page   — the file is a route element (rendered as a seeded gallery page).
 *   via    — transitively rendered: kit/shadcn primitives (kit stories) or a
 *            module component rendered within its module's page.
 *   allow  — non-visual (context/provider/notification-listener/types/guards).
 *   pending— interaction-only (drawer/dialog/modal/sheet) — needs an open-state
 *            entry; the honest remaining work, surfaced by the parity report.
 *
 * Run: node scripts/classify-gallery-coverage.mjs
 */
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = path.dirname(fileURLToPath(import.meta.url))
const UI = path.resolve(HERE, '..')
const COVERAGE = path.resolve(UI, 'src/dev/gallery/coverage.ts')
const GEN = path.resolve(UI, 'src/dev/gallery/galleryCoverage.generated.ts')

// Route-element files → gallery pages (from module.tsx route registrations).
const PAGE_FILES = new Set([
  'modules/app/SetupPage',
  'modules/assistant/pages/AssistantsSettings',
  'modules/assistant/pages/UserAssistantsSettings',
  'modules/auth-providers/AuthProvidersSettingsPage',
  'modules/auth/AuthCallbackPage',
  'modules/auth/AuthPage',
  'modules/auth/LinkAccountPage',
  'modules/auth/SessionSettingsPage',
  'modules/chat/pages/NewChatPage',
  'modules/citations/pages/CitationsSettingsPage',
  'modules/code-sandbox/components/SandboxSettingsPage',
  'modules/file-rag/pages/FileRagAdminPage',
  'modules/hardware/HardwareMonitor',
  'modules/hardware/HardwareSettings',
  'modules/hub/HubPage',
  'modules/literature/components/settings/LitSearchSettingsPage',
  'modules/literature/components/settings/LitSearchUserKeysPage',
  'modules/llm-local-runtime/components/RuntimeVersionSettings',
  'modules/llm-provider/components/LlmProviderSettings',
  'modules/llm-repository/components/LlmRepositorySettings',
  'modules/mcp/components/system/SystemMcpServersPage',
  'modules/mcp/components/user/McpServersSettings',
  'modules/memory/pages/MemoryAdminPage',
  'modules/memory/pages/MemorySettingsPage',
  'modules/profile/pages/ProfileSettingsPage',
  'modules/projects/pages/ProjectDetailPage',
  'modules/projects/pages/ProjectsListPage',
  'modules/server-update/AboutSettings',
  'modules/settings-general/GeneralSettings',
  'modules/skill/components/SkillsList',
  'modules/skill/components/admin/AdminSkillsPage',
  'modules/summarization/pages/SummarizationAdminPage',
  'modules/user/components/group/UserGroupsSettings',
  'modules/user/components/user/UsersSettings',
  'modules/web-search/components/WebSearchSettingsPage',
  'modules/web-search/components/WebSearchUserKeysPage',
  'modules/workflow/components/WorkflowsList',
  'modules/workflow/components/admin/AdminWorkflowsPage',
])

const INTERACTION = /(Drawer|Dialog|Modal|Sheet|Popover|Menu)(\.|$)/
const NONVISUAL = /(Provider|Context|Notifications?|Listener|Guard|Boundary|types|constants|Registry|\.store)$/i

function classify(id) {
  const base = id.split('/').pop()
  if (PAGE_FILES.has(id)) return `page(${JSON.stringify(id)})`
  if (id.startsWith('components/ui/kit/') || id.startsWith('components/ui/shadcn/'))
    return `via('kit-stories')`
  if (id.startsWith('components/ui/'))
    return `allow('ui primitive/util — rendered via kit consumers')`
  if (INTERACTION.test(base))
    return `pending('interaction-only — needs an open-state gallery entry')`
  if (NONVISUAL.test(base))
    return `allow('non-visual — context/provider/listener/types')`
  const m = /^modules\/([^/]+)\//.exec(id)
  const mod = m ? m[1] : 'app'
  if (id.includes('/widgets/')) return `via('slot-widget in ${mod}')`
  return `via('rendered within the ${mod} module page')`
}

const surfaces = fs
  .readFileSync(GEN, 'utf8')
  .split('\n')
  .map(l => l.match(/^\s*"([^"]+)",/)?.[1])
  .filter(Boolean)

const entries = surfaces
  .map(id => `  ${JSON.stringify(id)}: ${classify(id)},`)
  .join('\n')

let cov = fs.readFileSync(COVERAGE, 'utf8')
cov = cov.replace(
  /export const GALLERY_COVERAGE = \{[\s\S]*?\n\} satisfies Record<GallerySurface, Coverage>/,
  `export const GALLERY_COVERAGE = {\n${entries}\n  // <<< scaffold-insert >>>\n} satisfies Record<GallerySurface, Coverage>`,
)
fs.writeFileSync(COVERAGE, cov)

const counts = {}
for (const id of surfaces) {
  const kind = classify(id).split('(')[0]
  counts[kind] = (counts[kind] || 0) + 1
}
console.log(`Classified ${surfaces.length} surfaces:`, JSON.stringify(counts))
