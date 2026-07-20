import { useEffect } from 'react'
import {
  Alert,
  Card,
  Combobox,
  Form,
  FormField,
  InputNumber,
  Paragraph,
  Select,
  Separator,
  Spinner,
  Switch,
  Text,
  Textarea,
  message,
  useForm,
  zodResolver,
} from '@ziee/kit'
import { z } from 'zod'
import { Stores } from '@ziee/framework/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'

const READ_PERM = Permissions.AgentSettingsRead
const MANAGE_PERM = Permissions.AgentSettingsManage

const MAX_REVIEWER_POLICY_LEN = 32_768

// Enum vocabularies — mirror the backend CHECK constraints
// (agent_admin_settings migration + `VALID_SANDBOX_MODES` / `VALID_APPROVAL_POLICIES`).
const SANDBOX_MODE_OPTIONS = [
  { value: 'read-only', label: 'Read-only' },
  { value: 'workspace-write', label: 'Workspace write' },
  { value: 'danger-full-access', label: 'Danger — full access' },
]
const APPROVAL_POLICY_OPTIONS = [
  { value: 'untrusted', label: 'Untrusted (prompt for everything untrusted)' },
  { value: 'on-failure', label: 'On failure' },
  { value: 'on-request', label: 'On request' },
  { value: 'never', label: 'Never (fully unattended)' },
]

// Reviewer risk-threshold editor (DEC-83/84). The backend consumes a FLAT
// `{ band: decision }` JSON object (agent-core `RiskThresholds::from_json`):
// band ∈ low|high|critical, decision ∈ auto|prompt|review|deny. A band the map
// OMITS falls back to the built-in ladder (Low→Auto, High→Prompt, Critical→Deny),
// so the `'default'` option here maps to omission on save.
//
// NOTE (flagged): DEC-84/89 reserve a FUTURE per-category nest in the same jsonb
// (RiskCategory → threshold), but the crate's `from_json` does NOT read it today
// (the classifier carries `category` only for journalling; the decision gates on
// band + authorization). So only the KNOWN band overrides are typed + editable
// here; a per-category editor is deferred until the crate consumes that nest.
const REVIEWER_DECISION_OPTIONS = [
  { value: 'default', label: 'Default (built-in ladder)' },
  { value: 'auto', label: 'Auto — allow without asking' },
  { value: 'prompt', label: 'Prompt — ask a human' },
  { value: 'review', label: 'Review — re-run the reviewer' },
  { value: 'deny', label: 'Deny — block the call' },
]

const RISK_DECISION_ENUM = [
  'default',
  'auto',
  'prompt',
  'review',
  'deny',
] as const
type ReviewerDecision = (typeof RISK_DECISION_ENUM)[number]
// The real decisions (everything except the `default` sentinel).
const VALID_DECISIONS: readonly string[] = ['auto', 'prompt', 'review', 'deny']

/** Read a single band's stored override out of the `unknown` jsonb, narrowing
 *  safely (no `as any`); an unknown / invalid value degrades to `'default'`. */
function readBandOverride(
  obj: Record<string, unknown>,
  band: string,
): ReviewerDecision {
  const raw = obj[band]
  if (typeof raw === 'string') {
    const v = raw.trim().toLowerCase()
    if (VALID_DECISIONS.includes(v)) return v as ReviewerDecision
  }
  return 'default'
}

/** Parse `settings.reviewer_risk_thresholds` (typed `unknown`) into the three
 *  per-band form fields. A non-object / array value → all defaults. */
function readThresholds(raw: unknown): {
  risk_low: ReviewerDecision
  risk_high: ReviewerDecision
  risk_critical: ReviewerDecision
} {
  if (raw && typeof raw === 'object' && !Array.isArray(raw)) {
    const obj = raw as Record<string, unknown>
    return {
      risk_low: readBandOverride(obj, 'low'),
      risk_high: readBandOverride(obj, 'high'),
      risk_critical: readBandOverride(obj, 'critical'),
    }
  }
  return { risk_low: 'default', risk_high: 'default', risk_critical: 'default' }
}

