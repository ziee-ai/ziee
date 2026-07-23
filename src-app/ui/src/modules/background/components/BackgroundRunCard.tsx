import { ExternalLink, FileText, MessageSquare, XCircle } from 'lucide-react'
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'

import type { BackgroundRunSummary } from '@/api-client/types'
import {
  Alert,
  Button,
  Card,
  Confirm,
  Flex,
  message,
  Spin,
  Tag,
  type TagTone,
  Text,
  Textarea,
} from '@ziee/kit'

import { isTerminalRunStatus } from '../stores/BackgroundRuns.store'
import { BackgroundRunResult } from './BackgroundRunResult'
import { BackgroundRuns } from '@/modules/background/stores/BackgroundRuns.store'

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
 * One background-run row (ITEM-8 / ITEM-25). Shows the run's status badge, label,
 * kind, relative start time and a "result ready" indicator; lets the user cancel
 * a non-terminal run (confirmed), queue a steering note to it, and jump to the
 * conversation the result landed in.
 *
 * Cancel + steer are gated on `!isTerminalRunStatus(run.status)` — the exact
 * boundary the backend enforces (both endpoints 409 on a terminal run).
 */
export function BackgroundRunCard({ run }: { run: BackgroundRunSummary }) {
  const navigate = useNavigate()
  const [cancelOpen, setCancelOpen] = useState(false)
  const [cancelling, setCancelling] = useState(false)
  const [steerOpen, setSteerOpen] = useState(false)
  const [note, setNote] = useState('')
  const [posting, setPosting] = useState(false)
  const [resultOpen, setResultOpen] = useState(false)

  const terminal = isTerminalRunStatus(run.status)
  // Reactive read (subscribes) — the row re-renders when its notes load / change.
  const notes = BackgroundRuns.notesByRun[run.id] ?? []
  const pendingNotes = notes.filter(n => !n.consumed_at)

  // Reactive reads for the inline result view (subscribe → re-render on fetch).
  // `runDetailLoading` is the store's internal fetch-dedup guard; here the panel
  // derives its loading state from `detail` being absent (no error) instead.
  const detail = BackgroundRuns.detailsByRun[run.id]
  const detailError = BackgroundRuns.detailErrorByRun[run.id]

  const toggleResult = () => {
    setResultOpen(open => {
      const next = !open
      // Lazy-fetch the result body only when the view is first opened; the store
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

  return (
    <Card data-testid={`background-run-card-${run.id}`}>
      <Flex className="flex-col gap-2">
        {/* Status + label */}
        <Flex className="flex-wrap items-center gap-2">
          <Tag
            variant="outline"
            tone={STATUS_TONE[run.status] ?? 'default'}
            data-testid={`background-run-status-${run.id}`}
          >
            {run.status}
          </Tag>
          <Text strong className="min-w-0 truncate">
            {run.label ?? 'Untitled run'}
          </Text>
        </Flex>

        {/* Kind + start time + result indicator */}
        <Flex className="flex-wrap items-center gap-2">
          <Tag variant="outline" data-testid={`background-run-kind-${run.id}`}>
            {KIND_LABEL[run.job_kind] ?? run.job_kind}
          </Tag>
          <Text type="secondary" className="text-xs">
            {relativeTime(run.created_at)}
          </Text>
          {run.has_result && (
            <Tag
              variant="outline"
              tone="success"
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
          {run.conversation_id && (
            <Button
              variant="link"
              icon={<ExternalLink />}
              data-testid={`background-run-open-${run.id}`}
              onClick={() => navigate(`/chat/${run.conversation_id}`)}
            >
              Open conversation
            </Button>
          )}
          {terminal && (
            <Button
              variant="ghost"
              icon={<FileText />}
              aria-expanded={resultOpen}
              aria-controls={`background-run-result-panel-${run.id}`}
              aria-label={resultOpen ? 'Hide result' : 'View result'}
              data-testid={`background-run-result-toggle-${run.id}`}
              onClick={toggleResult}
            >
              {resultOpen ? 'Hide result' : 'View result'}
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

        {/* Inline result view (terminal runs only) — lazily fetched on expand. */}
        {terminal && resultOpen && (
          <Flex
            id={`background-run-result-panel-${run.id}`}
            className="flex-col gap-2 rounded-md border p-3"
            data-testid={`background-run-result-panel-${run.id}`}
          >
            {detailError ? (
              <Alert
                tone="error"
                title="Couldn't load the result"
                description={detailError}
                data-testid={`background-run-result-error-${run.id}`}
              />
            ) : detail ? (
              <BackgroundRunResult detail={detail} />
            ) : (
              <Flex className="justify-center py-4">
                <Spin label="Loading result" />
              </Flex>
            )}
          </Flex>
        )}
      </Flex>
    </Card>
  )
}
