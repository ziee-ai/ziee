/**
 * Mermaid block — the code⇄render toggle renderer (`components/common/
 * MermaidBlock`). Exercises BOTH modes plus the edge states so the visual /
 * runtime gate covers the toggle end to end (closes AFFORDANCE_MATRIX G1/G2).
 */
import { MermaidBlock } from '@/components/common/MermaidBlock'
import type { GalleryStory } from '../story'

const VALID = [
  'graph TD',
  '  A[Start] --> B{Decision}',
  '  B -->|yes| C[Done]',
  '  B -->|no| A',
].join('\n')

// Deliberately malformed so the inline error state renders (no crash).
const INVALID = ['graph TD', '  A --> ((( broken syntax'].join('\n')

export const mermaidStories: GalleryStory[] = [
  {
    id: 'mermaid-block',
    title: 'Mermaid block (code⇄render toggle)',
    note: 'render + source modes, invalid-diagram error, streaming placeholder — closes AFFORDANCE_MATRIX G1/G2',
    cases: [
      {
        key: 'render',
        label: 'Render mode (default)',
        render: () => <MermaidBlock code={VALID} isIncomplete={false} language="mermaid" />,
      },
      {
        key: 'source',
        label: 'Source mode',
        render: () => (
          <MermaidBlock code={VALID} isIncomplete={false} language="mermaid" defaultMode="source" />
        ),
      },
      {
        key: 'error',
        label: 'Invalid diagram (inline error)',
        render: () => <MermaidBlock code={INVALID} isIncomplete={false} language="mermaid" />,
      },
      {
        key: 'streaming',
        label: 'Streaming (incomplete → deferred)',
        render: () => <MermaidBlock code={VALID} isIncomplete={true} language="mermaid" />,
      },
    ],
  },
]