const schema = z.object({
  default_sandbox_mode: z.enum([
    'read-only',
    'workspace-write',
    'danger-full-access',
  ]),
  unattended_approval_policy: z.enum([
    'untrusted',
    'on-failure',
    'on-request',
    'never',
  ]),
  reviewer_enabled: z.boolean(),
  // Combobox clears to '' — coerced to null on submit.
  reviewer_model_id: z.string().nullable(),
  reviewer_policy: z
    .string()
    .max(MAX_REVIEWER_POLICY_LEN, 'must be ≤ 32 KiB')
    .nullable(),
  per_run_token_cap: z
    .number()
    .refine(v => v >= 1_000 && v <= 1_000_000_000, 'must be 1000..=1000000000'),
  per_step_token_cap: z
    .number()
    .refine(v => v >= 1_000 && v <= 1_000_000_000, 'must be 1000..=1000000000'),
  default_max_steps: z
    .number()
    .refine(v => v >= 1 && v <= 1_000, 'must be 1..=1000'),
  fan_out_max_threads: z
    .number()
    .refine(v => v >= 1 && v <= 64, 'must be 1..=64'),
  fan_out_max_depth: z.number().refine(v => v >= 1 && v <= 5, 'must be 1..=5'),
  // Per-`delegate`-call child cap (DEC-1), bounds mirror the PUT handler (1..=64).
  fan_out_max_children_per_call: z
    .number()
    .refine(v => v >= 1 && v <= 64, 'must be 1..=64'),
  // Goal-seeking (DEC-61/62). Evaluator model is nullable (Combobox clears to
  // '' → coerced to null on submit); the turn cap is 1..=50.
  goal_eval_model_id: z.string().nullable(),
  goal_seek_max_turns: z
    .number()
    .refine(v => v >= 1 && v <= 50, 'must be 1..=50'),
  // Reviewer per-band risk thresholds (DEC-83). `'default'` = omit the band.
  risk_low: z.enum(RISK_DECISION_ENUM),
  risk_high: z.enum(RISK_DECISION_ENUM),
  risk_critical: z.enum(RISK_DECISION_ENUM),
})
type FormValues = z.infer<typeof schema>

/** Reconstruct the flat `{ band: decision }` jsonb from the three form fields;
 *  a `'default'` band is OMITTED (→ the crate's built-in ladder). An empty
 *  object clears every override back to the default ladder. */
function buildThresholds(v: FormValues): Record<string, string> {
  const out: Record<string, string> = {}
  if (v.risk_low !== 'default') out.low = v.risk_low
  if (v.risk_high !== 'default') out.high = v.risk_high
  if (v.risk_critical !== 'default') out.critical = v.risk_critical
  return out
}

const EMPTY_DEFAULTS: FormValues = {
  default_sandbox_mode: 'workspace-write',
  unattended_approval_policy: 'on-request',
  reviewer_enabled: true,
  reviewer_model_id: null,
  reviewer_policy: null,
  per_run_token_cap: 5_000_000,
  per_step_token_cap: 2_000_000,
  default_max_steps: 30,
  fan_out_max_threads: 6,
  fan_out_max_depth: 1,
  fan_out_max_children_per_call: 8,
  goal_eval_model_id: null,
  goal_seek_max_turns: 10,
  risk_low: 'default',
  risk_high: 'default',
  risk_critical: 'default',
}

/**
 * Deployment-wide agent policy admin card: sandbox/approval mode for unattended
 * runs, the reviewer agent (enable + model + steering policy), and the token /
 * step / fan-out budget caps. Rendered inside `AgentSettingsPage`. Permission-
 * aware: without `agent::settings::manage` the form goes read-only and Save
 * hides; without `agent::settings::read` a permission-denied alert shows.
 */
