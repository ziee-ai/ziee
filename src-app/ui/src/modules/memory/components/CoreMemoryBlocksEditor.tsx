import { useEffect, useState } from 'react'
import { z } from 'zod'
import {
  Title,
  Paragraph,
  Text,
  Card,
  Input,
  Textarea,
  Button,
  Dialog,
  Form,
  FormField,
  useForm,
  zodResolver,
  Confirm,
  message,
  Space,
  Empty,
  InputNumber,
} from '@/components/ui'
import { Trash2, Plus, Pencil } from 'lucide-react'
import { Stores } from '@/core/stores'
import type { CoreMemoryBlock } from '@/api-client/types'

/**
 * Letta-style core-memory block editor for an assistant. Renders below
 * the assistant's regular `instructions` field on the assistant edit
 * drawer. Backed by `/api/assistants/{id}/core-memory` (GET / PUT / DELETE).
 *
 * Each user gets their own set of blocks per assistant — the UI just
 * lists what's there and lets the user add / edit / delete. Plan §9
 * Phase 6: "Assistant designer UI to set/edit blocks".
 */
export function CoreMemoryBlocksEditor({
  assistantId,
}: {
  assistantId: string
}) {
  const { blocksByAssistant, loadingByAssistant } = Stores.CoreMemoryBlocks
  const blocks = blocksByAssistant[assistantId] ?? []
  const loading = loadingByAssistant[assistantId] ?? false
  const [editing, setEditing] = useState<CoreMemoryBlock | null>(null)
  const [creating, setCreating] = useState(false)

  useEffect(() => {
    if (assistantId) Stores.CoreMemoryBlocks.load(assistantId)
  }, [assistantId])

  return (
    <Card
      title={
        <Space>
          <Title level={5} className="!mb-0">
            Core memory blocks
          </Title>
        </Space>
      }
      extra={
        <Button
          size="sm"
          icon={<Plus />}
          onClick={() => setCreating(true)}
        >
          Add block
        </Button>
      }
    >
      <Paragraph type="secondary" className="!mb-3 text-xs">
        Core memory blocks (Letta-style) are prepended to the assistant&rsquo;s
        system prompt on every turn. Use them for persona / human
        context / standing instructions you want always in view —
        unlike auto-extracted memories, these are explicit and never
        get retrieved by similarity.
      </Paragraph>

      {blocks.length === 0 && !loading ? (
        <Empty description="No blocks yet" />
      ) : (
        <div className="space-y-2">
          {blocks.map(b => (
            <Card key={b.id} size="sm">
              <div className="flex items-start justify-between">
                <div className="flex-1">
                  <Text strong>{b.block_label}</Text>
                  <Paragraph
                    type="secondary"
                    className="!mt-1 !mb-0 line-clamp-2"
                  >
                    {b.content}
                  </Paragraph>
                </div>
                <Space>
                  <Button
                    icon={<Pencil />}
                    size="sm"
                    onClick={() => setEditing(b)}
                  />
                  <Confirm
                    title="Delete this block?"
                    description={`The "${b.block_label}" block will be removed from this assistant's core memory.`}
                    okText="Delete"
                    cancelText="Cancel"
                    okButtonProps={{ danger: true }}
                    onConfirm={async () => {
                      try {
                        await Stores.CoreMemoryBlocks.remove(
                          assistantId,
                          b.block_label,
                        )
                        message.success('Block deleted')
                      } catch (error) {
                        message.error(
                          error instanceof Error
                            ? error.message
                            : 'Delete failed',
                        )
                      }
                    }}
                  >
                    <Button
                      icon={<Trash2 />}
                      size="sm"
                      variant="destructive"
                      aria-label={`Delete block ${b.block_label}`}
                    />
                  </Confirm>
                </Space>
              </div>
            </Card>
          ))}
        </div>
      )}

      <BlockFormModal
        open={creating}
        assistantId={assistantId}
        onClose={() => setCreating(false)}
      />
      <BlockFormModal
        open={!!editing}
        assistantId={assistantId}
        existing={editing ?? undefined}
        onClose={() => setEditing(null)}
      />
    </Card>
  )
}

const blockSchema = z.object({
  block_label: z
    .string()
    .min(1, 'Required')
    .regex(/^[a-z0-9_-]{1,64}$/, '1-64 chars, lowercase letters, digits, _ or -'),
  content: z.string().min(1, 'Required').max(50_000),
  char_limit: z.number().min(1).max(50_000),
})
type BlockFormValues = z.infer<typeof blockSchema>

function BlockFormModal({
  open,
  assistantId,
  existing,
  onClose,
}: {
  open: boolean
  assistantId: string
  existing?: CoreMemoryBlock
  onClose: () => void
}) {
  const form = useForm<BlockFormValues>({
    resolver: zodResolver(blockSchema),
    defaultValues: { block_label: '', content: '', char_limit: 2000 },
  })
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    if (open) {
      form.reset(
        existing ?? { block_label: '', content: '', char_limit: 2000 },
      )
    }
  }, [open, existing])

  const handleSubmit = async (values: BlockFormValues) => {
    setSaving(true)
    try {
      await Stores.CoreMemoryBlocks.upsert({
        assistant_id: assistantId,
        block_label: values.block_label,
        content: values.content,
        char_limit: values.char_limit,
      })
      message.success(existing ? 'Block updated' : 'Block added')
      onClose()
    } catch (error) {
      message.error(error instanceof Error ? error.message : 'Save failed')
    } finally {
      setSaving(false)
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(v) => { if (!v) onClose() }}
      title={
        existing ? `Edit "${existing.block_label}"` : 'Add core memory block'
      }
      footer={
        <>
          <Button variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button
            loading={saving}
            onClick={() => form.handleSubmit(handleSubmit)()}
          >
            {existing ? 'Save' : 'Add'}
          </Button>
        </>
      }
    >
      <Form form={form} onSubmit={handleSubmit} layout="vertical">
        <FormField
          name="block_label"
          label="Label"
          required
        >
          <Input
            disabled={!!existing /* label is the natural key; can't rename */}
            placeholder="persona"
          />
        </FormField>
        <FormField
          name="content"
          label="Content"
          required
        >
          <Textarea
            rows={6}
            placeholder="Always-in-context content. The assistant will see this prepended to every system prompt."
          />
        </FormField>
        <FormField
          name="char_limit"
          label="Soft char limit"
          description="Advisory; the LLM may exceed when writing back. Used as a hint in the system prompt."
        >
          <InputNumber min={1} max={50_000} />
        </FormField>
      </Form>
    </Dialog>
  )
}
