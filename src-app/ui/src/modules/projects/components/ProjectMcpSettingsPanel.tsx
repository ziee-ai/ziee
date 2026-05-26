import { useEffect, useState } from 'react'
import { App, Button, Flex, Form, Input, Radio, Typography } from 'antd'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import {
  Permissions,
  type Project,
  type UpdateProjectMcpSettingsRequest,
} from '@/api-client/types'

const { Text } = Typography

interface ProjectMcpSettingsPanelProps {
  project: Project
}

type ApprovalMode = 'disabled' | 'auto_approve' | 'manual_approve'

/**
 * Editor for a project's MCP defaults. The settings are SNAPSHOTTED onto
 * every conversation created in this project — changes here do NOT
 * propagate to existing conversations (matches Plan 5 §4 precedence).
 *
 * For v1 the auto_approved_tools and disabled_servers JSONB columns are
 * edited as raw JSON. A richer per-server tool-toggle UI lives on the
 * conversation MCP panel today and can be ported as a v1.1 polish if
 * users need it on the project level.
 *
 * Permission gating (audit Q3): Form is `disabled={!canEdit}` so users
 * without `ProjectsEdit` see the panel in read-only mode; Save is
 * GATED (hidden, not just disabled); footer follows the canonical
 * Cancel-before-Submit + Flex layout.
 */
export function ProjectMcpSettingsPanel({
  project,
}: ProjectMcpSettingsPanelProps) {
  const { message } = App.useApp()
  const canEdit = usePermission(Permissions.ProjectsEdit)
  const [form] = Form.useForm<{
    approval_mode: ApprovalMode
    auto_approved_tools_json: string
    disabled_servers_json: string
  }>()
  const [saving, setSaving] = useState(false)

  const populateFromProject = () => {
    form.setFieldsValue({
      approval_mode: (project.mcp_approval_mode as ApprovalMode) ?? 'manual_approve',
      auto_approved_tools_json: JSON.stringify(
        project.mcp_auto_approved_tools ?? [],
        null,
        2,
      ),
      disabled_servers_json: JSON.stringify(
        project.mcp_disabled_servers ?? [],
        null,
        2,
      ),
    })
  }

  useEffect(() => {
    populateFromProject()
    // `form` is intentionally omitted from the dep array: Form.useForm()
    // returns a STABLE instance for the life of the component (antd
    // contract), so adding it would never change behavior but would
    // trick a reader into thinking it might. `populateFromProject`
    // closes over `form` + `project` only — both covered.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [project])

  // Hard cap so an accidental 100MB paste doesn't freeze the tab on
  // JSON.parse. Backend per-list cap is 256 entries × 256 tools ×
  // 256-byte names ≈ 16.8 MB worst case, but the strict shape makes
  // anything beyond ~64 KB pure noise.
  const MAX_JSON_BYTES = 65_536

  const handleSave = async () => {
    let values
    try {
      values = await form.validateFields()
    } catch {
      return
    }

    if (
      values.auto_approved_tools_json.length > MAX_JSON_BYTES ||
      values.disabled_servers_json.length > MAX_JSON_BYTES
    ) {
      message.error(
        `JSON fields are limited to ${MAX_JSON_BYTES.toLocaleString()} characters`,
      )
      return
    }

    let autoApproved: unknown
    let disabledServers: unknown
    try {
      autoApproved = JSON.parse(values.auto_approved_tools_json)
      disabledServers = JSON.parse(values.disabled_servers_json)
    } catch {
      message.error('Invalid JSON in one of the array fields')
      return
    }

    if (!Array.isArray(autoApproved) || !Array.isArray(disabledServers)) {
      message.error('Auto-approved tools and disabled servers must be JSON arrays')
      return
    }

    const req: UpdateProjectMcpSettingsRequest = {
      approval_mode: values.approval_mode,
      auto_approved_tools: autoApproved,
      disabled_servers: disabledServers,
    }

    try {
      setSaving(true)
      await Stores.ProjectDetail.updateMcpSettings(project.id, req)
      message.success('MCP settings saved')
    } catch (err) {
      message.error(
        err instanceof Error ? err.message : 'Failed to save MCP settings',
      )
    } finally {
      setSaving(false)
    }
  }

  const handleReset = () => {
    populateFromProject()
  }

  return (
    <div>
      <Text type="secondary" className="block mb-4">
        Default MCP approval mode and per-server settings for every NEW
        conversation in this project. Existing conversations keep their own
        settings — changes here do not retroactively apply.
      </Text>

      <Form
        form={form}
        layout="vertical"
        disabled={!canEdit}
        initialValues={{
          approval_mode: 'manual_approve',
          auto_approved_tools_json: '[]',
          disabled_servers_json: '[]',
        }}
      >
        <Form.Item
          name="approval_mode"
          label="Approval mode"
          tooltip="manual_approve = ask before each tool call; auto_approve = run all; disabled = block tool calls."
          rules={[{ required: true }]}
        >
          <Radio.Group>
            <Radio value="manual_approve">Manual approve</Radio>
            <Radio value="auto_approve">Auto approve</Radio>
            <Radio value="disabled">Disabled</Radio>
          </Radio.Group>
        </Form.Item>

        <Form.Item
          name="auto_approved_tools_json"
          label="Auto-approved tools (JSON)"
          tooltip='Array of {"server_id": "uuid", "tools": ["tool1"]} entries. Max 64 KiB.'
        >
          <Input.TextArea
            rows={6}
            spellCheck={false}
            placeholder="[]"
            className="font-mono"
            maxLength={MAX_JSON_BYTES}
          />
        </Form.Item>

        <Form.Item
          name="disabled_servers_json"
          label="Disabled servers (JSON)"
          tooltip='Array of {"server_id": "uuid", "tools": []} entries. Empty `tools` = whole server disabled; non-empty = those tools disabled. Max 64 KiB.'
        >
          <Input.TextArea
            rows={6}
            spellCheck={false}
            placeholder="[]"
            className="font-mono"
            maxLength={MAX_JSON_BYTES}
          />
        </Form.Item>

        <Flex className="justify-end gap-2 pt-2">
          <Button onClick={handleReset} disabled={saving || !canEdit}>
            {canEdit ? 'Reset' : 'Close'}
          </Button>
          {canEdit && (
            <Button type="primary" onClick={handleSave} loading={saving}>
              Save
            </Button>
          )}
        </Flex>
      </Form>
    </div>
  )
}
