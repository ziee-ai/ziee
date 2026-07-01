import { useEffect, useState } from 'react'
import {
  Alert,
  Card,
  Separator,
  Form,
  FormField,
  Input,
  InputNumber,
  Spin,
  Text,
  Paragraph,
  useForm,
  zodResolver,
  message,
} from '@/components/ui'
import { z } from 'zod'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'
import {
  Permissions,
  type CodeSandboxResourceLimits,
  type UpdateCodeSandboxResourceLimits,
} from '@/api-client/types'

const MANAGE_PERM = Permissions.CodeSandboxResourceLimitsManage
const READ_PERM = Permissions.CodeSandboxResourceLimitsRead

const MIB = 1024 * 1024
const GIB = 1024 * 1024 * 1024

/** Form values mirror the API row but make the byte fields editable in MiB. */
type FormValues = {
  memory_max_mib: number
  memory_swap_max_mib: number
  pids_max: number
  cpu_max: string
  address_space_mib: number
  fsize_mib: number
  nproc_max: number
  nofile_max: number
  cpu_secs_max: number
  timeout_secs: number
  vm_idle_evict_secs: number
  mac_vm_vcpus: number
  mac_vm_ram_mib: number
  vm_max_concurrent_execs: number
}

const schema = z.object({
  memory_max_mib: z.number().refine(v => v >= 16, 'must be ≥ 16 MiB'),
  memory_swap_max_mib: z.number(),
  pids_max: z
    .number()
    .refine(v => v >= 8 && v <= 100_000, 'must be 8..=100000'),
  cpu_max: z
    .string()
    .regex(/^[0-9]+ [0-9]+$/, 'shape: "<quota> <period>" (digits)'),
  address_space_mib: z.number().refine(v => v >= 16, 'must be ≥ 16 MiB'),
  fsize_mib: z.number().refine(v => v >= 1, 'must be ≥ 1 MiB'),
  nproc_max: z
    .number()
    .refine(v => v >= 8 && v <= 100_000, 'must be 8..=100000'),
  nofile_max: z
    .number()
    .refine(v => v >= 64 && v <= 1_048_576, 'must be 64..=1048576'),
  cpu_secs_max: z
    .number()
    .refine(v => v >= 10 && v <= 86_400, 'must be 10..=86400'),
  timeout_secs: z
    .number()
    .refine(v => v >= 5 && v <= 86_400, 'must be 5..=86400'),
  vm_idle_evict_secs: z.number().refine(v => v >= 0, 'must be ≥ 0'),
  mac_vm_vcpus: z
    .number()
    .refine(v => v >= 1 && v <= 128, 'must be in 1..=128'),
  mac_vm_ram_mib: z
    .number()
    .refine(v => v >= 256 && v <= 262_144, 'must be in 256..=262144 MiB'),
  vm_max_concurrent_execs: z
    .number()
    .refine(v => v >= 1 && v <= 1000, 'must be in 1..=1000'),
})

const EMPTY_DEFAULTS: FormValues = {
  memory_max_mib: 512,
  memory_swap_max_mib: 0,
  pids_max: 256,
  cpu_max: '100000 100000',
  address_space_mib: 4096,
  fsize_mib: 256,
  nproc_max: 256,
  nofile_max: 1024,
  cpu_secs_max: 1240,
  timeout_secs: 620,
  vm_idle_evict_secs: 900,
  mac_vm_vcpus: 2,
  mac_vm_ram_mib: 2048,
  vm_max_concurrent_execs: 4,
}

function rowToForm(row: CodeSandboxResourceLimits): FormValues {
  return {
    memory_max_mib: Math.round(row.memory_max_bytes / MIB),
    memory_swap_max_mib: Math.round(row.memory_swap_max_bytes / MIB),
    pids_max: row.pids_max,
    cpu_max: row.cpu_max,
    address_space_mib: Math.round(row.address_space_bytes / MIB),
    fsize_mib: Math.round(row.fsize_bytes / MIB),
    nproc_max: row.nproc_max,
    nofile_max: row.nofile_max,
    cpu_secs_max: row.cpu_secs_max,
    timeout_secs: row.timeout_secs,
    vm_idle_evict_secs: row.vm_idle_evict_secs,
    mac_vm_vcpus: row.mac_vm_vcpus,
    mac_vm_ram_mib: row.mac_vm_ram_mib,
    vm_max_concurrent_execs: row.vm_max_concurrent_execs,
  }
}

