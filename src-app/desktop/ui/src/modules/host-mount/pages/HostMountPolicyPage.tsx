/**
 * Host-mount policy admin page (DESKTOP-ONLY).
 *
 * Controls the deployment policy for host-folder mounting into the code
 * sandbox: master enable, the allowed host path prefixes (empty = any), and
 * whether read-write mounts are permitted. Gated by `host_mount::manage`.
 */

import { useEffect } from 'react'
import {
  Card,
  Form,
  FormField,
  MultiSelect,
  Switch,
  message,
  useForm,
} from '@/components/ui'

import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'
import { Stores } from '@/core/stores'

type FormValues = {
  enabled: boolean
  allow_readwrite: boolean
  allowed_prefixes: string[]
}

export function HostMountPolicyPage() {
  const { policy, loading, saving } = Stores.HostMountPolicy

  const form = useForm<FormValues>({
    defaultValues: {
      enabled: true,
      allow_readwrite: false,
      allowed_prefixes: [],
    },
  })

  // Re-seed from the loaded policy only when the form has no unsaved edits
  // (mirrors the WebSearchGlobalSection re-seed guard).
  useEffect(() => {
    if (policy && !form.formState.isDirty) {
      form.reset({
        enabled: policy.enabled,
        allow_readwrite: policy.allow_readwrite,
        allowed_prefixes: policy.allowed_prefixes ?? [],
      })
    }
  }, [policy, form])

  const onSubmit = async (v: FormValues) => {
    try {
      await Stores.HostMountPolicy.updatePolicy({
        enabled: v.enabled,
        allow_readwrite: v.allow_readwrite,
        allowed_prefixes: v.allowed_prefixes,
      })
      form.reset(v) // saved → allow the next store update to re-seed
      message.success('Saved host-mount policy')
    } catch {
      message.error('Failed to save host-mount policy')
    }
  }

  return (
    <SettingsPageContainer
      title="Host Mount Policy"
      subtitle="Control whether folders from this machine can be mounted into the code sandbox, and which paths are allowed."
    >
      <Card
        loading={loading && !policy}
        data-test-section="host-mount-policy"
        data-testid="desktop-hostmount-policy-card"
        footer={
          <SettingsFormActions
            onSave={form.handleSubmit(onSubmit)}
            onCancel={() => form.reset()}
            saving={saving}
            saveDisabled={!form.formState.isDirty}
            saveTestid="desktop-hostmount-policy-save-btn"
            cancelTestid="desktop-hostmount-policy-cancel-btn"
          />
        }
      >
        <Form
          data-testid="desktop-hostmount-policy-form"
          form={form}
          layout="horizontal"
          onSubmit={onSubmit}
        >
          <FormField
            name="enabled"
            label="Allow host-folder mounting"
            valuePropName="checked"
            description="When off, no host folders are mounted into the sandbox on any project or conversation."
          >
            <Switch
              aria-label="Allow host-folder mounting"
              data-testid="desktop-hostmount-policy-enabled-switch"
            />
          </FormField>

          <FormField
            name="allow_readwrite"
            label="Allow read-write mounts"
            valuePropName="checked"
            description="Off by default — mounts are read-only. Enabling this lets the sandbox modify the real files in mounted folders."
          >
            <Switch
              aria-label="Allow read-write mounts"
              data-testid="desktop-hostmount-policy-readwrite-switch"
            />
          </FormField>

          <FormField
            name="allowed_prefixes"
            label="Allowed path prefixes"
            description="A folder is only mountable if its path starts with one of these. Leave empty to allow any path (typical for a single-user desktop)."
          >
            <MultiSelect
              options={[]}
              allowCreate
              tokenSeparators={[',']}
              placeholder="/Users/me/data"
              searchPlaceholder="Type a path prefix"
              emptyText="No prefixes added"
              removeLabel={label => `Remove ${label}`}
              aria-label="Allowed path prefixes"
              data-testid="desktop-hostmount-policy-prefixes-select"
            />
          </FormField>
        </Form>
      </Card>
    </SettingsPageContainer>
  )
}
