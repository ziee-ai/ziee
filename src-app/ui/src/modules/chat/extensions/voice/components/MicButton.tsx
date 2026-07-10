import { type ReactNode, useEffect, useState } from 'react'
import { Captions, CaptionsOff, Loader2, Mic, Square, X } from 'lucide-react'
import { Button, Popover, Tooltip } from '@/components/ui'
import { cn } from '@/lib/utils'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { isRecordingSupported } from '../Voice.store'

const PRIVACY_HINT_KEY = 'ziee.voice.privacyHintDismissed'
const NOT_READY_HELP_ID = 'voice-mic-not-ready-help'

function formatElapsed(ms: number): string {
  const total = Math.floor(ms / 1000)
  const m = Math.floor(total / 60)
  const s = total % 60
  return `${m}:${s.toString().padStart(2, '0')}`
}

/**
 * Composer mic button — voice dictation.
 *
 * Visibility / states:
 *   - HIDDEN when the feature is disabled, capability hasn't resolved, or the
 *     browser can't record (insecure context / no getUserMedia).
 *   - DISABLED-LOOKING (but keyboard-focusable, with an accessible remediation
 *     description) when enabled but the server isn't set up to transcribe yet.
 *   - Idle → click to record. Recording → pulsing dot + timer + Stop + Cancel.
 *     Requesting → spinner + Cancel (escape a hung permission prompt).
 *     Transcribing → spinner with the staged status text.
 *
 * A11y: a SINGLE persistent `aria-live` region (rendered in every visible
 * state) announces only DISCRETE transitions from `VoiceStore.announcement`
 * ("Recording started" / "Transcribing" / "Transcript added" / errors). The
 * per-second timer is intentionally kept OUT of any live region so screen
 * readers don't re-announce it every tick.
 */
