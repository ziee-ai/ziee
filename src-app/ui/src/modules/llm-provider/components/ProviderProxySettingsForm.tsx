import {
  Card,
  Form,
  FormField,
  Input,
  PasswordInput,
  Switch,
  useForm,
  zodResolver,
} from '@ziee/kit'
import { z } from 'zod'
import { useEffect, useState } from 'react'
import type { ProxySettings } from '@/api-client/types'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'

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
    <Card
      title="Proxy Settings"
      data-testid="llm-proxy-settings-card"
      footer={
        <SettingsFormActions
          onSave={form.handleSubmit(handleSave)}
          onCancel={handleReset}
          saving={loading}
          cancelDisabled={disabled}
          saveDisabled={disabled}
          cancelLabel="Reset"
          saveTestid="llm-proxy-save-btn"
          cancelTestid="llm-proxy-reset-btn"
        />
      }
    >
      <Form
        name="provider-proxy-form"
        form={form}
        layout="horizontal"
        onSubmit={handleSave}
        disabled={disabled}
        data-testid="llm-proxy-form"
      >
        <FormField
          name="enabled"
          label="Enable proxy"
          description="Route all API requests through a proxy server."
          valuePropName="checked"
        >
          <Switch tooltip="Enable or disable proxy settings" data-testid="llm-proxy-enabled-switch" />
        </FormField>
        <FormField
          name="url"
          label="Proxy URL"
          description="The URL of your proxy server (e.g., http://proxy.company.com:8080)."
        >
          <Input placeholder="http://proxy.example.com:8080" data-testid="llm-proxy-url-input" />
        </FormField>
        <FormField
          name="username"
          label="Username"
          description="Optional credentials for proxy authentication."
        >
          <Input placeholder="Username (optional)" data-testid="llm-proxy-username-input" />
        </FormField>
        <FormField name="password" label="Password">
          <PasswordInput
            placeholder="Password (optional)"
            showLabel="Show"
            hideLabel="Hide"
            data-testid="llm-proxy-password-input"
          />
        </FormField>
        <FormField
          name="no_proxy"
          label="No-proxy hosts"
          description="Comma-separated list of hosts that should bypass the proxy."
        >
          <Input placeholder="localhost,127.0.0.1,.example.com" data-testid="llm-proxy-no-proxy-input" />
        </FormField>
        <FormField
          name="ignore_ssl_certificates"
          label="Ignore SSL certificate errors"
          description="Allow connections even if SSL certificate validation fails (not recommended for production)."
          valuePropName="checked"
        >
          <Switch tooltip="Ignore SSL certificate errors" data-testid="llm-proxy-ignore-ssl-switch" />
        </FormField>
      </Form>
    </Card>
  )
}
