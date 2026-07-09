import { useState } from 'react'
import { Loader2, Mic, Square, X } from 'lucide-react'
import { Button, Popover, Tooltip } from '@/components/ui'
import { cn } from '@/lib/utils'
import { Stores } from '@/core/stores'
import { isRecordingSupported } from '../Voice.store'

const PRIVACY_HINT_KEY = 'ziee.voice.privacyHintDismissed'

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
 *   - DISABLED (with an explanatory tooltip) when enabled but the server isn't
 *     set up to transcribe yet (no runtime / model).
 *   - Idle → click to record. Recording → pulsing dot + timer + Stop + Cancel.
 *     Transcribing → spinner with the staged status text.
 */
export function MicButton() {
  const { status, elapsedMs, capability, capabilityLoaded, stageText } =
    Stores.Chat.VoiceStore
  const [hintOpen, setHintOpen] = useState(
    () => typeof localStorage !== 'undefined' && localStorage.getItem(PRIVACY_HINT_KEY) == null,
  )

  // Hidden: feature off, capability not yet known, or recording unsupported.
  if (!capabilityLoaded || !capability || !capability.enabled) return null
  if (!isRecordingSupported()) return null

  const notReady = !capability.can_transcribe
  const isRecording = status === 'recording'
  const isTranscribing = status === 'transcribing'
  const isBusy = status === 'requesting' || isTranscribing

  const dismissHint = () => {
    try {
      localStorage.setItem(PRIVACY_HINT_KEY, '1')
    } catch {
      /* storage blocked — dismiss for this session only */
    }
    setHintOpen(false)
  }

  // Not-ready posture: a single disabled button with a guiding tooltip.
  if (notReady) {
    return (
      <Tooltip content="Voice dictation isn't set up yet — contact an administrator">
        <span className="inline-flex shrink-0">
          <Button
            data-testid="voice-mic-button"
            data-tooltip-wrapped=""
            icon={<Mic className="size-4" />}
            variant="ghost"
            size="default"
            disabled
            aria-label="Voice dictation (unavailable)"
          />
        </span>
      </Tooltip>
    )
  }

  // Recording: pulsing indicator + elapsed timer + Stop + Cancel.
  if (isRecording) {
    return (
      <div
        className="flex items-center gap-1"
        role="group"
        aria-label="Recording voice dictation"
      >
        <span
          className="size-2 rounded-full bg-destructive animate-pulse"
          aria-hidden="true"
        />
        <span
          className="text-xs tabular-nums text-muted-foreground min-w-9"
          data-testid="voice-elapsed"
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
        <span aria-live="polite" className="sr-only">
          Recording, {formatElapsed(elapsedMs)}
        </span>
      </div>
    )
  }

  // Transcribing / requesting: spinner + staged status text.
  if (isBusy) {
    const label = isTranscribing ? stageText || 'Transcribing…' : 'Starting…'
    return (
      <div className="flex items-center gap-1.5" data-testid="voice-transcribing">
        <Loader2 className="size-4 animate-spin text-muted-foreground" aria-hidden="true" />
        <span className="text-xs text-muted-foreground truncate max-w-40">{label}</span>
        <span aria-live="polite" className="sr-only">
          {label}
        </span>
      </div>
    )
  }

  // Idle: the primary mic affordance. A one-time privacy hint is shown as a
  // popover anchored to the button; once dismissed, a plain tooltip replaces it.
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

  if (hintOpen) {
    return (
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
  }

  return <Tooltip content="Dictate a message">{micButtonTrigger}</Tooltip>
}
