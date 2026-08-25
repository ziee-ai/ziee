import { Bot, ChevronDown, MessageSquare, Terminal, XCircle } from 'lucide-react'
import { useState } from 'react'

import type { BackgroundRunDetail, BackgroundRunSummary } from '@/api-client/types'
import {
  Alert,
  Button,
  Card,
  Confirm,
  Flex,
  message,
  Spin,
  Tabs,
  Tag,
  type TagTone,
  Text,
  Textarea,
} from '@ziee/kit'

import { isTerminalRunStatus } from '../stores/BackgroundRuns.store'
import { BackgroundRunResult } from './BackgroundRunResult'
import { BackgroundRuns } from '@/modules/background/stores/BackgroundRuns.store'
import { AgentActivityTimeline } from '@/modules/workflow/components/run/AgentActivityTimeline'
import type { AgentActivityEntry } from '@/modules/workflow/components/run/activityDescriptors'

// Status → Tag tone. `cancelled` stays neutral (`default`), never the red
// `error` of `failed` — mirrors the tool-call history convention so a
// user-cancelled task never reads as a failure.
const STATUS_TONE: Record<string, TagTone> = {
  pending: 'default',
  running: 'info',
  waiting: 'warning',
  resumable: 'warning',
  completed: 'success',
  failed: 'error',
  cancelled: 'default',
}

const KIND_LABEL: Record<string, string> = {
  subagent: 'Sub-agent',
  sandbox_exec: 'Sandbox',
}

/** Leading glyph per run kind — mirrors the sibling `SubAgentActivityCard`
 *  (a leading kind icon before the title). Sandbox runs read as a terminal. */
function KindIcon({ jobKind }: { jobKind: string }) {
  const Icon = jobKind === 'sandbox_exec' ? Terminal : Bot
  return <Icon aria-hidden className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
}

