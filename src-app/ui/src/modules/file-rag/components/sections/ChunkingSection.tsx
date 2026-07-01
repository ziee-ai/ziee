import { useEffect } from 'react'
import {
  Alert,
  Card,
  Form,
  FormField,
  useForm,
  zodResolver,
  InputNumber,
  Paragraph,
  message,
} from '@/components/ui'
import { z } from 'zod'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'
import { Permissions } from '@/api-client/types'
import { SettingsSectionStatus } from '@/components/common/SettingsSectionStatus'

const READ_PERM = Permissions.FileRagAdminRead
const MANAGE_PERM = Permissions.FileRagAdminManage

const schema = z.object({
  chunk_chars: z.number().min(200).max(8000),
  chunk_overlap_chars: z.number().min(0).max(4000),
  max_chunks_per_file: z.number().min(1).max(100000),
})

type FormValues = z.infer<typeof schema>

/**
 * Chunking parameters. Changes apply to files indexed AFTER saving. Existing
 * files keep their current chunking until they're re-uploaded or edited (a new
 * version re-indexes them) — backfill does NOT re-split files that already have
 * chunks.
 */
export function ChunkingSection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const { settings, saving, error } = Stores.FileRagAdmin
  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      chunk_chars: 1200,
      chunk_overlap_chars: 200,
      max_chunks_per_file: 10000,
    },
  })

  useEffect(() => {
    // Don't clobber the admin's unsaved edits on a mid-edit refetch.
    if (settings && !form.formState.isDirty) {
      form.reset({
        chunk_chars: settings.chunk_chars,
        chunk_overlap_chars: settings.chunk_overlap_chars,
        max_chunks_per_file: settings.max_chunks_per_file,
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card data-testid="filerag-chunking-card" title="Chunking">
        <Alert
          data-testid="filerag-chunking-noperm-alert"
          tone="warning"
          title="You don't have permission to view Document RAG admin settings."
        />
      </Card>
    )
  }
  if (!settings)
    return (
      <SettingsSectionStatus
        title="Chunking"
        error={error}
        onRetry={() => Stores.FileRagAdmin.load()}
      />
    )

  const handleSubmit = async (values: FormValues) => {
    if (values.chunk_overlap_chars >= values.chunk_chars) {
      message.error('Overlap must be smaller than the chunk size.')
      form.setError('chunk_chars', {
        type: 'manual',
        message: 'Must be larger than the overlap',
      })
      form.setError('chunk_overlap_chars', {
        type: 'manual',
        message: 'Must be smaller than the chunk size',
      })
      return
    }
    try {
      await Stores.FileRagAdmin.update({
        chunk_chars: values.chunk_chars,
        chunk_overlap_chars: values.chunk_overlap_chars,
        max_chunks_per_file: values.max_chunks_per_file,
      })
      message.success('Chunking settings saved.')
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Failed to save settings.',
      )
    }
  }

  return (
    <Card
      data-testid="filerag-chunking-card"
      title="Chunking"
      footer={canManage ? (
        <SettingsFormActions
          onSave={form.handleSubmit(handleSubmit)}
          onCancel={() => form.reset()}
          saving={saving}
          saveTestid="filerag-chunking-save"
          cancelTestid="filerag-chunking-cancel"
        />
      ) : undefined}
    >
      {error && (
        <Alert data-testid="filerag-chunking-error-alert" tone="error" className="!mb-4" title={error} />
      )}
      <Paragraph type="secondary" className="!mb-3 text-sm">
        Applies to files indexed after saving; existing files keep their current
        chunking until re-uploaded or edited.
      </Paragraph>
      <Form
        data-testid="filerag-chunking-form"
        name="file-rag-admin-chunking-form"
        form={form}
        layout="horizontal"
        labelWidth="10rem"
        onSubmit={handleSubmit}
        disabled={!canManage}
      >
        <FormField
          name="chunk_chars"
          label="Chunk size (characters)"
          description="Target window size per chunk. ~1200 chars ≈ 300 tokens — small enough for precise citations, large enough for coherent passages."
        >
          <InputNumber data-testid="filerag-chunking-chunk-chars" min={200} max={8000} step={100} className="w-40" />
        </FormField>

        <FormField
          name="chunk_overlap_chars"
          label="Chunk overlap (characters)"
          description="How much consecutive chunks overlap, so a passage split across a boundary is still retrievable. Must be smaller than the chunk size."
        >
          <InputNumber data-testid="filerag-chunking-overlap" min={0} max={4000} step={50} className="w-40" />
        </FormField>

        <FormField
          name="max_chunks_per_file"
          label="Max chunks per file"
          description="Safety cap; a file producing more chunks than this is truncated (with a server log) to bound storage and embedding cost."
        >
          <InputNumber data-testid="filerag-chunking-max-chunks" min={1} max={100000} step={100} className="w-40" />
        </FormField>

      </Form>
    </Card>
  )
}
