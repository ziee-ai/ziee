import {
  Alert,
  Card,
  Form,
  FormField,
  InputNumber,
  Select,
  Switch,
  useForm,
} from '@/components/ui'
import { Paragraph, message } from '@/components/ui'
import { useEffect, useMemo, useState } from 'react'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'

interface PolicyForm {
  http: boolean
  stdio: boolean
  flavor: string | null
  retention_days: number
}

/**
 * Admin card mounted on top of the System MCP Servers page. Governs which MCP
 * transports regular users may install (empty = user MCP disabled), the sandbox
 * flavor every user-installed stdio MCP runs inside, and the tool-call history
 * retention window. Hidden on single-admin desktop (multiUserMode=false).
 */
export function McpUserPolicyCard() {
  const { multiUserMode } = Stores.AppMode
  const { policy } = Stores.McpUserPolicy
  const allowedTransports = useMemo(
    () => policy?.allowed_transports ?? [],
    [policy],
  )
  const canEdit = usePermission(Permissions.McpUserPolicyEdit)
  const [saving, setSaving] = useState(false)
  const { flavors: rawFlavors, selectOptions: fallbackFlavorOptions } = Stores.SandboxFlavors
  // Rich options: the trigger shows just the flavor name (capitalized), the
  // dropdown shows name + description + size on two lines.
  const flavorOptions = rawFlavors.length
    ? rawFlavors.map((e) => ({
        value: e.flavor,
        selectedLabel: <span className="capitalize">{e.flavor}</span>,
        label: (
          <div className="flex flex-col py-0.5">
            <span className="font-medium capitalize">{e.flavor}</span>
            <span className="text-xs text-muted-foreground">
              {e.description}
            </span>
          </div>
        ),
      }))
    : fallbackFlavorOptions

  const form = useForm<PolicyForm>({
    defaultValues: { http: false, stdio: false, flavor: null, retention_days: 90 },
  })

  // Keep the form synced with the store when the policy updates from another
  // origin (another tab / backend event).
  useEffect(() => {
    form.reset({
      http: allowedTransports.includes('http'),
      stdio: allowedTransports.includes('stdio'),
      flavor: policy?.user_stdio_sandbox_flavor ?? null,
      retention_days: policy?.tool_call_retention_days ?? 90,
    })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [policy])

  const stdio = form.watch('stdio')
  const noTransports = !form.watch('http') && !stdio

  if (!multiUserMode) return null

  const handleSave = async (v: PolicyForm) => {
    if (saving) return
    if (v.stdio && !v.flavor) {
      message.error('Pick a sandbox flavor when stdio is allowed for users.')
      return
    }
    const transports = [v.http ? 'http' : null, v.stdio ? 'stdio' : null].filter(
      (t): t is string => t != null,
    )
    setSaving(true)
    try {
      await Stores.McpUserPolicy.update({
        allowed_transports: transports,
        user_stdio_sandbox_flavor: v.stdio ? v.flavor ?? undefined : undefined,
        tool_call_retention_days: v.retention_days,
      })
      message.success('MCP user policy updated')
    } catch (err: any) {
      message.error(`Failed to update policy: ${err?.message ?? String(err)}`)
    } finally {
      setSaving(false)
    }
  }

  return (
    <Card
      title="User MCP policy"
      data-testid="mcp-user-policy-card"
      footer={canEdit ? (
        <SettingsFormActions
          onSave={form.handleSubmit(handleSave)}
          onCancel={() => form.reset()}
          saving={saving}
          saveLabel="Save policy"
          saveTestid="mcp-policy-save-btn"
          cancelTestid="mcp-policy-cancel-btn"
        />
      ) : undefined}
    >
      <Paragraph type="secondary" className="!mb-4 text-sm">
        Govern which MCP transports regular users may install. Disable both to
        hide the Add button on /settings/mcp-servers and the MCP tab in the Hub
        for non-admins.
      </Paragraph>

      <Form
        form={form}
        layout="horizontal"
        onSubmit={handleSave}
        disabled={!canEdit}
        data-testid="mcp-policy-form"
      >
        <FormField
          name="http"
          label="Allow HTTP"
          description="Regular users may install HTTP MCP servers."
          valuePropName="checked"
        >
          <Switch tooltip="Allow users to install HTTP MCP servers" data-testid="mcp-policy-transport-http" />
        </FormField>

        <FormField
          name="stdio"
          label="Allow Standard I/O"
          description="Users may install stdio MCP servers; each runs inside the sandbox flavor below."
          valuePropName="checked"
        >
          <Switch tooltip="Allow users to install sandboxed stdio MCP servers" data-testid="mcp-policy-transport-stdio" />
        </FormField>

        {noTransports && (
          <Alert
            tone="warning"
            className="mb-3"
            data-testid="mcp-policy-no-transports-alert"
            title="Users cannot add any MCP server. The MCP tab in the Hub is hidden."
          />
        )}

        {stdio && (
          <FormField
            name="flavor"
            label="User stdio sandbox flavor"
            description="Every user-installed stdio MCP server runs inside this code_sandbox flavor. Users never pick a flavor — the server force-applies this on create."
          >
            <Select
              data-testid="mcp-policy-flavor-select"
              options={flavorOptions}
              placeholder="Pick a flavor"
            />
          </FormField>
        )}

        <FormField
          name="retention_days"
          label="Tool-call history retention"
          description="Days to keep the MCP tool-call history (shown in each server's “Calls” tab) before a background job prunes it. Set to 0 to keep it forever."
        >
          <InputNumber min={0} max={3650} suffix="days" className="w-40" data-testid="mcp-tool-call-retention-days" />
        </FormField>
      </Form>
    </Card>
  )
}