// Small dependency-free relative time ("2m ago" / "1h ago" / "3d ago") — mirrors
// the helper in AuthProvidersListSection; Intl.RelativeTimeFormat is heavier than
// this list UI needs.
function relativeTime(iso: string): string {
  const then = new Date(iso).getTime()
  if (Number.isNaN(then)) return ''
  const secs = Math.floor((Date.now() - then) / 1000)
  if (secs < 60) return `${secs}s ago`
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`
  if (secs < 86400) return `${Math.floor(secs / 3600)}h ago`
  return `${Math.floor(secs / 86400)}d ago`
}

const notifyError = (e: unknown, fallback: string): void => {
  message.error(e instanceof Error ? e.message : fallback)
}

/**
 * One background-run row (ITEM-8 / ITEM-25). Shows the run's kind glyph, status
 * badge, label, relative start time and token count; lets the user cancel a
 * non-terminal run (confirmed) and queue a steering note to it.
 *
 * A TERMINAL run expands (lazily) into a two-tab detail region — **Transcript**
 * (the durable agent-loop transcript, the shared workflow `AgentActivityTimeline`
 * over `detail.activity`) and **Result** (`BackgroundRunResult`). Transcript is
 * the default tab: the job a user has here is "open a task and see what the agent
 * did", and the result is one click away. Detail is fetched once on first expand
 * (`loadRunDetail`, cached), so the panel never eagerly fans out N detail fetches.
 *
 * The card carries `shrink-0`: the kit `Card` is `overflow-hidden`, which as a
 * flex child computes `min-height:0` and would otherwise let the panel's flex
 * column SHRINK each card and clip its meta row/actions (measured 58px offset vs
 * 152px content). `shrink-0` keeps the card at its natural height so the panel's
 * `overflow-y-auto` owns the scroll.
 *
 * Cancel + steer are gated on `!isTerminalRunStatus(run.status)` — the exact
 * boundary the backend enforces (both endpoints 409 on a terminal run). There is
 * deliberately NO "Open conversation" affordance: the card's only render site is
 * the in-conversation Tasks panel, whose runs already belong to the conversation
 * being read.
 */
export function BackgroundRunCard({ run }: { run: BackgroundRunSummary }) {
  const [cancelOpen, setCancelOpen] = useState(false)
  const [cancelling, setCancelling] = useState(false)
  const [steerOpen, setSteerOpen] = useState(false)
  const [note, setNote] = useState('')
  const [posting, setPosting] = useState(false)
  const [detailsOpen, setDetailsOpen] = useState(false)

  const terminal = isTerminalRunStatus(run.status)
  // Reactive read (subscribes) — the row re-renders when its notes load / change.
  const notes = BackgroundRuns.notesByRun[run.id] ?? []
  const pendingNotes = notes.filter(n => !n.consumed_at)

  // Reactive reads for the inline detail view (subscribe → re-render on fetch).
  const detail = BackgroundRuns.detailsByRun[run.id]
  const detailError = BackgroundRuns.detailErrorByRun[run.id]

  const toggleDetails = () => {
    setDetailsOpen(open => {
      const next = !open
      // Lazy-fetch the detail body only when the region is first opened; the store
      // caches it, so re-expanding a terminal run never refetches.
      if (next) void BackgroundRuns.loadRunDetail(run.id)
      return next
    })
  }

  const toggleSteer = () => {
    setSteerOpen(open => {
      const next = !open
      // Lazy-load the pending-note list only when the composer is opened.
      if (next) void BackgroundRuns.loadNotes(run.id)
      return next
    })
  }

  const submitNote = async () => {
    const text = note.trim()
    if (!text) return
    setPosting(true)
    try {
      await BackgroundRuns.postNote(run.id, text)
      setNote('')
      message.success('Steering note queued')
    } catch (e) {
      notifyError(e, 'Failed to queue the steering note')
    } finally {
      setPosting(false)
    }
  }

  const tokens = run.total_tokens > 0 ? run.total_tokens : null

  return (
    <Card size="sm" className="shrink-0" data-testid={`background-run-card-${run.id}`}>
      <Flex className="flex-col gap-2">
        {/* Kind glyph + title + status pill on one row. The title is `flex-1
            min-w-0 line-clamp-2`, so it takes the row's width and clamps to two
            lines (never a hard single-line truncate) while the status pill hugs
            the end; at 390px the title wraps to two lines and the pill stays
            top-right beside it — no clip. */}
        <Flex className="flex-wrap items-start gap-x-2 gap-y-1">
          <KindIcon jobKind={run.job_kind} />
          <Text strong className="min-w-0 flex-1 line-clamp-2 break-words text-sm">
            {run.label ?? 'Untitled run'}
          </Text>
          <Tag
            variant="outline"
            tone={STATUS_TONE[run.status] ?? 'default'}
            data-testid={`background-run-status-${run.id}`}
          >
            {run.status}
          </Tag>
        </Flex>

        {/* Compact meta row: kind · time · tokens · result-ready. */}
        <Flex className="flex-wrap items-center gap-x-2 gap-y-1 text-muted-foreground">
          <Tag variant="outline" data-testid={`background-run-kind-${run.id}`}>
            {KIND_LABEL[run.job_kind] ?? run.job_kind}
          </Tag>
          <Text type="secondary" className="text-xs">
            {relativeTime(run.created_at)}
          </Text>
          {tokens !== null && (
            <Text type="secondary" className="text-xs">
              {tokens.toLocaleString()} tokens
            </Text>
          )}
          {run.has_result && (
            <Tag
              variant="outline"
              tone="success"
              className="ms-auto"
              data-testid={`background-run-result-${run.id}`}
            >
              Result ready
            </Tag>
          )}
        </Flex>

        {/* Failure detail */}
        {run.status === 'failed' && run.error_message && (
          <Text
            type="danger"
            className="text-sm"
            data-testid={`background-run-error-${run.id}`}
          >
            {run.error_message}
          </Text>
        )}

        {/* Actions */}
        <Flex className="flex-wrap items-center gap-2">
          {terminal && (
            <Button
              variant="ghost"
              icon={
                <ChevronDown
                  className={detailsOpen ? 'rotate-180 transition-transform' : 'transition-transform'}
                />
              }
              aria-expanded={detailsOpen}
              aria-controls={`background-run-details-panel-${run.id}`}
              data-testid={`background-run-details-toggle-${run.id}`}
              onClick={toggleDetails}
            >
              {detailsOpen ? 'Hide details' : 'Show details'}
            </Button>
          )}
          {!terminal && (
            <Button
              variant="ghost"
              icon={<MessageSquare />}
              aria-expanded={steerOpen}
              data-testid={`background-run-steer-toggle-${run.id}`}
              onClick={toggleSteer}
            >
              Steer
            </Button>
          )}
          {!terminal && (
            <>
              <Button
                variant="destructive"
                icon={<XCircle />}
                loading={cancelling}
                data-testid={`background-run-cancel-${run.id}`}
                onClick={() => setCancelOpen(true)}
              >
                Cancel
              </Button>
              <Confirm
                data-testid={`background-run-cancel-confirm-${run.id}`}
                open={cancelOpen}
                onOpenChange={setCancelOpen}
                title="Cancel background task"
                description={`Stop "${run.label ?? 'this run'}"? It cannot be resumed.`}
                okText="Cancel task"
                cancelText="Keep running"
                okButtonProps={{ danger: true }}
                onConfirm={async () => {
                  setCancelling(true)
                  try {
                    await BackgroundRuns.cancelRun(run.id)
                    message.success('Background task cancelled')
                  } catch (e) {
                    notifyError(e, 'Failed to cancel the task')
                  } finally {
                    setCancelling(false)
                  }
                }}
              />
            </>
          )}
        </Flex>

        {/* Steering composer (non-terminal only) */}
        {!terminal && steerOpen && (
          <Flex
            className="flex-col gap-2 rounded-md border p-3"
            data-testid={`background-run-steer-${run.id}`}
          >
            {pendingNotes.length > 0 && (
              <Flex className="flex-col gap-1">
                <Text type="secondary" className="text-xs">
                  Pending notes
                </Text>
                {pendingNotes.map(n => (
                  <Text
                    key={n.id}
                    className="text-sm"
                    data-testid={`background-run-note-${n.id}`}
                  >
                    {n.note}
                  </Text>
                ))}
              </Flex>
            )}
            <Textarea
              data-testid={`background-run-note-input-${run.id}`}
              value={note}
              onChange={e => setNote(e.target.value)}
              placeholder="Nudge or redirect this task without restarting it…"
              rows={2}
              maxLength={4000}
              aria-label="Steering note"
            />
            <Flex className="justify-end">
              <Button
                variant="default"
                loading={posting}
                disabled={!note.trim()}
                data-testid={`background-run-note-send-${run.id}`}
                onClick={submitNote}
              >
                Send note
              </Button>
            </Flex>
          </Flex>
        )}

        {/* Detail region (terminal runs only) — Transcript + Result tabs,
            lazily fetched on expand. */}
        {terminal && detailsOpen && (
          <div
            id={`background-run-details-panel-${run.id}`}
            className="rounded-md border p-3"
            data-testid={`background-run-details-panel-${run.id}`}
          >
            {detailError ? (
              <Alert
                tone="error"
                title="Couldn't load the details"
                description={detailError}
                data-testid={`background-run-detail-error-${run.id}`}
              />
            ) : detail ? (
              <BackgroundRunDetailTabs run={run} detail={detail} />
            ) : (
              <Flex className="justify-center py-4">
                <Spin label="Loading details" />
              </Flex>
            )}
          </div>
        )}
      </Flex>
    </Card>
  )
}

/**
 * The Transcript + Result tab region for a terminal run's detail. Transcript is
 * the default tab (the primary job here); Result holds the final output. Both
 * reuse SHARED primitives — `AgentActivityTimeline` (`stepId="agent"`, the exact
 * projection the detail endpoint serves) and `BackgroundRunResult` — never a
 * bespoke re-implementation.
 */
export function BackgroundRunDetailTabs({
  run,
  detail,
}: {
  run: BackgroundRunSummary
  detail: BackgroundRunDetail
}) {
  const transcript: AgentActivityEntry[] = (detail.activity ?? []).filter(
    (e): e is AgentActivityEntry => e.type === 'agent_activity',
  )

  return (
    <Tabs
      data-testid={`background-run-detail-tabs-${run.id}`}
      defaultValue="transcript"
      variant="line"
      size="sm"
      items={[
        {
          key: 'transcript',
          label: 'Transcript',
          children:
            transcript.length > 0 ? (
              <div className="pt-2">
                <AgentActivityTimeline stepId="agent" entries={transcript} />
              </div>
            ) : (
              <Text
                type="secondary"
                className="block pt-2 text-sm"
                data-testid={`background-run-transcript-empty-${run.id}`}
              >
                No transcript was recorded for this task.
              </Text>
            ),
        },
        {
          key: 'result',
          label: 'Result',
          children: (
            <div className="pt-2">
              <BackgroundRunResult detail={detail} />
            </div>
          ),
        },
      ]}
    />
  )
}
