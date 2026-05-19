import { Fragment } from 'react'
import { Tag } from 'antd'
import { Stores } from '@/core/stores'
import type { Annotation, MessageContent, MessageContentDataAnnotatedText } from '@/api-client/types'

// Matches any [id] bracket in the text (annotation markers can be arbitrary strings)
const ANNOTATION_MARKER_RE = /\[([^\]]+)\]/g

interface AnnotationBadgeProps {
  displayNumber: number
  annotation: Annotation | undefined
  onOpen: (annotation: Annotation) => void
}

function AnnotationBadge({ displayNumber, annotation, onOpen }: AnnotationBadgeProps) {
  return (
    <sup style={{ display: 'inline-flex', verticalAlign: 'super', lineHeight: 1 }}>
      <Tag
        color="blue"
        style={{
          cursor: annotation ? 'pointer' : 'default',
          margin: 0,
          padding: '0 4px',
          fontSize: 10,
          lineHeight: '16px',
          borderRadius: 3,
          border: '1px solid #91caff',
        }}
        onClick={() => annotation && onOpen(annotation)}
      >
        {displayNumber}
      </Tag>
    </sup>
  )
}

interface AnnotatedContentRendererProps {
  content: MessageContent
}

/**
 * Renders a final MCP answer with clickable inline annotation badges.
 *
 * The server uses arbitrary IDs (e.g. chunk-abc-000001) as inline markers.
 * This renderer scans the text for [id] patterns that match any annotation ID,
 * assigns sequential display numbers ([1], [2], …) in order of first appearance,
 * and renders a clickable badge for each. Clicking opens the annotation drawer
 * which renders the server-provided content via AnnotationDrawer.
 *
 * Type-agnostic: works for any annotation_type (citation, image, audio, file, etc.).
 * Type-specific rendering is handled by AnnotationDrawer, not here.
 */
export function AnnotatedContentRenderer({ content }: AnnotatedContentRendererProps) {
  const { setOpenAnnotation } = Stores.Chat.McpStore

  const data = content.content as MessageContentDataAnnotatedText
  const { text, annotations } = data

  if (!text) return null

  // Build a lookup map from annotation id → Annotation
  const annotationById = new Map<string, Annotation>(annotations.map(a => [a.id, a]))

  // Assign sequential display numbers in order of first appearance in the text
  const idToNumber = new Map<string, number>()
  let counter = 1
  for (const match of text.matchAll(ANNOTATION_MARKER_RE)) {
    const id = match[1]
    if (annotationById.has(id) && !idToNumber.has(id)) {
      idToNumber.set(id, counter++)
    }
  }

  // Split text into text segments and annotation badges
  type Part =
    | { type: 'text'; value: string }
    | { type: 'badge'; id: string; displayNumber: number }

  const parts: Part[] = []
  let lastIndex = 0

  for (const match of text.matchAll(ANNOTATION_MARKER_RE)) {
    const id = match[1]
    const displayNumber = idToNumber.get(id)
    if (displayNumber === undefined) continue // not a known annotation → leave as-is

    const start = match.index!
    if (start > lastIndex) {
      parts.push({ type: 'text', value: text.slice(lastIndex, start) })
    }
    parts.push({ type: 'badge', id, displayNumber })
    lastIndex = start + match[0].length
  }
  if (lastIndex < text.length) {
    parts.push({ type: 'text', value: text.slice(lastIndex) })
  }

  return (
    <div className="w-full overflow-hidden pt-2 pl-2">
      <div style={{ whiteSpace: 'pre-wrap' }}>
        {parts.map((part, i) =>
          part.type === 'text' ? (
            <span key={i}>{part.value}</span>
          ) : (
            <Fragment key={i}>
              <AnnotationBadge
                displayNumber={part.displayNumber}
                annotation={annotationById.get(part.id)}
                onOpen={ann => setOpenAnnotation(ann)}
              />
            </Fragment>
          ),
        )}
      </div>
    </div>
  )
}
