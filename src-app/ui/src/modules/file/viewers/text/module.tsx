import { FileText } from 'lucide-react'
import type { FileViewerModule } from '../../types/viewer'
import { TextBody } from './body'
import { TextHeader } from './header'

// Plain text / code extensions. No longer needs to exclude md/csv/etc. —
// specific viewers register their own extensions and win on equal priority
// by being declared more narrowly (not behind a wildcard).
const TEXT_EXTS = [
  'txt', 'json', 'xml', 'yaml', 'yml', 'log', 'ini', 'conf',
  'sh', 'bash', 'py', 'js', 'ts', 'jsx', 'tsx',
  'css', 'scss', 'sql', 'env', 'rs', 'go', 'java',
  'c', 'cpp', 'h', 'rb', 'php', 'swift', 'kt',
  'r', 'lua', 'pl', 'cs', 'dart', 'scala', 'hs',
]

export const viewers: FileViewerModule[] = [
  {
    // Priority 5 marks this as a generic-plaintext fallback — any specific
    // viewer (e.g. markdown for `.md`, JSON tree for `.json`) registered with
    // default priority 0 will preempt it without needing this list updated.
    supportedTypes: TEXT_EXTS.map(ext => ({ ext, priority: 5 })),
    entry: {
      body: TextBody,
      headerActions: TextHeader,
      label: 'Document',
      icon: <FileText />,
      // Plain text / code via RawCodeView. Covers logs, JSON, source
      // files — anything we fall back to text rendering for.
      inline: true,
    },
  },
]
