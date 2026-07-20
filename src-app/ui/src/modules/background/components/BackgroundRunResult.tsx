import type { BackgroundRunDetail } from '@/api-client/types'
import { Flex, Tag, type TagTone, Text } from '@ziee/kit'

/**
 * Inline renderer for a terminal background run's `final_output_json` (ITEM-8
 * detail follow-up). The body is model-/tool-produced, so every shape is
 * NARROWED with `typeof` / `in` guards (never `as any`): a `subagent` run carries
 * `{ executor, final_text }`; a `sandbox_exec` run carries `{ stdout, stderr,
 * exit_code, timed_out, duration_ms, *_truncated }`; an unknown kind falls back
 * to a pretty-printed JSON block. A run that never produced a result (failed /
 * cancelled) renders an explanatory line rather than nothing.
 */
export function BackgroundRunResult({ detail }: { detail: BackgroundRunDetail }) {
  const output = detail.final_output_json
  const runId = detail.id

  // No structured result body — typical for a failed / cancelled run that never
  // produced one. Explain the state (never render nothing).
  if (!isRecord(output)) {
    const message =
      detail.status === 'failed'
        ? 'This task failed before producing a result.'
        : detail.status === 'cancelled'
          ? 'This task was cancelled before producing a result.'
          : 'No result was collected for this task.'
    return (
      <Text
        type="secondary"
        className="text-sm"
        data-testid={`background-run-result-none-${runId}`}
      >
        {message}
      </Text>
    )
  }

  // Dispatch on SHAPE, not on `job_kind` — the payload is untrusted output, so
  // the kind is only a hint; guard on the fields we actually read.
  if (typeof output.final_text === 'string') {
    return <SubagentResult output={output} runId={runId} />
  }
  if (
    'stdout' in output ||
    'stderr' in output ||
    'exit_code' in output ||
    'timed_out' in output
  ) {
    return <SandboxResult output={output} runId={runId} />
  }
  return <JsonFallback value={output} runId={runId} />
}

/** A sub-agent run's answer — its last assistant text (`final_text`). */
function SubagentResult({
  output,
  runId,
}: {
  output: Record<string, unknown>
  runId: string
}) {
  const finalText = asString(output.final_text)
  const tokens = asNumber(output.tokens_used)
  return (
    <Flex
      className="flex-col gap-2"
      data-testid={`background-run-result-subagent-${runId}`}
    >
      {tokens !== null && (
        <Text type="secondary" className="text-xs">
          {tokens.toLocaleString()} tokens
        </Text>
      )}
      {finalText ? (
        <div
          data-testid={`background-run-final-text-${runId}`}
          className="max-h-96 overflow-auto whitespace-pre-wrap break-words rounded-md bg-muted p-3 text-sm"
        >
          {finalText}
        </div>
      ) : (
        <Text type="secondary" className="text-sm">
          The sub-agent produced no text output.
        </Text>
      )}
    </Flex>
  )
}

/** A sandbox-exec run's stdout/stderr + an exit-code / timed-out badge row. */
function SandboxResult({
  output,
  runId,
}: {
  output: Record<string, unknown>
  runId: string
}) {
  const stdout = asString(output.stdout)
  const stderr = asString(output.stderr)
  const exitCode = asNumber(output.exit_code)
  const timedOut = output.timed_out === true
  const durationMs = asNumber(output.duration_ms)
  const stdoutTruncated = output.stdout_truncated === true
  const stderrTruncated = output.stderr_truncated === true

  const exitTone: TagTone = timedOut
    ? 'warning'
    : exitCode === null
      ? 'default'
      : exitCode === 0
        ? 'success'
        : 'error'

  return (
    <Flex
      className="flex-col gap-2"
      data-testid={`background-run-result-sandbox-${runId}`}
    >
      <Flex className="flex-wrap items-center gap-2">
        {timedOut && (
          <Tag
            variant="outline"
            tone="warning"
            data-testid={`background-run-timed-out-${runId}`}
          >
            Timed out
          </Tag>
        )}
        <Tag
          variant="outline"
          tone={exitTone}
          data-testid={`background-run-exit-code-${runId}`}
        >
          {exitCode === null ? 'exit —' : `exit ${exitCode}`}
        </Tag>
        {durationMs !== null && (
          <Text type="secondary" className="text-xs">
            {durationMs} ms
          </Text>
        )}
      </Flex>

      <StreamBlock
        label="stdout"
        text={stdout}
        truncated={stdoutTruncated}
        testid={`background-run-stdout-${runId}`}
      />
      {(stderr || stderrTruncated) && (
        <StreamBlock
          label="stderr"
          text={stderr}
          truncated={stderrTruncated}
          testid={`background-run-stderr-${runId}`}
        />
      )}
    </Flex>
  )
}

/** A labelled monospace output stream (stdout / stderr), with a truncation note. */
function StreamBlock({
  label,
  text,
  truncated,
  testid,
}: {
  label: string
  text: string
  truncated: boolean
  testid: string
}) {
  return (
    <Flex className="flex-col gap-1">
      <Text type="secondary" className="text-xs">
        {label}
        {truncated ? ' (truncated)' : ''}
      </Text>
      {text ? (
        <MonoBlock text={text} testid={testid} />
      ) : (
        <Text type="secondary" className="text-xs">
          (empty)
        </Text>
      )}
    </Flex>
  )
}

/** Unknown result shape — pretty-print the JSON so nothing is silently dropped. */
function JsonFallback({ value, runId }: { value: unknown; runId: string }) {
  let text: string
  try {
    text = JSON.stringify(value, null, 2)
  } catch {
    text = String(value)
  }
  return <MonoBlock text={text} testid={`background-run-result-json-${runId}`} />
}

/**
 * Tokenized monospace container. There is no kit code-block primitive, so this
 * is the sanctioned tokenized fallback (mirrors `LiveLogsPanel`): `bg-muted`
 * surface, its own `overflow-auto` so wide output scrolls inside the block
 * instead of the page.
 */
function MonoBlock({ text, testid }: { text: string; testid?: string }) {
  return (
    <div
      data-testid={testid}
      className="max-h-72 overflow-auto whitespace-pre-wrap break-words rounded-md bg-muted p-3 font-mono text-xs"
    >
      {text}
    </div>
  )
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

const asString = (value: unknown): string =>
  typeof value === 'string' ? value : ''

const asNumber = (value: unknown): number | null =>
  typeof value === 'number' && Number.isFinite(value) ? value : null
