import { Card, Flex, Form, Switch, Alert } from 'antd'

export function LlmModelCapabilitiesSection() {
  // Memory-plan §8 polish (gap #12): when text_embedding is ticked,
  // the model is an embedder. Chat-only capabilities (chat, vision,
  // audio, tools, code interpreter) and image_generator don't apply
  // — gray them out and surface a hint instead of letting an admin
  // tick contradictory flags.
  const isEmbedding = Form.useWatch(['capabilities', 'text_embedding'])
  const grayed = Boolean(isEmbedding)

  return (
    <Card title="Capabilities">
      <Flex vertical className="gap-2 w-full">
        {grayed && (
          <Alert
            type="info"
            showIcon
            className="!mb-1"
            message="This model is flagged as an embedder; chat capabilities are hidden because they don't apply."
          />
        )}

        <CapabilityRow
          label="Text Embedding"
          name="text_embedding"
          help="Generates vectors instead of chat text. Used by the Memory module."
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
    <div
      style={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
      }}
    >
      <span>
        {label}
        {help && (
          <span style={{ color: '#888', fontSize: 12, marginLeft: 8 }}>
            {help}
          </span>
        )}
      </span>
      <Form.Item
        name={['capabilities', name]}
        valuePropName="checked"
        style={{ marginBottom: 0 }}
      >
        <Switch />
      </Form.Item>
    </div>
  )
}
