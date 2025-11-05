import {
  Button,
  Card,
  Col,
  Divider,
  Flex,
  Form,
  Input,
  Row,
  Switch,
  Typography,
} from 'antd'
import { useEffect, useState } from 'react'
import type { ProxySettings } from '@/api-client/types'

const { Text } = Typography

export interface ProviderProxySettingsFormProps {
  initialSettings: ProxySettings | null
  onSave: (values: ProxySettings) => Promise<void> | void
  disabled?: boolean
}

export function ProviderProxySettingsForm({
  initialSettings,
  onSave,
  disabled = false,
}: ProviderProxySettingsFormProps) {
  const [form] = Form.useForm()
  const [loading, setLoading] = useState(false)

  // Update form when initial settings change
  useEffect(() => {
    if (initialSettings) {
      form.setFieldsValue(initialSettings)
    }
  }, [initialSettings, form])

  const handleSave = async () => {
    try {
      setLoading(true)
      const values = await form.validateFields()

      await onSave(values)
    } catch (error) {
      console.error('Failed to save proxy settings:', error)
    } finally {
      setLoading(false)
    }
  }

  const handleReset = () => {
    if (initialSettings) {
      form.setFieldsValue(initialSettings)
    }
  }

  return (
    <Card title={'Proxy Settings'}>
      <Form name="provider-proxy-form" form={form} layout="vertical" onFinish={handleSave}>
        <Flex className={'flex-col'}>
          <Flex className={'flex-col gap-3'}>
            {/* Enable Proxy Toggle */}
            <div>
              <div className={'flex justify-between items-center'}>
                <div style={{ flex: 1, marginRight: 16 }}>
                  <Text strong>Enable Proxy</Text>
                  <br />
                  <Text type="secondary">
                    Route all API requests through a proxy server
                  </Text>
                </div>
                <Form.Item
                  name="enabled"
                  valuePropName="checked"
                  style={{ margin: 0 }}
                >
                  <Switch disabled={disabled} aria-label="Enable or disable proxy settings" />
                </Form.Item>
              </div>
            </div>

            {/* Proxy URL */}
            <div>
              <Text strong>Proxy URL</Text>
              <br />
              <Text type="secondary">
                The URL of your proxy server (e.g., http://proxy.company.com:8080)
              </Text>
              <Form.Item
                name="url"
                style={{ marginTop: 8 }}
                dependencies={['enabled']}
                validateTrigger={['onChange', 'onBlur']}
                rules={[
                  () => ({
                    validator(_, value) {
                      if (value && value.trim() !== '') {
                        try {
                          const url = new URL(value)
                          const allowedProtocols = [
                            'http:',
                            'https:',
                            'socks5:',
                          ]
                          if (!allowedProtocols.includes(url.protocol)) {
                            return Promise.reject(
                              new Error(
                                'URL must start with http://, https://, or socks5://',
                              ),
                            )
                          }
                          return Promise.resolve()
                        } catch {
                          return Promise.reject(new Error('Invalid URL format'))
                        }
                      }
                      return Promise.resolve()
                    },
                  }),
                ]}
              >
                <Input
                  placeholder={'http://proxy.example.com:8080'}
                  disabled={disabled}
                />
              </Form.Item>
            </div>

            {/* Authentication */}
            <div>
              <Text strong>Authentication</Text>
              <br />
              <Text type="secondary">
                Optional username and password for proxy authentication
              </Text>
              <Row gutter={8} style={{ marginTop: 8 }}>
                <Col span={12}>
                  <Form.Item name="username">
                    <Input
                      placeholder={'Username (optional)'}
                      disabled={disabled}
                    />
                  </Form.Item>
                </Col>
                <Col span={12}>
                  <Form.Item name="password">
                    <Input.Password
                      placeholder={'Password (optional)'}
                      disabled={disabled}
                    />
                  </Form.Item>
                </Col>
              </Row>
            </div>

            {/* No Proxy */}
            <div>
              <Text strong>No Proxy Hosts</Text>
              <br />
              <Text type="secondary">
                Comma-separated list of hosts that should bypass the proxy
              </Text>
              <Form.Item name="no_proxy" style={{ marginTop: 8 }}>
                <Input
                  placeholder={'localhost,127.0.0.1,.example.com'}
                  disabled={disabled}
                />
              </Form.Item>
            </div>

            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
              }}
            >
              <div style={{ flex: 1, marginRight: 16 }}>
                <Text strong>Ignore SSL Certificate Errors</Text>
                <br />
                <Text type="secondary">
                  Allow connections even if SSL certificate validation fails
                  (not recommended for production)
                </Text>
              </div>
              <Form.Item
                name="ignore_ssl_certificates"
                valuePropName="checked"
                style={{ margin: 0 }}
              >
                <Switch disabled={disabled} aria-label="Ignore SSL certificate errors" />
              </Form.Item>
            </div>
          </Flex>
        </Flex>

        <Divider />

        <div className={'flex justify-end'}>
          <Flex className="gap-2">
            <Button onClick={handleReset} disabled={disabled}>
              Reset
            </Button>
            <Button
              type="primary"
              htmlType="submit"
              loading={loading}
              disabled={disabled}
            >
              Save
            </Button>
          </Flex>
        </div>
      </Form>
    </Card>
  )
}
