import {
  Button,
  Card,
  Flex,
  Form,
  FormField,
  Input,
  PasswordInput,
  Separator,
  Switch,
  Text,
  useForm,
  zodResolver,
} from '@/components/ui'
import { z } from 'zod'
import { useEffect, useState } from 'react'
import type { ProxySettings } from '@/api-client/types'

const proxySettingsSchema = z.object({
  enabled: z.boolean().optional(),
  url: z
    .string()
    .optional()
    .superRefine((value, ctx) => {
      if (value && value.trim() !== '') {
        try {
          const url = new URL(value)
          const allowedProtocols = ['http:', 'https:', 'socks5:']
          if (!allowedProtocols.includes(url.protocol)) {
            ctx.addIssue({
              code: z.ZodIssueCode.custom,
              message: 'URL must start with http://, https://, or socks5://',
            })
          }
        } catch {
          ctx.addIssue({
            code: z.ZodIssueCode.custom,
            message: 'Invalid URL format',
          })
        }
      }
    }),
  username: z.string().optional(),
  password: z.string().optional(),
  no_proxy: z.string().optional(),
  ignore_ssl_certificates: z.boolean().optional(),
})

type ProxySettingsFormValues = z.infer<typeof proxySettingsSchema>

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
  const form = useForm<ProxySettingsFormValues>({
    resolver: zodResolver(proxySettingsSchema),
    defaultValues: initialSettings ?? {},
  })
  const [loading, setLoading] = useState(false)

  // Update form when initial settings change
  useEffect(() => {
    if (initialSettings) {
      form.reset(initialSettings)
    }
  }, [initialSettings, form])

  const handleSave = async (values: ProxySettingsFormValues) => {
    try {
      setLoading(true)
      await onSave(values as ProxySettings)
    } catch (error) {
      console.error('Failed to save proxy settings:', error)
    } finally {
      setLoading(false)
    }
  }

  const handleReset = () => {
    if (initialSettings) {
      form.reset(initialSettings)
    }
  }

  return (
    <Card title={'Proxy Settings'}>
      <Form
        name="provider-proxy-form"
        form={form}
        layout="vertical"
        onSubmit={handleSave}
        disabled={disabled}
      >
        <Flex className={'flex-col'}>
          <Flex className={'flex-col gap-3'}>
            {/* Enable Proxy Toggle */}
            <div>
              <div className={'flex justify-between items-center'}>
                <div className="flex-1 mr-4">
                  <Text strong>Enable Proxy</Text>
                  <br />
                  <Text type="secondary">
                    Route all API requests through a proxy server
                  </Text>
                </div>
                <FormField
                  name="enabled"
                  aria-label="Enable proxy"
                  valuePropName="checked"
                >
                  <Switch
                    aria-label="Enable or disable proxy settings"
                  />
                </FormField>
              </div>
            </div>

            {/* Proxy URL */}
            <div>
              <Text strong>Proxy URL</Text>
              <br />
              <Text type="secondary">
                The URL of your proxy server (e.g.,
                http://proxy.company.com:8080)
              </Text>
              <div className="mt-2">
                <FormField
                  name="url"
                  aria-label="Proxy URL"
                >
                  <Input
                    placeholder={'http://proxy.example.com:8080'}
                  />
                </FormField>
              </div>
            </div>

            {/* Authentication */}
            <div>
              <Text strong>Authentication</Text>
              <br />
              <Text type="secondary">
                Optional username and password for proxy authentication
              </Text>
              <div className="mt-2 grid grid-cols-2 gap-2">
                <FormField name="username" aria-label="Proxy username">
                  <Input
                    placeholder={'Username (optional)'}
                  />
                </FormField>
                <FormField name="password" aria-label="Proxy password">
                  <PasswordInput
                    placeholder={'Password (optional)'}
                    showLabel="Show"
                    hideLabel="Hide"
                  />
                </FormField>
              </div>
            </div>

            {/* No Proxy */}
            <div>
              <Text strong>No Proxy Hosts</Text>
              <br />
              <Text type="secondary">
                Comma-separated list of hosts that should bypass the proxy
              </Text>
              <div className="mt-2">
                <FormField name="no_proxy" aria-label="No-proxy hosts">
                  <Input
                    placeholder={'localhost,127.0.0.1,.example.com'}
                  />
                </FormField>
              </div>
            </div>

            <div className="flex justify-between items-center">
              <div className="flex-1 mr-4">
                <Text strong>Ignore SSL Certificate Errors</Text>
                <br />
                <Text type="secondary">
                  Allow connections even if SSL certificate validation fails
                  (not recommended for production)
                </Text>
              </div>
              <FormField
                name="ignore_ssl_certificates"
                aria-label="Ignore SSL certificate errors"
                valuePropName="checked"
              >
                <Switch
                  aria-label="Ignore SSL certificate errors"
                />
              </FormField>
            </div>
          </Flex>
        </Flex>

        <Separator />

        <div className={'flex justify-end'}>
          <Flex className="gap-2">
            <Button variant="outline" onClick={handleReset}>
              Reset
            </Button>
            <Button
              type="submit"
              loading={loading}
            >
              Save
            </Button>
          </Flex>
        </div>
      </Form>
    </Card>
  )
}