function formToPatch(v: FormValues): UpdateCodeSandboxResourceLimits {
  return {
    memory_max_bytes: v.memory_max_mib * MIB,
    memory_swap_max_bytes: v.memory_swap_max_mib * MIB,
    pids_max: v.pids_max,
    cpu_max: v.cpu_max,
    address_space_bytes: v.address_space_mib * MIB,
    fsize_bytes: v.fsize_mib * MIB,
    nproc_max: v.nproc_max,
    nofile_max: v.nofile_max,
    cpu_secs_max: v.cpu_secs_max,
    timeout_secs: v.timeout_secs,
    vm_idle_evict_secs: v.vm_idle_evict_secs,
    mac_vm_vcpus: v.mac_vm_vcpus,
    mac_vm_ram_mib: v.mac_vm_ram_mib,
    vm_max_concurrent_execs: v.vm_max_concurrent_execs,
  }
}

/**
 * Resource-limits admin section. Rendered as a sequence of `<Card>` groups
 * inside the parent `SandboxSettingsPage`. Owns the singleton-row Form +
 * Save/Reset controls. Permission-aware: when the viewer lacks
 * `code_sandbox::resource_limits::manage`, the form goes read-only and Save
 * is disabled (read access is implicit via the surrounding container).
 */
export function SandboxResourceLimitsSection() {
  const { limits, loading, saving, error } = Stores.SandboxResourceLimits
  const canManage = usePermission(MANAGE_PERM)
  const canRead = usePermission(READ_PERM) || canManage

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: EMPTY_DEFAULTS,
  })
  const [dirty, setDirty] = useState(false)

  // Sync the form whenever the loaded row changes (initial load, save reply).
  useEffect(() => {
    if (limits) {
      form.reset(rowToForm(limits))
      setDirty(false)
    }
  }, [limits, form])

  // Track edits to enable/disable Reset + Save (replaces antd onValuesChange).
  useEffect(() => {
    const sub = form.watch(() => setDirty(true))
    return () => sub.unsubscribe()
  }, [form])

  const onSubmit = async (v: FormValues) => {
    try {
      await Stores.SandboxResourceLimits.saveLimits(formToPatch(v))
      message.success('Resource limits saved')
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to save')
    }
  }

  const onReset = () => {
    if (limits) {
      form.reset(rowToForm(limits))
      setDirty(false)
    }
  }

  if (!canRead) {
    return (
      <Card title="Resource limits" data-testid="sandbox-resource-limits-card">
        <Alert
          tone="warning"
          title="You don't have permission to view sandbox resource limits."
          data-testid="sandbox-resource-limits-noperm-alert"
        />
      </Card>
    )
  }

  return (
    <>
      {error && (
        <Alert
          tone="error"
          title="Failed to load resource limits"
          description={error}
          data-testid="sandbox-resource-limits-error-alert"
        />
      )}

      {loading && !limits ? (
        <Spin label="Loading resource limits…" description="Loading resource limits…" />
      ) : (
        <Card
          title="Resource limits"
          data-testid="sandbox-resource-limits-card"
          footer={
            <SettingsFormActions
              onSave={form.handleSubmit(onSubmit)}
              onCancel={onReset}
              saving={saving}
              saveDisabled={!canManage || !dirty}
              cancelDisabled={!dirty || saving}
              cancelLabel="Reset"
              saveTestid="sandbox-rl-save-btn"
              cancelTestid="sandbox-rl-reset-btn"
            />
          }
        >
        <Form
          form={form}
          layout="horizontal"
          onSubmit={onSubmit}
          disabled={!canManage}
          data-testid="sandbox-resource-limits-form"
        >
          {!canManage && (
            <Alert
              tone="info"
              title="Read-only view"
              description="You have read permission for resource limits but not manage. Save is disabled."
              data-testid="sandbox-resource-limits-readonly-alert"
            />
          )}

          {/* Flat form. Section headers via Separator (no sub-cards)
            * so the form reads as one cohesive surface, matching
            * the visual rhythm of HardwareSettings / MemoryAdmin
            * forms. */}
          <Separator titlePlacement="left">
            <Text type="secondary" className="text-xs">
              Memory
            </Text>
          </Separator>
          <FormField
            name="memory_max_mib"
            label="memory.max"
            description="cgroup v2 memory cap (MiB). OOM-kills the workload if exceeded."
          >
            <InputNumber min={16} suffix="MiB" className="w-full" data-testid="sandbox-rl-memory-max" />
          </FormField>
          <FormField
            name="memory_swap_max_mib"
            label="memory.swap.max"
            description="cgroup v2 swap cap (MiB). 0 disables swap."
          >
            <InputNumber min={0} suffix="MiB" className="w-full" data-testid="sandbox-rl-memory-swap-max" />
          </FormField>
          <FormField
            name="address_space_mib"
            label="rlimit --as"
            description="Virtual address space cap (MiB). prlimit backstop."
          >
            <InputNumber min={16} suffix="MiB" className="w-full" data-testid="sandbox-rl-address-space" />
          </FormField>

          <Separator titlePlacement="left">
            <Text type="secondary" className="text-xs">
              Processes &amp; CPU
            </Text>
          </Separator>
          <FormField name="pids_max" label="cgroup pids.max">
            <InputNumber min={8} max={100_000} className="w-full" data-testid="sandbox-rl-pids-max" />
          </FormField>
          <FormField name="nproc_max" label="rlimit --nproc">
            <InputNumber min={8} max={100_000} className="w-full" data-testid="sandbox-rl-nproc-max" />
          </FormField>
          <FormField
            name="cpu_max"
            label="cgroup cpu.max"
            description='"<quota_us> <period_us>" — "100000 100000" = 1 CPU'
          >
            <Input placeholder="100000 100000" data-testid="sandbox-rl-cpu-max" />
          </FormField>
          <FormField
            name="cpu_secs_max"
            label="rlimit --cpu (seconds)"
            description="CPU-seconds backstop. Largely redundant with the wall-clock timeout."
          >
            <InputNumber min={10} max={86_400} suffix="s" className="w-full" data-testid="sandbox-rl-cpu-secs-max" />
          </FormField>
          <FormField
            name="timeout_secs"
            label="Wall-clock per-exec timeout"
            description="Hard SIGKILL after this many seconds."
          >
            <InputNumber min={5} max={86_400} suffix="s" className="w-full" data-testid="sandbox-rl-timeout-secs" />
          </FormField>

          <Separator titlePlacement="left">
            <Text type="secondary" className="text-xs">
              Files &amp; descriptors
            </Text>
          </Separator>
          <FormField name="fsize_mib" label="rlimit --fsize (single file)">
            <InputNumber min={1} suffix="MiB" className="w-full" data-testid="sandbox-rl-fsize" />
          </FormField>
          <FormField name="nofile_max" label="rlimit --nofile">
            <InputNumber min={64} max={1_048_576} className="w-full" data-testid="sandbox-rl-nofile-max" />
          </FormField>

          <Separator titlePlacement="left">
            <Text type="secondary" className="text-xs">
              VM lifecycle (macOS + Windows)
            </Text>
          </Separator>
          <FormField
            name="vm_idle_evict_secs"
            label="Idle-evict timeout"
            description="After this many idle seconds with nothing in-flight, the per-flavor microVM / WSL2 distro is evicted to free its RAM. Set to 0 to disable eviction (warm VMs hold memory indefinitely)."
          >
            <InputNumber min={0} suffix="s (0 = never)" className="w-full" data-testid="sandbox-rl-vm-idle-evict" />
          </FormField>
          <FormField
            name="vm_max_concurrent_execs"
            label="Concurrent execs per VM / distro"
            description="Cap on parallel execute_command calls that share one VM. Each call is cgroup-capped at memory.max; this bound keeps N concurrent calls from summing past the VM's RAM ceiling. Applies to macOS libkrun + Windows WSL2."
          >
            <InputNumber min={1} max={1000} className="w-full" data-testid="sandbox-rl-vm-max-execs" />
          </FormField>

          <Separator titlePlacement="left">
            <Text type="secondary" className="text-xs">
              macOS libkrun VM sizing
            </Text>
          </Separator>
          <FormField
            name="mac_vm_vcpus"
            label="vCPUs"
            description="Per-flavor libkrun microVM vCPU count (krun_set_vm_config num_vcpus). Applies on the NEXT cold boot of a flavor; warm VMs keep their boot-time sizing."
          >
            <InputNumber min={1} max={128} className="w-full" data-testid="sandbox-rl-mac-vcpus" />
          </FormField>
          <FormField
            name="mac_vm_ram_mib"
            label="RAM ceiling"
            description="Per-flavor libkrun microVM RAM ceiling in MiB. Host RAM is demand-paged; this is the upper bound. Applies on the NEXT cold boot of a flavor."
          >
            <InputNumber min={256} max={262_144} suffix="MiB" className="w-full" data-testid="sandbox-rl-mac-ram" />
          </FormField>

          <Paragraph type="secondary" className="mt-6">
            Defaults: 512 MiB memory, 256 PIDs, 1 CPU, 4 GiB address space,
            256 MiB single-file, 1024 nofile, 1240 s CPU-seconds backstop,
            620 s wall-clock, 900 s VM idle-evict. Values stored at{' '}
            <code>code_sandbox_settings</code>; the server invalidates its
            in-process cache on save.
          </Paragraph>
        </Form>
        </Card>
      )}
    </>
  )
}

// Marker to silence the `GIB` unused-import warning while keeping the constant
// available for future inputs that prefer GiB display granularity.
void GIB
