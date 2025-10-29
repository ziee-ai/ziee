import {
  CloudDownloadOutlined,
  EyeInvisibleOutlined,
  EyeTwoTone,
} from '@ant-design/icons'
import { App, Button, Form, Input, Select, Switch, Typography } from 'antd'
import { useEffect, useState } from 'react'
import { Drawer } from '@/components/common/Drawer.tsx'
import { Stores } from '@/core/stores'
import {
  createLlmRepository,
  testLlmRepositoryConnection,
  updateLlmRepository,
} from '../store'
import type {
  LlmRepository,
  CreateLlmRepositoryRequest,
  UpdateLlmRepositoryRequest,
} from '@/api-client/types'

const { Text } = Typography

interface LlmRepositoryDrawerProps {
  repository: LlmRepository | null
  open: boolean
  onClose: () => void
  onSuccess?: () => void
}

export function LlmRepositoryDrawer({
  repository,
  open,
  onClose,
  onSuccess,
}: LlmRepositoryDrawerProps) {
  const { message } = App.useApp()
  const [form] = Form.useForm()
  const [loading, setLoading] = useState(false)

  const { creating, updating, testing } = Stores.LlmRepository

  // Update form when editing repository
  useEffect(() => {
    if (repository && open) {
      form.setFieldsValue({
        name: repository.name,
        url: repository.url,
        auth_type: repository.auth_type,
        api_key: repository.auth_config?.api_key,
        username: repository.auth_config?.username,
        password: repository.auth_config?.password,
        token: repository.auth_config?.token,
        auth_test_api_endpoint:
          repository.auth_config?.auth_test_api_endpoint,
        enabled: repository.enabled,
      })
    } else if (!repository && open) {
      form.setFieldsValue({
        auth_type: 'none',
        enabled: true,
      })
    }
  }, [repository, open, form])

  const testRepositoryFromForm = async () => {
    const values = form.getFieldsValue()

    // Validate required fields
    if (!values.name) {
      message.warning('Please enter a repository name first')
      return
    }
    if (!values.url) {
      message.warning('Please enter a repository URL first')
      return
    }

    // Validate auth fields based on type
    if (values.auth_type === 'api_key' && !values.api_key) {
      message.warning('Please enter an API key first')
      return
    }
    if (
      values.auth_type === 'basic_auth' &&
      (!values.username || !values.password)
    ) {
      message.warning('Please enter username and password first')
      return
    }
    if (values.auth_type === 'bearer_token' && !values.token) {
      message.warning('Please enter a bearer token first')
      return
    }

    try {
      const testData = {
        name: values.name,
        url: values.url,
        auth_type: values.auth_type,
        auth_config: {
          api_key: values.api_key,
          username: values.username,
          password: values.password,
          token: values.token,
          auth_test_api_endpoint: values.auth_test_api_endpoint,
        },
      }

      const result = await testLlmRepositoryConnection(testData)

      if (result.success) {
        message.success(
          result.message || `Connection to ${values.name} successful!`,
        )
      } else {
        message.error(result.message || `Connection to ${values.name} failed`)
      }
    } catch (error: any) {
      console.error('Repository connection test failed:', error)
      message.error(error?.message || `Connection to ${values.name} failed`)
    }
  }

  const handleClose = () => {
    form.resetFields()
    onClose()
  }

  const handleSubmit = async (values: any) => {
    setLoading(true)

    let repositoryData: UpdateLlmRepositoryRequest

    if (repository?.built_in) {
      // For built-in repositories, only allow authentication-related fields
      repositoryData = {
        auth_config: {
          api_key: values.api_key,
          username: values.username,
          password: values.password,
          token: values.token,
          auth_test_api_endpoint: values.auth_test_api_endpoint,
        },
      }
    } else {
      // For custom repositories, allow all fields
      repositoryData = {
        name: values.name,
        url: values.url,
        auth_type: values.auth_type,
        auth_config: {
          api_key: values.api_key,
          username: values.username,
          password: values.password,
          token: values.token,
          auth_test_api_endpoint: values.auth_test_api_endpoint,
        },
        enabled: values.enabled ?? true,
      }
    }

    try {
      if (repository) {
        // Update existing repository
        await updateLlmRepository(repository.id, repositoryData)
        message.success('Repository updated successfully')
      } else {
        // Add new repository - need full CreateLlmRepositoryRequest
        const createData: CreateLlmRepositoryRequest = {
          name: values.name,
          url: values.url,
          auth_type: values.auth_type,
          auth_config: {
            api_key: values.api_key,
            username: values.username,
            password: values.password,
            token: values.token,
            auth_test_api_endpoint: values.auth_test_api_endpoint,
          },
          enabled: values.enabled ?? true,
        }
        await createLlmRepository(createData)
        message.success('Repository added successfully')
      }

      handleClose()
      onSuccess?.()
    } catch (error: any) {
      console.error('Failed to save repository:', error)
      message.error(error?.message || 'Failed to save repository')
    } finally {
      setLoading(false)
    }
  }

  return (
    <Drawer
      title={
        repository
          ? repository.built_in
            ? 'Edit Built-in Repository (Authentication Only)'
            : 'Edit Repository'
          : 'Add Repository'
      }
      open={open}
      onClose={handleClose}
      footer={null}
      width={600}
      maskClosable={false}
    >
      <Form form={form} layout="vertical" onFinish={handleSubmit}>
        <Form.Item
          name="name"
          label="Repository Name"
          rules={[
            { required: true, message: 'Please enter a repository name' },
          ]}
        >
          <Input
            placeholder="My Custom Repository"
            disabled={repository?.built_in}
          />
        </Form.Item>

        <Form.Item
          name="url"
          label="Repository URL"
          rules={[
            { required: true, message: 'Please enter a repository URL' },
            { type: 'url', message: 'Please enter a valid URL' },
          ]}
        >
          <Input
            placeholder="https://your-custom-repo.com/models"
            disabled={repository?.built_in}
          />
        </Form.Item>

        <Form.Item
          name="auth_type"
          label="Authentication Type"
          rules={[{ required: true }]}
        >
          <Select disabled={repository?.built_in}>
            <Select.Option value="none">No Authentication</Select.Option>
            <Select.Option value="api_key">API Key</Select.Option>
            <Select.Option value="basic_auth">
              Basic Authentication
            </Select.Option>
            <Select.Option value="bearer_token">Bearer Token</Select.Option>
          </Select>
        </Form.Item>

        <Form.Item dependencies={['auth_type']} noStyle>
          {({ getFieldValue }) => {
            const authType = getFieldValue('auth_type')

            if (authType === 'api_key') {
              return (
                <Form.Item name="api_key" label="API Key">
                  <Input.Password
                    placeholder="Enter your API key"
                    iconRender={visible =>
                      visible ? <EyeTwoTone /> : <EyeInvisibleOutlined />
                    }
                  />
                </Form.Item>
              )
            }

            if (authType === 'basic_auth') {
              return (
                <>
                  <Form.Item name="username" label="Username">
                    <Input placeholder="Enter your username" />
                  </Form.Item>
                  <Form.Item name="password" label="Password">
                    <Input.Password
                      placeholder="Enter your password"
                      iconRender={visible =>
                        visible ? <EyeTwoTone /> : <EyeInvisibleOutlined />
                      }
                    />
                  </Form.Item>
                </>
              )
            }

            if (authType === 'bearer_token') {
              return (
                <Form.Item name="token" label="Bearer Token">
                  <Input.Password
                    placeholder="Enter your bearer token"
                    iconRender={visible =>
                      visible ? <EyeTwoTone /> : <EyeInvisibleOutlined />
                    }
                  />
                </Form.Item>
              )
            }

            return null
          }}
        </Form.Item>

        <Form.Item
          name="auth_test_api_endpoint"
          label="Authentication Test Endpoint"
          tooltip="Custom endpoint to test authentication. If not provided, the main repository URL will be used for testing."
        >
          <Input
            disabled={repository?.built_in}
            placeholder="https://api.example.com/auth/test"
          />
        </Form.Item>

        {/* Test Connection Section */}
        <Form.Item
          dependencies={[
            'url',
            'auth_type',
            'api_key',
            'username',
            'password',
            'token',
            'auth_test_api_endpoint',
          ]}
          noStyle
        >
          {({ getFieldValue }) => {
            const authType = getFieldValue('auth_type')
            const url = getFieldValue('url')

            // Only show test button if URL is provided and auth is configured (if needed)
            const showTestButton =
              url &&
              (authType === 'none' ||
                (authType === 'api_key' && getFieldValue('api_key')) ||
                (authType === 'basic_auth' &&
                  getFieldValue('username') &&
                  getFieldValue('password')) ||
                (authType === 'bearer_token' && getFieldValue('token')))

            if (showTestButton) {
              return (
                <Form.Item label="Connection Test">
                  <div>
                    <Text type="secondary" className="block mb-3">
                      Test your repository configuration to ensure it's
                      accessible
                    </Text>
                    <Button
                      type="default"
                      icon={<CloudDownloadOutlined />}
                      loading={testing}
                      onClick={testRepositoryFromForm}
                    >
                      Test Connection
                    </Button>
                  </div>
                </Form.Item>
              )
            }

            return null
          }}
        </Form.Item>

        <Form.Item
          name="enabled"
          label="Enable Repository"
          valuePropName="checked"
        >
          <Switch disabled={repository?.built_in} />
        </Form.Item>

        <div className="flex justify-end gap-3 pt-4">
          <Button onClick={handleClose} disabled={loading || creating || updating}>
            Cancel
          </Button>
          <Button
            type="primary"
            htmlType="submit"
            loading={loading || creating || updating}
          >
            {repository ? 'Update' : 'Add'} Repository
          </Button>
        </div>
      </Form>
    </Drawer>
  )
}