export function MicButton() {
  const { status, elapsedMs, capability, capabilityLoaded, stageText, announcement, interimText, liveCaptions } =
    Stores.Chat.VoiceStore
  // PERMISSION gate (layer 4 — explicit, at the render site). Independent of the
  // feature/binary-availability gate below: a user whose group lacks
  // `voice::transcribe` sees NO mic affordance AT ALL — not even the muted
  // "not set up yet" state. Kept separate from the capability check so the two
  // hide the button for their own distinct reason (perm vs. not-provisioned),
  // and so a future default `capability` can never silently leak the button to
  // an unpermitted user. (The store ALSO skips the capability fetch when this
  // perm is absent, and the endpoint 403s — this is the explicit third layer.)
  const canDictate = usePermission(Permissions.VoiceTranscribe)
  const [hintOpen, setHintOpen] = useState(
    () => typeof localStorage !== 'undefined' && localStorage.getItem(PRIVACY_HINT_KEY) == null,
  )

  // Stop the recorder, release the MediaStream (mic off), and clear timers when
  // the composer unmounts mid-flow — otherwise navigating away leaves the mic
  // live until max_clip auto-stop fires and transcribes into a left conversation.
  useEffect(() => {
    return () => {
      Stores.Chat.VoiceStore.cancelRecording()
    }
  }, [])

  // PERMISSION gate first: no `voice::transcribe` → no affordance whatsoever.
  if (!canDictate) return null

  // Hidden: feature off, capability not yet known, or recording unsupported.
  if (!capabilityLoaded || !capability || !capability.enabled) return null
  if (!isRecordingSupported()) return null

  const notReady = !capability.can_transcribe
  const isRecording = status === 'recording'
  const isTranscribing = status === 'transcribing'
  const isRequesting = status === 'requesting'
  const isBusy = isRequesting || isTranscribing

  const dismissHint = () => {
    try {
      localStorage.setItem(PRIVACY_HINT_KEY, '1')
    } catch {
      /* storage blocked — dismiss for this session only */
    }
    setHintOpen(false)
  }

  // Live-captions availability + per-device toggle (only when the deployment
  // offers streaming captions). Idle-only affordance so the mode is chosen before
  // recording (the interim loop is armed at record start).
  const streamingAvailable = !!capability.streaming_enabled
  const liveToggle = streamingAvailable ? (
    <Tooltip content={liveCaptions ? 'Live captions on' : 'Live captions off'}>
      <Button
        data-testid="voice-live-toggle"
        data-tooltip-wrapped=""
        icon={
          liveCaptions ? (
            <Captions className="size-4" />
          ) : (
            <CaptionsOff className="size-4 text-muted-foreground" />
          )
        }
        variant="ghost"
        size="default"
        aria-label={liveCaptions ? 'Turn live captions off' : 'Turn live captions on'}
        aria-pressed={liveCaptions}
        onClick={() => Stores.Chat.VoiceStore.setLiveCaptions(!liveCaptions)}
      />
    </Tooltip>
  ) : null

  // The primary mic affordance (idle), reused by the plain + privacy-hint states.
  const micButtonTrigger = (
    <span className="inline-flex shrink-0">
      <Button
        data-testid="voice-mic-button"
        data-tooltip-wrapped=""
        icon={<Mic className={cn('size-4', status === 'error' && 'text-destructive')} />}
        variant="ghost"
        size="default"
        aria-label="Start voice dictation"
        aria-pressed={false}
        onClick={() => Stores.Chat.VoiceStore.startRecording()}
      />
    </span>
  )

  // Build the state-specific content, then render it BELOW one persistent live
  // region (see the single return). The live region must be a stable DOM node
  // across every transition — a region remounted with its text already present
  // is frequently NOT announced — so it lives at a fixed position above this
  // branch switch rather than inside each branch.
  let content: ReactNode
  if (notReady) {
    // Not-ready posture: a muted-but-FOCUSABLE button whose remediation is
    // exposed to AT via aria-describedby (not tooltip-only, which is unreachable
    // on a disabled/untabbable control).
    content = (
      <>
        <Tooltip content="Voice dictation isn't set up yet — contact an administrator">
          <span className="inline-flex shrink-0">
            <Button
              data-testid="voice-mic-button"
              data-tooltip-wrapped=""
              icon={<Mic className="size-4" />}
              variant="ghost"
              size="default"
              aria-disabled={true}
              aria-label="Voice dictation (unavailable)"
              aria-describedby={NOT_READY_HELP_ID}
              className="opacity-50"
              onClick={e => e.preventDefault()}
            />
          </span>
        </Tooltip>
        <span id={NOT_READY_HELP_ID} className="sr-only">
          Voice dictation isn't set up yet. Contact an administrator to install a
          speech runtime and model.
        </span>
      </>
    )
  } else if (isRecording) {
    // Recording: pulsing indicator + elapsed timer + Stop + Cancel. The visible
    // timer is aria-hidden — the live region carries the discrete announcements.
    content = (
      <div
        className="flex items-center gap-2"
        role="group"
        aria-label="Recording voice dictation"
      >
        {/* Live-caption preview (transient, visual-only — the persistent live
            region carries discrete announcements; a growing transcript must not
            be re-announced every tick). Never written to the composer. */}
        {liveCaptions && interimText && (
          <span
            data-testid="voice-live-caption"
            aria-hidden="true"
            className="text-xs text-muted-foreground truncate max-w-48"
          >
            {interimText}
          </span>
        )}
        <span
          className="size-2 rounded-full bg-destructive animate-pulse"
          aria-hidden="true"
        />
        <span
          className="text-xs tabular-nums text-muted-foreground min-w-9"
          data-testid="voice-elapsed"
          aria-hidden="true"
        >
          {formatElapsed(elapsedMs)}
        </span>
        <Tooltip content="Stop &amp; transcribe">
          <Button
            data-testid="voice-mic-button"
            data-tooltip-wrapped=""
            icon={<Square className="size-4" />}
            variant="ghost"
            size="default"
            aria-label="Stop recording and transcribe"
            aria-pressed={true}
            onClick={() => Stores.Chat.VoiceStore.stopRecording()}
          />
        </Tooltip>
        <Tooltip content="Cancel">
          <Button
            data-testid="voice-cancel-button"
            data-tooltip-wrapped=""
            icon={<X className="size-4" />}
            variant="ghost"
            size="default"
            aria-label="Cancel recording"
            onClick={() => Stores.Chat.VoiceStore.cancelRecording()}
          />
        </Tooltip>
      </div>
    )
  } else if (isBusy) {
    // Requesting / transcribing: spinner + staged status text. Requesting also
    // gets a Cancel button so a never-answered permission prompt is escapable.
    const label = isTranscribing ? stageText || 'Transcribing…' : 'Starting…'
    content = (
      <div className="flex items-center gap-1.5" data-testid="voice-transcribing">
        <Loader2 className="size-4 animate-spin text-muted-foreground" aria-hidden="true" />
        <span className="text-xs text-muted-foreground truncate max-w-40">{label}</span>
        {isRequesting && (
          <Tooltip content="Cancel">
            <Button
              data-testid="voice-cancel-button"
              data-tooltip-wrapped=""
              icon={<X className="size-4" />}
              variant="ghost"
              size="default"
              aria-label="Cancel microphone request"
              onClick={() => Stores.Chat.VoiceStore.cancelRecording()}
            />
          </Tooltip>
        )}
      </div>
    )
  } else if (hintOpen) {
    // Idle + one-time privacy hint (popover anchored to the button).
    content = (
      <Popover
        open
        onOpenChange={open => {
          if (!open) dismissHint()
        }}
        align="start"
        side="top"
        className="w-auto max-w-64"
        content={
          <div className="flex flex-col gap-2 p-1" data-testid="voice-privacy-hint">
            <p className="text-xs text-muted-foreground">
              Audio is transcribed locally on your server — never sent to the cloud.
            </p>
            <Button
              data-testid="voice-privacy-hint-dismiss"
              size="default"
              variant="outline"
              onClick={dismissHint}
            >
              Got it
            </Button>
          </div>
        }
      >
        {micButtonTrigger}
      </Popover>
    )
    content = (
      <div className="flex items-center gap-1">
        {content}
        {liveToggle}
      </div>
    )
  } else {
    // Idle (hint dismissed): plain mic affordance + the live-captions toggle.
    content = (
      <div className="flex items-center gap-1">
        <Tooltip content="Dictate a message">{micButtonTrigger}</Tooltip>
        {liveToggle}
      </div>
    )
  }

  // ONE persistent live region (stable position-0 node) + the state content.
  // Only `announcement`'s text changes across transitions; the node itself never
  // remounts, so "Recording started" / "Transcript added" / errors are announced.
  return (
    <>
      <span
        aria-live="polite"
        aria-atomic="true"
        className="sr-only"
        data-testid="voice-live-region"
      >
        {announcement}
      </span>
      {content}
    </>
  )
}