export function AgentSettingsSection() {
  const canManage = usePermission(MANAGE_PERM)
  const canRead = usePermission(READ_PERM) || canManage
  const { settings, availableModels, loading, saving, loadingModels, error } =
    Stores.AgentAdminSettings

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: EMPTY_DEFAULTS,
  })

  useEffect(() => {
    if (settings) {
      form.reset({
        default_sandbox_mode:
          settings.default_sandbox_mode as FormValues['default_sandbox_mode'],
        unattended_approval_policy:
          settings.unattended_approval_policy as FormValues['unattended_approval_policy'],
        reviewer_enabled: settings.reviewer_enabled,
        reviewer_model_id: settings.reviewer_model_id ?? null,
        reviewer_policy: settings.reviewer_policy ?? null,
        per_run_token_cap: settings.per_run_token_cap,
        per_step_token_cap: settings.per_step_token_cap,
        default_max_steps: settings.default_max_steps,
        fan_out_max_threads: settings.fan_out_max_threads,
        fan_out_max_depth: settings.fan_out_max_depth,
        fan_out_max_children_per_call: settings.fan_out_max_children_per_call,
        goal_eval_model_id: settings.goal_eval_model_id ?? null,
        goal_seek_max_turns: settings.goal_seek_max_turns,
        ...readThresholds(settings.reviewer_risk_thresholds),
      })
    }
  }, [settings, form])

  if (!canRead) {
    return (
      <Card title="Agent policy" data-testid="agent-settings-card">
        <Alert
          tone="warning"
          title="You don't have permission to view agent settings."
          data-testid="agent-settings-noperm-alert"
        />
      </Card>
    )
  }

  const onSubmit = async (v: FormValues) => {
    try {
      await Stores.AgentAdminSettings.update({
        default_sandbox_mode: v.default_sandbox_mode,
        unattended_approval_policy: v.unattended_approval_policy,
        reviewer_enabled: v.reviewer_enabled,
        // Empty selection → null (clear back to "fall back to the run's model").
        reviewer_model_id: v.reviewer_model_id ? v.reviewer_model_id : null,
        reviewer_policy: v.reviewer_policy ? v.reviewer_policy : null,
        per_run_token_cap: v.per_run_token_cap,
        per_step_token_cap: v.per_step_token_cap,
        default_max_steps: v.default_max_steps,
        fan_out_max_threads: v.fan_out_max_threads,
        fan_out_max_depth: v.fan_out_max_depth,
        fan_out_max_children_per_call: v.fan_out_max_children_per_call,
        // Empty selection → null (clear back to "use the run's own model").
        goal_eval_model_id: v.goal_eval_model_id ? v.goal_eval_model_id : null,
        goal_seek_max_turns: v.goal_seek_max_turns,
        reviewer_risk_thresholds: buildThresholds(v),
      })
      message.success('Agent settings saved')
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to save agent settings')
    }
  }

  const onReset = () => {
    if (settings) form.reset()
  }

  return (
    <>
      {error && (
        <Alert
          tone="error"
          title="Failed to load agent settings"
          description={error}
          className="mb-4"
          data-testid="agent-settings-error-alert"
        />
      )}

      {loading && !settings ? (
        <Spinner label="Loading agent settings" />
      ) : (
        <Card
          title="Agent policy"
          data-testid="agent-settings-card"
          footer={
            canManage ? (
              <SettingsFormActions
                onSave={form.handleSubmit(onSubmit)}
                onCancel={onReset}
                saving={saving}
                cancelLabel="Reset"
                saveTestid="agent-settings-save-btn"
                cancelTestid="agent-settings-reset-btn"
              />
            ) : undefined
          }
        >
          <Form
            name="agent-settings-form"
            form={form}
            layout="horizontal"
            onSubmit={onSubmit}
            disabled={!canManage}
            data-testid="agent-settings-form"
          >
            {!canManage && (
              <Alert
                tone="info"
                title="Read-only view"
                description="You can view the agent policy but not change it. Save is hidden."
                className="mb-4"
                data-testid="agent-settings-readonly-alert"
              />
            )}

            <Separator titlePlacement="left">
              <Text type="secondary" className="text-xs">
                Sandbox &amp; approval
              </Text>
            </Separator>
            <FormField
              name="default_sandbox_mode"
              label="Default sandbox mode"
              description="How much filesystem / network an unattended run may touch (Codex SandboxMode)."
            >
              <Select
                options={SANDBOX_MODE_OPTIONS}
                placeholder="Select a sandbox mode"
                className="max-w-[420px]"
                data-testid="agent-settings-sandbox-mode"
              />
            </FormField>
            <FormField
              name="unattended_approval_policy"
              label="Unattended approval policy"
              description="When an unattended run hits a mutating / external tool call, this decides whether it pauses for approval (Codex ApprovalMode)."
            >
              <Select
                options={APPROVAL_POLICY_OPTIONS}
                placeholder="Select an approval policy"
                className="max-w-[420px]"
                data-testid="agent-settings-approval-policy"
              />
            </FormField>

            <Separator titlePlacement="left">
              <Text type="secondary" className="text-xs">
                Reviewer agent
              </Text>
            </Separator>
            <FormField
              name="reviewer_enabled"
              label="Enable reviewer agent"
              description="A cheap agent that risk-classifies each approval-needing tool call before it escalates to a human. Fail-closed when on."
              valuePropName="checked"
            >
              <Switch
                aria-label="Enable reviewer agent"
                data-testid="agent-settings-reviewer-enabled"
              />
            </FormField>
            <FormField
              name="reviewer_model_id"
              label="Reviewer model"
              description="Model the reviewer uses. Leave empty to fall back to the run's own model."
            >
              <Combobox
                data-testid="agent-settings-reviewer-model"
                placeholder={
                  !loadingModels && availableModels.length === 0
                    ? 'No models — add one on the LLM Providers page'
                    : 'Select a reviewer model (optional)'
                }
                searchPlaceholder="Search models"
                emptyText="No models"
                loading={loadingModels}
                options={availableModels.map(m => ({
                  value: m.id,
                  label: m.display_name || m.name,
                }))}
                className="max-w-[420px]"
              />
            </FormField>
            <FormField
              name="reviewer_policy"
              label="Reviewer policy"
              description="Free-text steering for the reviewer's risk classification. Leave empty for the built-in default prompt."
            >
              <Textarea
                rows={4}
                maxLength={MAX_REVIEWER_POLICY_LEN}
                placeholder="e.g. Treat any write to a shared credential store as Critical."
                data-testid="agent-settings-reviewer-policy"
              />
            </FormField>
            <FormField
              name="risk_low"
              label="Low-risk decision"
              description="What the reviewer does with a call it classifies Low-risk. Default: Auto (allow without asking)."
            >
              <Select
                options={REVIEWER_DECISION_OPTIONS}
                className="max-w-[420px]"
                data-testid="agent-settings-risk-low"
              />
            </FormField>
            <FormField
              name="risk_high"
              label="High-risk decision"
              description="What the reviewer does with a High-risk call. Default: Prompt (ask a human). A well-authorized High call can still auto-proceed via the authorization gate."
            >
              <Select
                options={REVIEWER_DECISION_OPTIONS}
                className="max-w-[420px]"
                data-testid="agent-settings-risk-high"
              />
            </FormField>
            <FormField
              name="risk_critical"
              label="Critical-risk decision"
              description="What the reviewer does with a Critical-risk call. Default: Deny (block it). Critical never auto-proceeds regardless of this override."
            >
              <Select
                options={REVIEWER_DECISION_OPTIONS}
                className="max-w-[420px]"
                data-testid="agent-settings-risk-critical"
              />
            </FormField>

            <Separator titlePlacement="left">
              <Text type="secondary" className="text-xs">
                Budget &amp; limits
              </Text>
            </Separator>
            <FormField
              name="per_run_token_cap"
              label="Per-run token cap"
              description="Total tokens a single agent run may consume before it halts. The real cost bound."
            >
              <InputNumber
                min={1_000}
                max={1_000_000_000}
                suffix="tokens"
                className="w-full max-w-[280px]"
                data-testid="agent-settings-per-run-token-cap"
              />
            </FormField>
            <FormField
              name="per_step_token_cap"
              label="Per-step token cap"
              description="Tokens a single step (one LLM call) may consume."
            >
              <InputNumber
                min={1_000}
                max={1_000_000_000}
                suffix="tokens"
                className="w-full max-w-[280px]"
                data-testid="agent-settings-per-step-token-cap"
              />
            </FormField>
            <FormField
              name="default_max_steps"
              label="Default max steps"
              description="Iteration failsafe: the maximum number of tool-call loops per run."
            >
              <InputNumber
                min={1}
                max={1_000}
                className="w-full max-w-[200px]"
                data-testid="agent-settings-default-max-steps"
              />
            </FormField>
            <FormField
              name="fan_out_max_threads"
              label="Fan-out max threads"
              description="Maximum number of subagents that may run in parallel during a fan-out."
            >
              <InputNumber
                min={1}
                max={64}
                className="w-full max-w-[200px]"
                data-testid="agent-settings-fan-out-max-threads"
              />
            </FormField>
            <FormField
              name="fan_out_max_depth"
              label="Fan-out max depth"
              description="How deep fan-out may nest. 1 = subagents cannot themselves fan out."
            >
              <InputNumber
                min={1}
                max={5}
                className="w-full max-w-[200px]"
                data-testid="agent-settings-fan-out-max-depth"
              />
            </FormField>
            <FormField
              name="fan_out_max_children_per_call"
              label="Fan-out max children per call"
              description="Maximum number of subagents accepted in a single delegate call. Over-cap requests are truncated with an explicit note, never silently dropped."
            >
              <InputNumber
                min={1}
                max={64}
                className="w-full max-w-[200px]"
                data-testid="agent-settings-fan-out-max-children-per-call"
              />
            </FormField>

            <Separator titlePlacement="left">
              <Text type="secondary" className="text-xs">
                Goal-seeking
              </Text>
            </Separator>
            <FormField
              name="goal_eval_model_id"
              label="Goal evaluator model"
              description="Cheap model that judges a goal-seeking turn's result against the task's completion condition. Leave empty to fall back to the run's own model."
            >
              <Combobox
                data-testid="agent-settings-goal-eval-model"
                placeholder={
                  !loadingModels && availableModels.length === 0
                    ? 'No models — add one on the LLM Providers page'
                    : 'Select an evaluator model (optional)'
                }
                searchPlaceholder="Search models"
                emptyText="No models"
                loading={loadingModels}
                options={availableModels.map(m => ({
                  value: m.id,
                  label: m.display_name || m.name,
                }))}
                className="max-w-[420px]"
              />
            </FormField>
            <FormField
              name="goal_seek_max_turns"
              label="Goal-seeking max turns"
              description="Maximum turns a goal-seeking loop may fire before it stops 'incomplete'. The scheduler's horizon backstop is the other ceiling."
            >
              <InputNumber
                min={1}
                max={50}
                className="w-full max-w-[200px]"
                data-testid="agent-settings-goal-seek-max-turns"
              />
            </FormField>

            <Paragraph type="secondary" className="mt-6">
              Defaults: workspace-write sandbox, on-request approval, reviewer
              on with the built-in risk ladder (Low → Auto, High → Prompt,
              Critical → Deny), 5,000,000 per-run / 2,000,000 per-step token
              caps, 30 max steps, fan-out 6 threads × depth 1 × 8 children per
              call, goal-seeking 10 turns on the run's own model. Stored at{' '}
              <code>agent_admin_settings</code>; the server re-reads the row at
              use.
            </Paragraph>
          </Form>
        </Card>
      )}
    </>
  )
}
