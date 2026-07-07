/**
 * Project DETAIL deep-state fixtures — a RICH project used to drive the
 * full-page `ProjectDetailPage` gallery surface (`deep-project-detail`).
 *
 * The GET-driven page pass renders `/projects/:projectId` from the mock
 * cassette, but the cassette's project is thin (and its conversation/file lists
 * empty), so the enumerated page never shows the loaded page with real content.
 * These fixtures are seeded through the REAL `ProjectDetail` + `ProjectFiles`
 * stores (via `holdPatch`) so the page renders its populated form: an instructed
 * project with a description, a multi-item conversation list, and attached
 * knowledge files.
 *
 * Everything here is typed against the generated api-client response types, so a
 * shape drift fails `tsc` (same correctness contract as the recorded cassettes).
 */
import type {
  ConversationResponse,
  File as FileEntity,
  Project,
} from '@/api-client/types'
import type { Cassette } from '../mockApi'

const NOW = '2026-07-05T20:16:49.884839Z'
const PROJECT_USER_ID = 'aaaa0000-0000-0000-0000-000000000001'

/** The rich showcase project id the `deep-project-detail` surface pins to. */
export const DEEP_PROJECT_ID = 'proj-deep-0000-0000-0000-000000000001'

/** A fully-populated project: description + model-facing instructions. */
export const deepProject: Project = {
  id: DEEP_PROJECT_ID,
  user_id: PROJECT_USER_ID,
  name: 'GLP-1 receptor agonists — literature review',
  description:
    'Systematic review of cardiovascular outcomes for GLP-1 receptor agonists ' +
    'across the SUSTAIN, LEADER, and SELECT trial families. Shared context for ' +
    'every conversation filed here.',
  instructions:
    'You are assisting a clinical researcher. Always cite primary sources with a ' +
    'resolvable DOI or PMID; never invent a citation. Prefer randomized controlled ' +
    'trials over observational data. When summarizing a trial, state the primary ' +
    'endpoint, the hazard ratio with its 95% CI, and the population n.',
  default_assistant_id: undefined,
  default_model_id: 'claude-opus-4-8',
  created_at: '2026-05-02T09:12:00.000000Z',
  updated_at: NOW,
}

function conversation(
  id: string,
  title: string,
  messageCount: number,
  updatedAt: string,
): ConversationResponse {
  return {
    id,
    title,
    user_id: PROJECT_USER_ID,
    active_branch_id: `${id.slice(0, 8)}-branch-0001`,
    message_count: messageCount,
    model_id: 'claude-opus-4-8',
    created_at: '2026-05-04T10:00:00.000000Z',
    updated_at: updatedAt,
  }
}

/** A realistic multi-item conversation list filed under this project. */
export const deepProjectConversations: ConversationResponse[] = [
  conversation(
    'convd001-0000-4000-8000-000000000001',
    'SELECT trial — primary CV endpoint breakdown',
    24,
    '2026-07-05T18:40:00.000000Z',
  ),
  conversation(
    'convd002-0000-4000-8000-000000000002',
    'Semaglutide vs tirzepatide — head-to-head evidence',
    11,
    '2026-07-04T14:05:00.000000Z',
  ),
  conversation(
    'convd003-0000-4000-8000-000000000003',
    'LEADER trial — renal secondary outcomes',
    8,
    '2026-07-02T11:22:00.000000Z',
  ),
  conversation(
    'convd004-0000-4000-8000-000000000004',
    'Draft: inclusion / exclusion criteria table',
    5,
    '2026-06-29T09:00:00.000000Z',
  ),
  conversation(
    'convd005-0000-4000-8000-000000000005',
    'Adverse-event signal — pancreatitis meta-analysis',
    17,
    '2026-06-27T16:48:00.000000Z',
  ),
]

function file(
  id: string,
  filename: string,
  size: number,
  mime: string,
): FileEntity {
  return {
    id,
    user_id: PROJECT_USER_ID,
    created_by: PROJECT_USER_ID,
    filename,
    file_size: size,
    mime_type: mime,
    checksum: `sha256:${id}`,
    blob_version_id: `${id}-v1`,
    current_version_id: `${id}-v1`,
    version: 1,
    has_thumbnail: false,
    preview_page_count: 0,
    text_page_count: 0,
    processing_metadata: {},
    created_at: '2026-05-03T08:00:00.000000Z',
    updated_at: NOW,
  }
}

/**
 * Cassette so the REAL load paths return well-shaped bodies for the deep project.
 * Without a `Project.listFiles` entry the mock's `makeSafeEmpty()` fallback
 * serializes to a bare `[]`, so the store's `response.files` is `undefined` →
 * `ProjectFilesInlinePreview` crashes on `files.length`. A typed `FileListResponse`
 * keeps every project load path well-formed (the deep surface then `holdPatch`es
 * the rich lists over these on top). Values are typed against the api-client
 * response types, so a shape drift fails `tsc`.
 */
export const projectDeepCassette: Cassette = {
  'Project.get': ({ params }) =>
    params.id === DEEP_PROJECT_ID ? deepProject : { ...deepProject, id: params.id },
  'Project.listFiles': { files: [], total: 0 },
  'Project.listConversations': [],
}

/** Attached knowledge files (the project's inline knowledge card). */
export const deepProjectFiles: FileEntity[] = [
  file(
    'filed001-0000-4000-8000-000000000001',
    'SELECT-trial-protocol.pdf',
    2_418_112,
    'application/pdf',
  ),
  file(
    'filed002-0000-4000-8000-000000000002',
    'inclusion-exclusion-criteria.csv',
    18_442,
    'text/csv',
  ),
  file(
    'filed003-0000-4000-8000-000000000003',
    'endpoint-adjudication-notes.md',
    9_204,
    'text/markdown',
  ),
]
