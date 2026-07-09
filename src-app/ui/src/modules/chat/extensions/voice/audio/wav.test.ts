import { test } from 'node:test'
import assert from 'node:assert/strict'
import { downmixToMono, encodeWav, resampleLinear } from './wav.ts'

/**
 * Minimal AudioBuffer stand-in — downmixToMono only touches numberOfChannels,
 * length, and getChannelData(c), so we don't need a real WebAudio buffer.
 */
function fakeAudioBuffer(channels: Float32Array[]): AudioBuffer {
  return {
    numberOfChannels: channels.length,
    length: channels[0]?.length ?? 0,
    getChannelData: (c: number) => channels[c],
  } as unknown as AudioBuffer
}

// Read the little-endian header fields back out of the encoded WAV Blob.
async function headerOf(blob: Blob) {
  const buf = await blob.arrayBuffer()
  const view = new DataView(buf)
  const ascii = (offset: number, len: number) =>
    String.fromCharCode(
      ...Array.from({ length: len }, (_, i) => view.getUint8(offset + i)),
    )
  return {
    byteLength: buf.byteLength,
    riff: ascii(0, 4),
    wave: ascii(8, 4),
    fmt: ascii(12, 4),
    audioFormat: view.getUint16(20, true),
    numChannels: view.getUint16(22, true),
    sampleRate: view.getUint32(24, true),
    byteRate: view.getUint32(28, true),
    blockAlign: view.getUint16(32, true),
    bitsPerSample: view.getUint16(34, true),
    dataTag: ascii(36, 4),
    dataLength: view.getUint32(40, true),
  }
}

test('encodeWav writes a canonical 16 kHz mono 16-bit PCM header', async () => {
  const samples = new Float32Array([0, 0.5, -0.5, 1, -1])
  const blob = await encodeWav(samples, 16000)
  const h = await headerOf(blob)

  assert.equal(h.riff, 'RIFF')
  assert.equal(h.wave, 'WAVE')
  assert.equal(h.fmt, 'fmt ')
  assert.equal(h.audioFormat, 1, 'PCM')
  assert.equal(h.numChannels, 1, 'mono')
  assert.equal(h.sampleRate, 16000, '16 kHz')
  assert.equal(h.bitsPerSample, 16, '16-bit')
  assert.equal(h.blockAlign, 2, 'mono * 2 bytes')
  assert.equal(h.byteRate, 16000 * 2, 'sampleRate * blockAlign')
  assert.equal(h.dataTag, 'data')
  // 5 samples * 2 bytes = 10 bytes of PCM data; 44-byte header.
  assert.equal(h.dataLength, samples.length * 2)
  assert.equal(h.byteLength, 44 + samples.length * 2)
})

test('encodeWav clamps samples to the int16 range', async () => {
  const blob = await encodeWav(new Float32Array([2, -2]), 16000)
  const buf = await blob.arrayBuffer()
  const view = new DataView(buf)
  assert.equal(view.getInt16(44, true), 0x7fff, '+full-scale clamps to +32767')
  assert.equal(view.getInt16(46, true), -0x8000, '-full-scale clamps to -32768')
})

test('resampleLinear downsamples 48 kHz -> 16 kHz to a third of the samples', () => {
  const input = new Float32Array(4800) // 0.1s @ 48 kHz
  for (let i = 0; i < input.length; i++) input[i] = Math.sin(i / 10)
  const out = resampleLinear(input, 48000, 16000)
  // round(4800 * 16000/48000) = 1600
  assert.equal(out.length, 1600)
})

test('resampleLinear is a no-op when rates match', () => {
  const input = new Float32Array([0.1, 0.2, 0.3])
  assert.strictEqual(resampleLinear(input, 16000, 16000), input)
})

test('resampleLinear upsamples 8 kHz -> 16 kHz to double the samples', () => {
  const input = new Float32Array(800)
  const out = resampleLinear(input, 8000, 16000)
  assert.equal(out.length, 1600)
})

test('resampleLinear linearly interpolates midpoints when 2x upsampling', () => {
  // A non-zero ramp so wrong interpolation is observable in the VALUES, not
  // just the length. 2x upsample places a true midpoint between each pair.
  const input = new Float32Array([0, 1, 2, 3])
  const out = resampleLinear(input, 1, 2)
  // round(4 * 2) = 8 samples; last index clamps to the final input sample.
  assert.equal(out.length, 8)
  const expected = [0, 0.5, 1, 1.5, 2, 2.5, 3, 3]
  for (let i = 0; i < expected.length; i++) {
    assert.ok(
      Math.abs(out[i] - expected[i]) < 1e-6,
      `sample ${i}: got ${out[i]}, expected ${expected[i]}`,
    )
  }
})

test('resampleLinear interpolates on downsample (3 -> 2)', () => {
  // ratio = 2/3; srcPos for i=1 is 1.5 → midpoint of input[1] & input[2].
  const input = new Float32Array([0, 10, 20])
  const out = resampleLinear(input, 3, 2)
  assert.equal(out.length, 2) // round(3 * 2/3) = 2
  assert.ok(Math.abs(out[0] - 0) < 1e-6, `sample 0: got ${out[0]}`)
  assert.ok(Math.abs(out[1] - 15) < 1e-6, `sample 1: got ${out[1]}`) // 10*.5 + 20*.5
})

test('downmixToMono averages all channels sample-by-sample', () => {
  const left = new Float32Array([0, 1, 0.5])
  const right = new Float32Array([1, 0, -0.5])
  const mono = downmixToMono(fakeAudioBuffer([left, right]))
  const expected = [0.5, 0.5, 0] // per-sample average of L and R
  assert.equal(mono.length, expected.length)
  for (let i = 0; i < expected.length; i++) {
    assert.ok(
      Math.abs(mono[i] - expected[i]) < 1e-6,
      `sample ${i}: got ${mono[i]}, expected ${expected[i]}`,
    )
  }
})

test('downmixToMono returns the sole channel unchanged for mono input', () => {
  const only = new Float32Array([0.2, -0.4, 0.8])
  const mono = downmixToMono(fakeAudioBuffer([only]))
  assert.strictEqual(mono, only, 'mono input is returned as-is (no copy/averaging)')
})
