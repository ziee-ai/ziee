import type { File as FileEntity } from '@/api-client/types'

/** Source extensions the code editor (CodeMirror) handles as plain text. */
const CODE_EXTS = new Set([
  'js', 'jsx', 'ts', 'tsx', 'mjs', 'cjs', 'py', 'rs', 'go', 'java', 'c', 'h',
  'cpp', 'cc', 'hpp', 'rb', 'sh', 'bash', 'zsh', 'sql', 'json', 'jsonc', 'yaml',
  'yml', 'toml', 'xml', 'css', 'scss', 'less', 'html', 'htm', 'php', 'swift',
  'kt', 'lua', 'r', 'pl', 'ini', 'env', 'txt', 'text', 'log', 'diff', 'patch',
  'graphql', 'proto', 'dockerfile', 'makefile', 'gradle',
])

export type EditableKind = 'markdown' | 'code' | 'csv' | null

/**
 * Which canvas editor (if any) can edit this file. markdown → Plate WYSIWYG;
 * csv → editable grid; code → CodeMirror; everything else (pdf/image/office/
 * binary) is view-only. Single source used by both the FilePanel edit toggle and
 * the edit body. Order matters: csv is checked before the generic text→code
 * fallback (`text/csv` also starts with `text/`).
 */
export function editableKind(file: FileEntity): EditableKind {
  const ext = file.filename.split('.').pop()?.toLowerCase() ?? ''
  if (ext === 'md' || ext === 'markdown' || file.mime_type === 'text/markdown') {
    return 'markdown'
  }
  if (ext === 'csv' || file.mime_type === 'text/csv') {
    return 'csv'
  }
  if (CODE_EXTS.has(ext) || file.mime_type?.startsWith('text/')) {
    return 'code'
  }
  return null
}
