import { Card, Flex, FormField, Switch, Alert, useWatch } from '@/components/ui'

export function LlmModelCapabilitiesSection() {
  // Memory-plan §8 polish (gap #12): when text_embedding is ticked,
  // the model is an embedder. Chat-only capabilities (chat, vision,
  // audio, tools, code interpreter) and image_generator don't apply
  // — gray them out and surface a hint instead of letting an admin
  // tick contradictory flags.
  const isEmbedding = useWatch({ name: 'capabilities.text_embedding' })
  const isRerank = useWatch({ name: 'capabilities.rerank' })
  // An embedder OR a reranker is a non-chat model — chat capabilities don't
  // apply, so gray them out (same as the embedder rule).
  const grayed = Boolean(isEmbedding || isRerank)

  return (
    <Card title="Capabilities" data-testid="llm-capabilities-card">
      <Flex vertical className="gap-2 w-full">
        {grayed && (
          <Alert
            tone="info"
            className="!mb-1"
            data-testid="llm-capabilities-embedder-alert"
            title="This model is flagged as an embedder or reranker; chat capabilities are hidden because they don't apply."
          />
        )}

        <CapabilityRow
          label="Text Embedding"
          name="text_embedding"
          help="Generates vectors instead of chat text. Used by the Memory module."
        />

        <CapabilityRow
          label="Reranker"
          name="rerank"
          help="Cross-encoder that re-scores retrieved passages. Used by Document RAG / knowledge bases to improve retrieval quality."
        />

        {!grayed && (
          <>
            <CapabilityRow label="Chat" name="chat" />
            <CapabilityRow label="Vision" name="vision" />
            <CapabilityRow label="Audio" name="audio" />
            <CapabilityRow label="Tools" name="tools" />
            <CapabilityRow label="Code Interpreter" name="codeInterpreter" />
            <CapabilityRow label="Image Generator" name="image_generator" />
          </>
        )}
      </Flex>
    </Card>
  )
}

function CapabilityRow({
  label,
  name,
  help,
}: {
  label: string
  name: string
  help?: string
}) {
  return (
    <div className="flex items-center justify-between gap-3 min-h-9">
      {/* Label (+ optional help as a description line beneath it) takes the row;
          min-w-0 lets it truncate/wrap instead of crushing the toggle. */}
      <div className="min-w-0">
        <span className="text-sm">{label}</span>
        {help && (
          <span className="text-muted-foreground text-xs block">{help}</span>
        )}
      </div>
      {/* w-auto shrink-0: the Field defaults to w-full, which would stretch across
          the row and jam the Switch against the label — override it so
          justify-between can right-align the toggle. */}
      <FormField
        name={`capabilities.${name}`}
        aria-label={label}
        valuePropName="checked"
        className="mb-0 w-auto shrink-0"
      >
        <Switch data-testid={`llm-capability-switch-${name}`} />
      </FormField>
    </div>
  )
}
