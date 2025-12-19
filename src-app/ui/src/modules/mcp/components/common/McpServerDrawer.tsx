import { Button, Form, Input, Select, Switch, App } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { useEffect } from 'react'
import { Stores } from '@/core/stores'
import { useMcpServerDrawerStore } from '@/modules/mcp/stores'
import type {
  CreateMcpServerRequest,
  UpdateMcpServerRequest,
} from '@/api-client/types'

const { TextArea } = Input

const TRANSPORT_TYPES = [
  {
    label: 'Standard I/O',
    value: 'stdio',
    description:
      'Start MCP server as a local process communicating via stdin/stdout',
  },
  {
    label: 'HTTP',
    value: 'http',
    description: 'Connect to MCP server via HTTP/HTTPS endpoint',
  },
  {
    label: 'Server-Sent Events',
    value: 'sse',
    description: 'Connect to MCP server via Server-Sent Events',
  },
]

export function McpServerDrawer() {
  const [form] = Form.useForm()
  const { message } = App.useApp()

  const { open, loading, mode, editingServer } = useMcpServerDrawerStore()

  // Populate form when editing server changes
  useEffect(() => {
    if (editingServer && open && (mode === 'edit' || mode === 'edit-system')) {
      const formValues = {
        name: editingServer.name,
        display_name: editingServer.display_name,
        description: editingServer.description,
        transport_type: editingServer.transport_type,
        url: editingServer.url,
        command: editingServer.command,
        args:
          editingServer.args && editingServer.args.length > 0
            ? JSON.stringify(editingServer.args, null, 2)
            : '',
        env: editingServer.environment_variables
          ? JSON.stringify(editingServer.environment_variables, null, 2)
          : '',
        enabled: editingServer.enabled,
      }
      form.setFieldsValue(formValues)
    } else if (open && (mode === 'create' || mode === 'create-system')) {
      form.resetFields()
      form.setFieldsValue({
        transport_type: 'stdio',
        enabled: true,
      })
    }
  }, [editingServer, open, mode, form])

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields()
      Stores.McpServerDrawer.setMcpServerDrawerLoading(true)

      // Parse arguments from JSON array string
      let args: string[] = []
      if (values.args && values.args.trim()) {
        try {
          const parsed = JSON.parse(values.args)
          if (!Array.isArray(parsed)) {
            message.error('Arguments must be a JSON array')
            Stores.McpServerDrawer.setMcpServerDrawerLoading(false)
            return
          }
          args = parsed
        } catch (_error) {
          message.error('Invalid JSON in arguments')
          Stores.McpServerDrawer.setMcpServerDrawerLoading(false)
          return
        }
      }

      // Parse environment variables from JSON string
      let environmentVariables = {}
      if (values.env && values.env.trim()) {
        try {
          environmentVariables = JSON.parse(values.env)
          if (
            typeof environmentVariables !== 'object' ||
            Array.isArray(environmentVariables)
          ) {
            message.error('Environment variables must be a JSON object')
            Stores.McpServerDrawer.setMcpServerDrawerLoading(false)
            return
          }
        } catch (_error) {
          message.error('Invalid JSON in environment variables')
          Stores.McpServerDrawer.setMcpServerDrawerLoading(false)
          return
        }
      }

      const serverData = {
        name: values.name,
        display_name: values.display_name,
        description: values.description,
        transport_type: values.transport_type,
        url: values.url,
        command: values.command,
        args: args,
        environment_variables: environmentVariables,
        enabled: values.enabled ?? true,
      }

      if (mode === 'create') {
        await Stores.McpServer.createMcpServer(
          serverData as CreateMcpServerRequest,
        )
        message.success('MCP server created successfully')
      } else if (mode === 'edit' && editingServer) {
        const updateData: UpdateMcpServerRequest = {
          display_name: values.display_name,
          description: values.description,
          url: values.url,
          command: values.command,
          args: args,
          environment_variables: environmentVariables,
          enabled: values.enabled ?? true,
        }
        await Stores.McpServer.updateMcpServer(editingServer.id, updateData)
        message.success('MCP server updated successfully')
      } else if (mode === 'create-system') {
        await Stores.SystemMcpServer.createSystemServer(
          serverData as CreateMcpServerRequest,
        )
        message.success('System MCP server created successfully')
      } else if (mode === 'edit-system' && editingServer) {
        const updateData: UpdateMcpServerRequest = {
          display_name: values.display_name,
          description: values.description,
          url: values.url,
          command: values.command,
          args: args,
          environment_variables: environmentVariables,
          enabled: values.enabled ?? true,
        }
        await Stores.SystemMcpServer.updateSystemServer(
          editingServer.id,
          updateData,
        )
        message.success('System MCP server updated successfully')
      }

      Stores.McpServerDrawer.closeMcpServerDrawer()
      form.resetFields()
    } catch (error) {
      console.error('Failed to save MCP server:', error)
      message.error('Failed to save MCP server')
    } finally {
      Stores.McpServerDrawer.setMcpServerDrawerLoading(false)
    }
  }

  const handleClose = () => {
    Stores.McpServerDrawer.closeMcpServerDrawer()
    form.resetFields()
  }

  const getTitle = () => {
    switch (mode) {
      case 'create':
        return 'Add MCP Server'
      case 'edit':
        return 'Edit MCP Server'
      case 'create-system':
        return 'Add System Server'
      case 'edit-system':
        return 'Edit System Server'
      default:
        return 'MCP Server'
    }
  }

  const getButtonText = () => {
    switch (mode) {
      case 'create':
        return 'Create Server'
      case 'edit':
        return 'Update Server'
      case 'create-system':
        return 'Create System Server'
      case 'edit-system':
        return 'Update System Server'
      default:
        return 'Save'
    }
  }

  const transportType = Form.useWatch('transport_type', form)

  return (
    <Drawer open={open} onClose={handleClose} title={getTitle()} size={600}>
      <div className="flex flex-col gap-4">
        <Form name="mcp-server-form" form={form} layout="vertical">
          {/* Name (only for create mode) */}
          {(mode === 'create' || mode === 'create-system') && (
            <Form.Item
              label="Name"
              name="name"
              rules={[
                { required: true, message: 'Please enter a name' },
                {
                  pattern: /^[a-z0-9-]+$/,
                  message:
                    'Name must contain only lowercase letters, numbers, and hyphens',
                },
              ]}
            >
              <Input placeholder="e.g., filesystem, fetch, custom-tool" />
            </Form.Item>
          )}

          {/* Display Name */}
          <Form.Item
            label="Display Name"
            name="display_name"
            rules={[{ required: true, message: 'Please enter a display name' }]}
          >
            <Input placeholder="e.g., Filesystem Access, Web Fetch" />
          </Form.Item>

          {/* Description */}
          <Form.Item label="Description" name="description">
            <TextArea
              placeholder="Brief description of what this server does"
              rows={2}
            />
          </Form.Item>

          {/* Transport Type */}
          <Form.Item
            label="Transport Type"
            name="transport_type"
            rules={[
              { required: true, message: 'Please select a transport type' },
            ]}
          >
            <Select
              disabled={mode === 'edit' || mode === 'edit-system'}
              options={TRANSPORT_TYPES.map(type => ({
                ...type,
                disabled:
                  (mode === 'edit' || mode === 'edit-system') && editingServer
                    ? editingServer.transport_type !== type.value
                    : false,
              }))}
            />
          </Form.Item>

          {/* Transport-specific fields */}
          {transportType === 'stdio' && (
            <>
              <Form.Item
                label="Command"
                name="command"
                rules={[{ required: true, message: 'Please enter a command' }]}
              >
                <Input placeholder="e.g., npx, uvx, node" />
              </Form.Item>

              <Form.Item
                label="Arguments"
                name="args"
                help="JSON array format, e.g., [&quot;-y&quot;, &quot;@modelcontextprotocol/server-filesystem&quot;]"
              >
                <TextArea
                  placeholder='["-y", "@modelcontextprotocol/server-filesystem"]'
                  rows={3}
                  className="font-mono text-xs"
                />
              </Form.Item>

              <Form.Item
                label="Environment Variables"
                name="env"
                help="JSON object format, e.g., {&quot;KEY&quot;: &quot;value&quot;}"
              >
                <TextArea
                  placeholder='{"KEY": "value"}'
                  rows={4}
                  className="font-mono text-xs"
                />
              </Form.Item>
            </>
          )}

          {(transportType === 'http' || transportType === 'sse') && (
            <Form.Item
              label="URL"
              name="url"
              rules={[
                { required: true, message: 'Please enter a URL' },
                { type: 'url', message: 'Please enter a valid URL' },
              ]}
            >
              <Input placeholder="https://example.com/mcp" />
            </Form.Item>
          )}

          {/* Enabled */}
          <Form.Item label="Enabled" name="enabled" valuePropName="checked">
            <Switch />
          </Form.Item>
        </Form>

        <div className="flex gap-2 justify-end">
          <Button onClick={handleClose}>Cancel</Button>
          <Button type="primary" loading={loading} onClick={handleSubmit}>
            {getButtonText()}
          </Button>
        </div>
      </div>
    </Drawer>
  )
}
