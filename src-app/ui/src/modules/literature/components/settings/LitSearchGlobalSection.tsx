import { useEffect } from 'react'
import {
  Alert,
  Button,
  Card,
  Flex,
  Form,
  FormField,
  InputNumber,
  Paragraph,
  Separator,
  Spin,
  Switch,
  Text,
  message,
  useForm,
} from '@/components/ui'
import { Permissions, type UpdateLitSearchSettingsRequest } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'

interface CapsForm {
  max_results: number
  per_source_limit: number
  request_timeout_secs: number
}

/**
 * General card for the Literature Search settings page: master enable +
 * completeness toggle + result caps. Split out as its own section file to
 * mirror the web_search peer (WebSearchGlobalSection), keeping the page shell
 * thin.
 */
export function LitSearchGlobalSection() {
  const { settings, loading, savingSettings } = Stores.LitSearchAdmin
  const canManage = usePermission(Permissions.LitSearchAdminManage)
  const form = useForm<CapsForm>()

  useEffect(() => {
    if (settings && !form.formState.isDirty) {
      form.reset({
        max_results: settings.max_results,
        per_source_limit: settings.per_source_limit,
        request_timeout_secs: settings.request_timeout_secs,
      })
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings?.max_results, settings?.per_source_limit, settings?.request_timeout_secs])

  if (loading && !settings) {
    return (
      <Card title="General">
        <Spin label="Loading" />
      </Card>
    )
  }
  if (!settings) return null

  const save = async (patch: UpdateLitSearchSettingsRequest, label = 'Saved') => {
    try {
      await Stores.LitSearchAdmin.updateSettings(patch)
      message.success(label)
    } catch (e: any) {
      message.error(e?.message ?? 'Update failed')
    }
  }

  const handleCapsSubmit = async (v: CapsForm) => {
    await save(v, 'Literature search settings saved')
    // Reset to submitted values to clear RHF dirty state so a later
    // settings refetch (sync-driven reload) can re-seed the form.
    form.reset(v)
  }

  return (
    <Card title="General">
      {!canManage && (
        <Alert
          tone="info"
          title="Read-only view"
          description="You can view literature search settings but not change them."
          className="mb-3"
        />
      )}
      <Flex align="center" gap="small" className="mb-3">
        <Switch
          aria-label="Enable literature search"
          checked={settings.enabled}
          disabled={!canManage}
          onChange={v => save({ enabled: v }, v ? 'Literature search enabled' : 'Disabled')}
        />
        <Text>Enable literature search</Text>
      </Flex>

      <Flex align="center" gap="small" className="mb-3">
        <Switch
          aria-label="Show completeness estimate"
          checked={settings.completeness_estimate_enabled}
          disabled={!canManage}
          onChange={v => save({ completeness_estimate_enabled: v }, 'Completeness estimate updated')}
        />
        <Text>Show completeness (saturation) estimate</Text>
      </Flex>

      <Paragraph type="secondary" className="text-xs">
        The saturation estimate is a heuristic — never a measured recall rate. This
        feature is an adjunct to, not a replacement for, systematic searching.
      </Paragraph>

      <Separator titlePlacement="left">
        <Text className="text-sm">Caps</Text>
      </Separator>

      <Form
        form={form}
        name="lit-caps"
        layout="horizontal"
        labelWidth="42%"
        disabled={!canManage}
        onSubmit={handleCapsSubmit}
      >
        <FormField label="Max deduped results" name="max_results">
          <InputNumber min={1} max={200} className="w-full" />
        </FormField>
        <FormField label="Per-source limit" name="per_source_limit">
          <InputNumber min={1} max={100} className="w-full" />
        </FormField>
        <FormField label="Request timeout (s)" name="request_timeout_secs">
          <InputNumber min={1} max={120} className="w-full" />
        </FormField>
        <Flex justify="end">
          <Button
            type="submit"
            loading={savingSettings}
            disabled={!canManage || !form.formState.isDirty}
          >
            Save caps
          </Button>
        </Flex>
      </Form>
    </Card>
  )
}
