import { test } from 'node:test'
import assert from 'node:assert/strict'
import { encodeWav, resampleLinear } from './wav.ts'

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
