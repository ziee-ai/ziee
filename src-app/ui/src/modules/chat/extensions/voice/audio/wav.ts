/**
 * Pure audio helpers for the voice-dictation composer.
 *
 * The whisper backend expects 16 kHz mono 16-bit PCM WAV. MediaRecorder gives
 * us an opaque compressed blob (webm/ogg/mp4 depending on browser), so we
 * decode it via the WebAudio `AudioContext`, downmix to mono, linearly resample
 * to 16 kHz, and re-encode a canonical RIFF/WAVE container.
 *
 * `encodeWav` + `resampleLinear` are pure (no DOM) so they're directly
 * unit-testable; `recordedBlobToWav16k` is the browser-only orchestration.
 */

/** Target sample rate the whisper server expects. */
export const TARGET_SAMPLE_RATE = 16000

function writeAscii(view: DataView, offset: number, text: string): void {
  for (let i = 0; i < text.length; i++) {
    view.setUint8(offset + i, text.charCodeAt(i))
  }
}

/**
 * Encode mono float32 PCM samples (each in [-1, 1]) as a 16-bit PCM WAV Blob.
 * The header is a canonical 44-byte RIFF/WAVE / `fmt `(PCM) / `data` layout.
 */
export function encodeWav(samples: Float32Array, sampleRate: number): Blob {
  const numChannels = 1
  const bytesPerSample = 2
  const blockAlign = numChannels * bytesPerSample
  const byteRate = sampleRate * blockAlign
  const dataLength = samples.length * bytesPerSample
  const buffer = new ArrayBuffer(44 + dataLength)
  const view = new DataView(buffer)

  // RIFF chunk descriptor
  writeAscii(view, 0, 'RIFF')
  view.setUint32(4, 36 + dataLength, true)
  writeAscii(view, 8, 'WAVE')

  // fmt sub-chunk
  writeAscii(view, 12, 'fmt ')
  view.setUint32(16, 16, true) // sub-chunk size (16 for PCM)
  view.setUint16(20, 1, true) // audio format = 1 (PCM)
  view.setUint16(22, numChannels, true)
  view.setUint32(24, sampleRate, true)
  view.setUint32(28, byteRate, true)
  view.setUint16(32, blockAlign, true)
  view.setUint16(34, 16, true) // bits per sample

  // data sub-chunk
  writeAscii(view, 36, 'data')
  view.setUint32(40, dataLength, true)

  let offset = 44
  for (let i = 0; i < samples.length; i++) {
    let s = Math.max(-1, Math.min(1, samples[i]))
    s = s < 0 ? s * 0x8000 : s * 0x7fff
    view.setInt16(offset, s, true)
    offset += 2
  }

  return new Blob([view], { type: 'audio/wav' })
}

/**
 * Linear-interpolation resample of a mono float32 buffer from `inRate` to
 * `outRate`. Output length is `round(input.length * outRate / inRate)`.
 */
export function resampleLinear(
  input: Float32Array,
  inRate: number,
  outRate: number,
): Float32Array {
  if (inRate === outRate || input.length === 0) return input
  const ratio = outRate / inRate
  const outLength = Math.round(input.length * ratio)
  const output = new Float32Array(outLength)
  for (let i = 0; i < outLength; i++) {
    const srcPos = i / ratio
    const i0 = Math.floor(srcPos)
    const i1 = Math.min(i0 + 1, input.length - 1)
    const frac = srcPos - i0
    output[i] = input[i0] * (1 - frac) + input[i1] * frac
  }
  return output
}

/** Average all channels of an AudioBuffer into a single mono Float32Array. */
export function downmixToMono(audioBuffer: AudioBuffer): Float32Array {
  const channels = audioBuffer.numberOfChannels
  const length = audioBuffer.length
  if (channels === 1) return audioBuffer.getChannelData(0)
  const mono = new Float32Array(length)
  for (let c = 0; c < channels; c++) {
    const data = audioBuffer.getChannelData(c)
    for (let i = 0; i < length; i++) mono[i] += data[i]
  }
  for (let i = 0; i < length; i++) mono[i] /= channels
  return mono
}

/**
 * Decode a recorder Blob (any browser-native codec), downmix to mono, resample
 * to 16 kHz, and re-encode as a 16-bit PCM WAV Blob — the format the whisper
 * transcription endpoint accepts.
 */
export async function recordedBlobToWav16k(blob: Blob): Promise<Blob> {
  const arrayBuffer = await blob.arrayBuffer()
  const AudioCtx: typeof AudioContext =
    window.AudioContext ||
    (window as unknown as { webkitAudioContext: typeof AudioContext })
      .webkitAudioContext
  const ctx = new AudioCtx()
  try {
    const audioBuffer = await ctx.decodeAudioData(arrayBuffer)
    const mono = downmixToMono(audioBuffer)
    const resampled = resampleLinear(
      mono,
      audioBuffer.sampleRate,
      TARGET_SAMPLE_RATE,
    )
    return encodeWav(resampled, TARGET_SAMPLE_RATE)
  } finally {
    void ctx.close()
  }
}
