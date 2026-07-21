import { useEffect, useState } from 'react'
import {
  Alert,
  Button,
  Card,
  Form,
  FormField,
  InputNumber,
  List,
  Paragraph,
  Select,
  Separator,
  Space,
  Spin,
  Switch,
  Text,
  Tooltip,
  message,
  useForm,
} from '@ziee/kit'
import { ArrowDown, ArrowUp, Trash2 } from 'lucide-react'
import { usePermission } from '@/core/permissions'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'
import { Permissions } from '@/api-client/permissions'
import { WebSearchAdmin } from '@/modules/web-search/stores/webSearchAdmin'

const MIB = 1024 * 1024

type FormValues = {
  enabled: boolean
  max_results: number
  fetch_max_mib: number
  fetch_max_chars: number
  request_timeout_secs: number
}

/**
 * Global web-search settings: the master enable switch, the ordered
 * provider fallback chain, and the request caps. The chain editor saves on
 * each reorder/add/remove; the caps form saves on its own Save button.
 */
export function WebSearchGlobalSection() {
  const { settings, providers, loading, savingSettings } = WebSearchAdmin
  const canManage = usePermission(Permissions.WebSearchAdminManage)

  const form = useForm<FormValues>()
  // Local in-flight flag for chain edits, so they don't share the store's
  // `savingSettings` flag with the caps Save button (which would cross-trigger
  // the caps spinner on a chain edit and vice-versa).
  const [savingChain, setSavingChain] = useState(false)

  // Re-seed from the store ONLY when the form has no unsaved edits. The chain
  // editor saves imperatively (move/add/remove → updateSettings), which
  // replaces `settings`; without the `!isDirty` guard that re-seed would clobber
  // in-progress caps/enabled edits the admin hasn't saved yet.
  useEffect(() => {
    if (settings && !form.formState.isDirty) {
      form.reset({
        enabled: settings.enabled,
        max_results: settings.max_results,
        fetch_max_mib: Math.round(settings.fetch_max_bytes / MIB),
        fetch_max_chars: settings.fetch_max_chars,
        request_timeout_secs: settings.request_timeout_secs,
      })
    }
  }, [settings, form])

  const onSubmit = async (v: FormValues) => {
    try {
      await WebSearchAdmin.updateSettings({
        enabled: v.enabled,
        max_results: v.max_results,
        fetch_max_bytes: v.fetch_max_mib * MIB,
        fetch_max_chars: v.fetch_max_chars,
        request_timeout_secs: v.request_timeout_secs,
      })
      form.reset(v) // saved → allow the next store update to re-seed
      message.success('Web search settings saved')
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to save')
    }
  }

  const chain = settings?.provider_chain ?? []
  const nameOf = (key: string) =>
    providers.find(p => p.key === key)?.display_name ?? key
  const configuredOf = (key: string) =>
    providers.find(p => p.key === key)?.configured ?? false
  const notInChain = providers.filter(p => !chain.includes(p.key))

  const saveChain = async (next: string[]) => {
    setSavingChain(true)
    try {
      await WebSearchAdmin.updateSettings({ provider_chain: next })
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to update provider chain')
    } finally {
      setSavingChain(false)
    }
  }

  const move = (i: number, dir: -1 | 1) => {
    const next = [...chain]
    const j = i + dir
    if (j < 0 || j >= next.length) return
    ;[next[i], next[j]] = [next[j], next[i]]
    void saveChain(next)
  }
  const remove = (i: number) => void saveChain(chain.filter((_, idx) => idx !== i))
  const add = (key: string) => void saveChain([...chain, key])

  if (loading && !settings) {
    return (
      <Card data-testid="websearch-global-card" title="Web search">
        <Spin label="Loading" />
      </Card>
    )
  }

  return (
    <>
    <Card
      data-testid="websearch-global-card"
      title="Web search"
      footer={
        <SettingsFormActions
          onSave={form.handleSubmit(onSubmit)}
          onCancel={() => form.reset()}
          saving={savingSettings}
          saveDisabled={!canManage || !form.formState.isDirty}
          cancelDisabled={!canManage}
          saveTestid="websearch-global-save"
          cancelTestid="websearch-global-cancel"
        />
      }
    >
      {!canManage && (
        <Alert
          data-testid="websearch-global-readonly-alert"
          tone="info"
          title="Read-only view"
          description="You can view web search settings but not change them."
          className="mb-3"
        />
      )}

      <Form
        data-testid="websearch-global-form"
        form={form}
        layout="horizontal"
        disabled={!canManage}
        onSubmit={onSubmit}
      >
        <FormField
          name="enabled"
          label="Enable web search"
          valuePropName="checked"
          description="Master switch. Even when on, web tools only attach to a chat once a provider in the chain is configured."
        >
          <Switch data-testid="websearch-global-enabled" />
        </FormField>

        <Separator titlePlacement="left">
          <Text className="text-xs" type="secondary">
            Caps
          </Text>
        </Separator>
        <FormField name="max_results" label="Max results per search">
          <InputNumber data-testid="websearch-global-max-results" min={1} max={20} className="w-full" />
        </FormField>
        <FormField
          name="fetch_max_mib"
          label="Page fetch size cap"
          description="Maximum bytes downloaded per fetch_url call."
        >
          <InputNumber data-testid="websearch-global-fetch-mib" min={1} max={100} suffix="MiB" className="w-full" />
        </FormField>
        <FormField
          name="fetch_max_chars"
          label="Page fetch char cap"
          description="Extracted markdown is truncated to this many characters."
        >
          <InputNumber data-testid="websearch-global-fetch-chars" min={1000} max={500000} step={1000} className="w-full" />
        </FormField>
        <FormField name="request_timeout_secs" label="Request timeout">
          <InputNumber data-testid="websearch-global-timeout" min={1} max={120} suffix="s" className="w-full" />
        </FormField>

      </Form>
    </Card>

    <Card data-testid="websearch-chain-card" title="Provider chain">
      <Paragraph type="secondary" className="text-sm !mb-4">
        Engines are tried top-to-bottom. The chain advances to the next engine
        only on failure (error / timeout / quota) — an engine returning no
        results is treated as a valid answer.
      </Paragraph>

      {chain.length === 0 ? (
        <Alert
          data-testid="websearch-global-empty-chain-alert"
          tone="warning"
          title="No providers in the chain"
          description="Add at least one provider below and configure it for web search to work."
          className="mb-3"
        />
      ) : (
        <List
          data-testid="websearch-global-chain-list"
          rowKey={key => key}
          size="sm"
          className="border rounded-md"
          dataSource={chain}
          renderItem={(key, i) => (
            <div className="flex items-center justify-between gap-2">
              <Space>
                <Text>{`${i + 1}. ${nameOf(key)}`}</Text>
                {!configuredOf(key) && (
                  <Text type="warning" className="text-xs">
                    (not configured)
                  </Text>
                )}
              </Space>
              {canManage && (
                <Space>
                  <Tooltip content="Move up">
                    <Button
                      data-testid={`websearch-chain-${key}-up`}
                      variant="ghost"
                      size="default"
                      aria-label={`Move ${nameOf(key)} up`}
                      icon={<ArrowUp />}
                      disabled={i === 0 || savingChain}
                      onClick={() => move(i, -1)}
                    />
                  </Tooltip>
                  <Tooltip content="Move down">
                    <Button
                      data-testid={`websearch-chain-${key}-down`}
                      variant="ghost"
                      size="default"
                      aria-label={`Move ${nameOf(key)} down`}
                      icon={<ArrowDown />}
                      disabled={i === chain.length - 1 || savingChain}
                      onClick={() => move(i, 1)}
                    />
                  </Tooltip>
                  <Tooltip content="Remove from chain">
                    <Button
                      data-testid={`websearch-chain-${key}-remove`}
                      variant="outline"
                      size="default"
                      aria-label={`Remove ${nameOf(key)} from chain`}
                      icon={<Trash2 />}
                      disabled={savingChain}
                      onClick={() => remove(i)}
                    />
                  </Tooltip>
                </Space>
              )}
            </div>
          )}
        />
      )}

      {canManage && notInChain.length > 0 && (
        <Select
          data-testid="websearch-global-add-provider"
          className="mt-3 w-full"
          placeholder="Add a provider to the chain…"
          value={undefined}
          disabled={savingChain}
          onChange={(key: string) => add(key)}
          options={notInChain.map(p => ({ value: p.key, label: p.display_name }))}
        />
      )}
    </Card>
    </>
  )
}
