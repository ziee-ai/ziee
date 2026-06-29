import {
  Alert,
  Button,
  Card,
  Checkbox,
  InputNumber,
  Select,
} from '@/components/ui'
import { Title, Paragraph, Text, message } from '@/components/ui'
import { useEffect, useMemo, useState } from 'react'
import { Stores } from '@/core/stores'
import { Can, usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

/**
 * Admin card mounted on top of the System MCP Servers page.
 * Governs two things:
 *
 *   1. `allowed_transports` — which transports regular users may
 *      install via /settings/mcp-servers + the Hub MCP tab.
 *      Empty array = user MCP fully disabled (Add button + Hub tab
 *      hidden for non-admins).
 *
 *   2. `user_stdio_sandbox_flavor` — the sandbox rootfs flavor that
 *      every user-installed stdio MCP runs inside. Required when
 *      `stdio` is in `allowed_transports`. Users never pick a
 *      flavor; the server force-applies this on create/update.
 *
 * Hidden on single-admin desktop (multiUserMode=false) — there's no
 * meaningful policy distinction when the single user IS the admin.
 *
 * Read-only view: a user with `McpServersAdminRead` but NOT
 * `McpUserPolicyEdit` reaches this page but cannot save. The form
 * controls are visually disabled in that case so they don't think
 * their changes will persist (the gate on the Save button alone
 * was confusing).
 */
export function McpUserPolicyCard() {
  const { multiUserMode } = Stores.AppMode

  // Read the policy state property (not function accessors) so this
  // card re-renders when another tab saves the policy. Function
  // properties on the Stores proxy don't subscribe — see
  // core/stores.ts:250-280.
  const { policy } = Stores.McpUserPolicy
  const allowedTransports = useMemo(
    () => policy?.allowed_transports ?? [],
    [policy],
  )
  const userStdioSandboxFlavor = policy?.user_stdio_sandbox_flavor ?? null
  const canEdit = usePermission(Permissions.McpUserPolicyEdit)

  const [transports, setTransports] = useState<string[]>(allowedTransports)
  const [flavor, setFlavor] = useState<string | null>(userStdioSandboxFlavor)
  const [retentionDays, setRetentionDays] = useState<number>(
    policy?.tool_call_retention_days ?? 90,
  )
  const [saving, setSaving] = useState(false)
  // Shared catalog — lazy-loaded by the store on first access (see
  // SandboxFlavors.store.ts), reused by McpServerDrawer too.
  const { selectOptions: flavorOptions } = Stores.SandboxFlavors

  // Keep local form state synced with the store when the policy
  // updates from another origin (another tab; backend event). Dep
  // is the `policy` reference — Zustand returns the same object
  // until `setState` rewrites it, so this effect only re-fires when
  // the policy actually changes. (allowedTransports + flavor are
  // derived from `policy` so they'd be redundant in the deps.)
  useEffect(() => {
    setTransports(allowedTransports)
    setFlavor(userStdioSandboxFlavor)
    setRetentionDays(policy?.tool_call_retention_days ?? 90)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [policy])

  if (!multiUserMode) return null

  const stdioAllowed = transports.includes('stdio')
  const noTransports = transports.length === 0

  const handleSave = async () => {
    if (saving) return
    if (stdioAllowed && !flavor) {
      message.error('Pick a sandbox flavor when stdio is allowed for users.')
      return
    }
    setSaving(true)
    try {
      await Stores.McpUserPolicy.update({
        allowed_transports: transports,
        user_stdio_sandbox_flavor: stdioAllowed
          ? flavor ?? undefined
          : undefined,
        tool_call_retention_days: retentionDays,
      })
      message.success('MCP user policy updated')
    } catch (err: any) {
      const msg = err?.message ?? String(err)
      message.error(`Failed to update policy: ${msg}`)
    } finally {
      setSaving(false)
    }
  }

  return (
    <Card data-testid="mcp-user-policy-card">
      <div className="flex flex-col gap-3">
        <div>
          <Title level={5} className="!m-0">
            User MCP policy
          </Title>
          <Paragraph type="secondary" className="!mb-0 !mt-1">
            Govern which MCP transports regular users may install. Disable
            both to hide the Add button on /settings/mcp-servers and the
            MCP tab in the Hub for non-admins.
          </Paragraph>
        </div>

        <div>
          <Text strong>Allowed transports for users</Text>
          <div className="mt-1 flex flex-col gap-2">
            {[
              { label: 'HTTP', value: 'http' },
              { label: 'Standard I/O (sandboxed)', value: 'stdio' },
            ].map(opt => (
              <label key={opt.value} className="flex items-center gap-2">
                <Checkbox
                  data-testid={`mcp-policy-transport-${opt.value}`}
                  checked={transports.includes(opt.value)}
                  onCheckedChange={checked => {
                    if (checked) {
                      setTransports([...transports, opt.value])
                    } else {
                      setTransports(transports.filter(t => t !== opt.value))
                    }
                  }}
                  disabled={!canEdit}
                />
                <span>{opt.label}</span>
              </label>
            ))}
          </div>
          {noTransports && (
            <Alert
              tone="warning"
              className="mt-2"
              data-testid="mcp-policy-no-transports-alert"
              title="Users cannot add any MCP server. The MCP tab in the Hub is hidden."
            />
          )}
        </div>

        {stdioAllowed && (
          <div>
            <Text strong>User stdio sandbox flavor</Text>
            <Paragraph type="secondary" className="!mb-1 !mt-1 !text-xs">
              Every user-installed stdio MCP server runs inside this
              code_sandbox flavor. Users never pick a flavor — the server
              force-applies this on create.
            </Paragraph>
            <Select
              className="w-full"
              data-testid="mcp-policy-flavor-select"
              value={flavor ?? undefined}
              onChange={v => setFlavor(v)}
              options={flavorOptions}
              disabled={!canEdit}
              placeholder="Pick a flavor"
            />
          </div>
        )}

        <div>
          <Text strong>Tool-call history retention</Text>
          <Paragraph type="secondary" className="!mb-1 !mt-1 !text-xs">
            Days to keep the MCP tool-call history (shown in each server's
            “Calls” tab) before a background job prunes it. Set to 0 to keep
            it forever.
          </Paragraph>
          <div className="flex items-center gap-2">
            <InputNumber
              min={0}
              max={3650}
              value={retentionDays}
              onChange={v => setRetentionDays(typeof v === 'number' ? v : 90)}
              disabled={!canEdit}
              data-testid="mcp-tool-call-retention-days"
            />
            <Text type="secondary">days</Text>
          </div>
        </div>

        <div className="flex justify-end">
          <Can permission={Permissions.McpUserPolicyEdit}>
            <Button loading={saving} onClick={handleSave} data-testid="mcp-policy-save-btn">
              Save policy
            </Button>
          </Can>
        </div>
      </div>
    </Card>
  )
}
