#!/usr/bin/env node
/**
 * Generate the binary + text assets referenced by showcase.sql.
 *
 * Node port of the former generate_files.py (the repo's only Python dep). Every
 * file has a FIXED, deterministic filename that matches a `files` row in
 * showcase.sql. Re-running is idempotent — it just overwrites the bytes, and
 * the output is deterministic (no timestamps / randomness), so a regenerate
 * never produces a spurious diff.
 *
 * Deps (devDependencies of the ui workspace, hoisted to the repo-root
 * node_modules so this resolves from anywhere in the tree):
 *   - pngjs    → chart.png
 *   - exceljs  → workbook.xlsx
 * The JPEG (photo.jpg), PDF (report.pdf), CSV, and text files are written by
 * hand with a small baseline-JPEG encoder + literal strings — no extra deps.
 *
 * To add a NEW file case:
 *   1. add a generator function below + call it in main(),
 *   2. add a matching `files` row + content block in showcase.sql,
 *   3. map its <file_id>.<ext> in load.sh's FILE_MAP.
 */
import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { PNG } from 'pngjs'
import ExcelJS from 'exceljs'

const HERE = dirname(fileURLToPath(import.meta.url))
const OUT = join(HERE, 'files')
mkdirSync(OUT, { recursive: true })
const out = (name) => join(OUT, name)

// ---------------------------------------------------------------------------
// Tiny 5x7 bitmap font — enough to label the chart (uppercased) + bar values.
// Unmapped characters render as blank (a space-width gap). '#' = on pixel.
// ---------------------------------------------------------------------------
const FONT = {
  ' ': ['.....', '.....', '.....', '.....', '.....', '.....', '.....'],
  A: ['.###.', '#...#', '#...#', '#####', '#...#', '#...#', '#...#'],
  B: ['####.', '#...#', '#...#', '####.', '#...#', '#...#', '####.'],
  C: ['.####', '#....', '#....', '#....', '#....', '#....', '.####'],
  D: ['####.', '#...#', '#...#', '#...#', '#...#', '#...#', '####.'],
  E: ['#####', '#....', '#....', '####.', '#....', '#....', '#####'],
  F: ['#####', '#....', '#....', '####.', '#....', '#....', '#....'],
  G: ['.####', '#....', '#....', '#.###', '#...#', '#...#', '.####'],
  H: ['#...#', '#...#', '#...#', '#####', '#...#', '#...#', '#...#'],
  I: ['#####', '..#..', '..#..', '..#..', '..#..', '..#..', '#####'],
  J: ['..###', '...#.', '...#.', '...#.', '#..#.', '#..#.', '.##..'],
  K: ['#...#', '#..#.', '#.#..', '##...', '#.#..', '#..#.', '#...#'],
  L: ['#....', '#....', '#....', '#....', '#....', '#....', '#####'],
  M: ['#...#', '##.##', '#.#.#', '#.#.#', '#...#', '#...#', '#...#'],
  N: ['#...#', '##..#', '#.#.#', '#.#.#', '#..##', '#...#', '#...#'],
  O: ['.###.', '#...#', '#...#', '#...#', '#...#', '#...#', '.###.'],
  P: ['####.', '#...#', '#...#', '####.', '#....', '#....', '#....'],
  Q: ['.###.', '#...#', '#...#', '#...#', '#.#.#', '#..#.', '.##.#'],
  R: ['####.', '#...#', '#...#', '####.', '#.#..', '#..#.', '#...#'],
  S: ['.####', '#....', '#....', '.###.', '....#', '....#', '####.'],
  T: ['#####', '..#..', '..#..', '..#..', '..#..', '..#..', '..#..'],
  U: ['#...#', '#...#', '#...#', '#...#', '#...#', '#...#', '.###.'],
  V: ['#...#', '#...#', '#...#', '#...#', '#...#', '.#.#.', '..#..'],
  W: ['#...#', '#...#', '#...#', '#.#.#', '#.#.#', '##.##', '#...#'],
  X: ['#...#', '#...#', '.#.#.', '..#..', '.#.#.', '#...#', '#...#'],
  Y: ['#...#', '#...#', '.#.#.', '..#..', '..#..', '..#..', '..#..'],
  Z: ['#####', '....#', '...#.', '..#..', '.#...', '#....', '#####'],
  0: ['.###.', '#...#', '#..##', '#.#.#', '##..#', '#...#', '.###.'],
  1: ['..#..', '.##..', '..#..', '..#..', '..#..', '..#..', '#####'],
  2: ['.###.', '#...#', '....#', '..##.', '.#...', '#....', '#####'],
  3: ['#####', '...#.', '..#..', '...#.', '....#', '#...#', '.###.'],
  4: ['...#.', '..##.', '.#.#.', '#..#.', '#####', '...#.', '...#.'],
  5: ['#####', '#....', '####.', '....#', '....#', '#...#', '.###.'],
  6: ['.###.', '#....', '#....', '####.', '#...#', '#...#', '.###.'],
  7: ['#####', '....#', '...#.', '..#..', '.#...', '.#...', '.#...'],
  8: ['.###.', '#...#', '#...#', '.###.', '#...#', '#...#', '.###.'],
  9: ['.###.', '#...#', '#...#', '.####', '....#', '....#', '.###.'],
  '.': ['.....', '.....', '.....', '.....', '.....', '.##..', '.##..'],
  '(': ['..#..', '.#...', '#....', '#....', '#....', '.#...', '..#..'],
  ')': ['..#..', '...#.', '....#', '....#', '....#', '...#.', '..#..'],
  '%': ['##..#', '##.#.', '..#..', '.#...', '#..##', '..#.#', '.#..#'],
  '+': ['.....', '..#..', '..#..', '#####', '..#..', '..#..', '.....'],
  '-': ['.....', '.....', '.....', '#####', '.....', '.....', '.....'],
}

// ---------------------------------------------------------------------------
// chart.png — a small bar chart drawn into an RGBA buffer via pngjs (stands in
// for a code_sandbox-generated matplotlib artifact returned via resource_link).
// ---------------------------------------------------------------------------
function hex(c) {
  const n = parseInt(c.slice(1), 16)
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255]
}

function genChartPng() {
  const W = 640
  const H = 400
  const png = new PNG({ width: W, height: H })
  const data = png.data
  const setPx = (x, y, [r, g, b]) => {
    if (x < 0 || x >= W || y < 0 || y >= H) return
    const i = (y * W + x) * 4
    data[i] = r
    data[i + 1] = g
    data[i + 2] = b
    data[i + 3] = 255
  }
  const fillRect = (x0, y0, x1, y1, rgb) => {
    for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) setPx(x, y, rgb)
  }
  const drawText = (text, x, y, rgb, scale = 1) => {
    let cx = x
    for (const chRaw of text.toUpperCase()) {
      const glyph = FONT[chRaw] || FONT[' ']
      for (let ry = 0; ry < 7; ry++) {
        for (let rx = 0; rx < 5; rx++) {
          if (glyph[ry][rx] === '#') {
            for (let sy = 0; sy < scale; sy++)
              for (let sx = 0; sx < scale; sx++)
                setPx(cx + rx * scale + sx, y + ry * scale + sy, rgb)
          }
        }
      }
      cx += 6 * scale
    }
  }

  fillRect(0, 0, W, H, hex('#0f172a')) // dark slate background
  drawText('Sales by Quarter (showcase chart.png)', 20, 16, hex('#e2e8f0'))

  const bars = [
    ['Q1', 120, '#38bdf8'],
    ['Q2', 200, '#34d399'],
    ['Q3', 160, '#fbbf24'],
    ['Q4', 280, '#f472b6'],
  ]
  const baseY = 360
  let x = 80
  for (const [label, val, color] of bars) {
    fillRect(x, baseY - val, x + 90, baseY, hex(color))
    drawText(label, x + 30, baseY + 8, hex('#cbd5e1'))
    drawText(String(val), x + 20, baseY - val - 16, hex('#cbd5e1'))
    x += 130
  }
  writeFileSync(out('chart.png'), PNG.sync.write(png))
}

// ---------------------------------------------------------------------------
// photo.jpg — a JPEG gradient (exercises the image renderer with a lossy type).
// Hand-written baseline (sequential, 4:4:4) JPEG encoder — no image deps.
// ---------------------------------------------------------------------------
const ZIGZAG = [
  0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40,
  48, 41, 34, 27, 20, 13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29,
  22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54,
  47, 55, 62, 63,
]
// Annex K.1 / K.2 base quantization tables (natural order).
const STD_LUM_Q = [
  16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16,
  24, 40, 57, 69, 56, 14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109,
  103, 77, 24, 35, 55, 64, 81, 104, 113, 92, 49, 64, 78, 87, 103, 121, 120,
  101, 72, 92, 95, 98, 112, 100, 103, 99,
]
const STD_CHR_Q = [
  17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56,
  99, 99, 99, 99, 99, 47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
  99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
  99, 99, 99, 99, 99, 99, 99,
]
// Annex K.3 standard Huffman table specs: 16 length-counts + value list.
const DC_LUM_BITS = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0]
const DC_CHR_BITS = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0]
const DC_VALS = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
const AC_LUM_BITS = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d]
const AC_LUM_VALS = [
  0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13,
  0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42,
  0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a,
  0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x34, 0x35,
  0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a,
  0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67,
  0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84,
  0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98,
  0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3,
  0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7,
  0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1,
  0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4,
  0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa,
]
const AC_CHR_BITS = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77]
const AC_CHR_VALS = [
  0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51,
  0x07, 0x61, 0x71, 0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1,
  0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0, 0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24,
  0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26, 0x27, 0x28, 0x29, 0x2a,
  0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
  0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66,
  0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82,
  0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96,
  0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa,
  0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
  0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9,
  0xda, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4,
  0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa,
]

function buildHuff(bits, vals) {
  const table = {}
  let code = 0
  let k = 0
  for (let len = 1; len <= 16; len++) {
    for (let i = 0; i < bits[len - 1]; i++) {
      table[vals[k]] = { code, length: len }
      k++
      code++
    }
    code <<= 1
  }
  return table
}

function scaleQuant(base, quality) {
  const q = quality < 50 ? 5000 / quality : 200 - quality * 2
  return base.map((v) => Math.min(255, Math.max(1, Math.floor((v * q + 50) / 100))))
}

// Precompute the separable DCT cosine matrix: COS[x][u] = cos((2x+1)uπ/16).
const COS = []
for (let x = 0; x < 8; x++) {
  COS[x] = []
  for (let u = 0; u < 8; u++) COS[x][u] = Math.cos(((2 * x + 1) * u * Math.PI) / 16)
}
const INV_SQRT2 = 1 / Math.SQRT2

function fdct(block) {
  const res = new Float64Array(64)
  for (let v = 0; v < 8; v++) {
    for (let u = 0; u < 8; u++) {
      let sum = 0
      for (let y = 0; y < 8; y++)
        for (let x = 0; x < 8; x++) sum += block[y * 8 + x] * COS[x][u] * COS[y][v]
      const cu = u === 0 ? INV_SQRT2 : 1
      const cv = v === 0 ? INV_SQRT2 : 1
      res[v * 8 + u] = 0.25 * cu * cv * sum
    }
  }
  return res
}

function encodeJPEG(W, H, rgb, quality = 85) {
  const lumQ = scaleQuant(STD_LUM_Q, quality)
  const chrQ = scaleQuant(STD_CHR_Q, quality)
  const dcLum = buildHuff(DC_LUM_BITS, DC_VALS)
  const acLum = buildHuff(AC_LUM_BITS, AC_LUM_VALS)
  const dcChr = buildHuff(DC_CHR_BITS, DC_VALS)
  const acChr = buildHuff(AC_CHR_BITS, AC_CHR_VALS)

  // RGB → YCbCr planes (level-shifted by -128 for all three).
  const Y = new Float64Array(W * H)
  const Cb = new Float64Array(W * H)
  const Cr = new Float64Array(W * H)
  for (let i = 0; i < W * H; i++) {
    const r = rgb[i * 3]
    const g = rgb[i * 3 + 1]
    const b = rgb[i * 3 + 2]
    Y[i] = 0.299 * r + 0.587 * g + 0.114 * b - 128
    Cb[i] = -0.168736 * r - 0.331264 * g + 0.5 * b
    Cr[i] = 0.5 * r - 0.418688 * g - 0.081312 * b
  }

  // Entropy bit writer with 0xFF byte-stuffing.
  const bytes = []
  let acc = 0
  let nbits = 0
  const emitByte = (bt) => {
    bytes.push(bt & 0xff)
    if ((bt & 0xff) === 0xff) bytes.push(0x00)
  }
  const writeBits = (code, length) => {
    for (let i = length - 1; i >= 0; i--) {
      acc = (acc << 1) | ((code >> i) & 1)
      nbits++
      if (nbits === 8) {
        emitByte(acc)
        acc = 0
        nbits = 0
      }
    }
  }
  const flushBits = () => {
    while (nbits > 0) {
      acc = (acc << 1) | 1
      nbits++
      if (nbits === 8) {
        emitByte(acc)
        acc = 0
        nbits = 0
      }
    }
  }
  const bitCode = (value) => {
    let m = value < 0 ? -value : value
    let size = 0
    while (m) {
      size++
      m >>= 1
    }
    const bits = value < 0 ? value + (1 << size) - 1 : value
    return [bits & ((1 << size) - 1), size]
  }

  const block = new Float64Array(64)
  const encodeBlock = (plane, bx, by, quant, dcHuff, acHuff, prevDC) => {
    for (let y = 0; y < 8; y++)
      for (let x = 0; x < 8; x++) block[y * 8 + x] = plane[(by + y) * W + (bx + x)]
    const dct = fdct(block)
    const quantized = new Int32Array(64)
    for (let i = 0; i < 64; i++) quantized[i] = Math.round(dct[i] / quant[i])
    // DC
    const diff = quantized[0] - prevDC
    const [dbits, dsize] = bitCode(diff)
    writeBits(dcHuff[dsize].code, dcHuff[dsize].length)
    if (dsize > 0) writeBits(dbits, dsize)
    // AC
    let run = 0
    for (let k = 1; k < 64; k++) {
      const coef = quantized[ZIGZAG[k]]
      if (coef === 0) {
        run++
        continue
      }
      while (run > 15) {
        writeBits(acHuff[0xf0].code, acHuff[0xf0].length)
        run -= 16
      }
      const [abits, asize] = bitCode(coef)
      const rs = (run << 4) | asize
      writeBits(acHuff[rs].code, acHuff[rs].length)
      writeBits(abits, asize)
      run = 0
    }
    if (run > 0) writeBits(acHuff[0x00].code, acHuff[0x00].length) // EOB
    return quantized[0]
  }

  let dcY = 0
  let dcCb = 0
  let dcCr = 0
  for (let by = 0; by < H; by += 8) {
    for (let bx = 0; bx < W; bx += 8) {
      dcY = encodeBlock(Y, bx, by, lumQ, dcLum, acLum, dcY)
      dcCb = encodeBlock(Cb, bx, by, chrQ, dcChr, acChr, dcCb)
      dcCr = encodeBlock(Cr, bx, by, chrQ, dcChr, acChr, dcCr)
    }
  }
  flushBits()

  // Assemble the JFIF stream.
  const head = []
  const u16 = (v) => head.push((v >> 8) & 0xff, v & 0xff)
  head.push(0xff, 0xd8) // SOI
  // APP0 / JFIF
  head.push(0xff, 0xe0)
  u16(16)
  head.push(0x4a, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00)
  u16(1)
  u16(1)
  head.push(0x00, 0x00)
  // DQT (luminance, id 0) — written in zigzag order
  head.push(0xff, 0xdb)
  u16(67)
  head.push(0x00)
  for (let k = 0; k < 64; k++) head.push(lumQ[ZIGZAG[k]])
  // DQT (chrominance, id 1)
  head.push(0xff, 0xdb)
  u16(67)
  head.push(0x01)
  for (let k = 0; k < 64; k++) head.push(chrQ[ZIGZAG[k]])
  // SOF0 (baseline)
  head.push(0xff, 0xc0)
  u16(17)
  head.push(8)
  u16(H)
  u16(W)
  head.push(3)
  head.push(1, 0x11, 0) // Y
  head.push(2, 0x11, 1) // Cb
  head.push(3, 0x11, 1) // Cr
  const writeDHT = (cls, id, bits, vals) => {
    head.push(0xff, 0xc4)
    u16(2 + 1 + 16 + vals.length)
    head.push((cls << 4) | id)
    for (const b of bits) head.push(b)
    for (const v of vals) head.push(v)
  }
  writeDHT(0, 0, DC_LUM_BITS, DC_VALS)
  writeDHT(1, 0, AC_LUM_BITS, AC_LUM_VALS)
  writeDHT(0, 1, DC_CHR_BITS, DC_VALS)
  writeDHT(1, 1, AC_CHR_BITS, AC_CHR_VALS)
  // SOS
  head.push(0xff, 0xda)
  u16(12)
  head.push(3)
  head.push(1, 0x00) // Y: DC0/AC0
  head.push(2, 0x11) // Cb: DC1/AC1
  head.push(3, 0x11) // Cr: DC1/AC1
  head.push(0x00, 0x3f, 0x00)

  return Buffer.concat([Buffer.from(head), Buffer.from(bytes), Buffer.from([0xff, 0xd9])])
}

function genPhotoJpg() {
  const W = 480
  const H = 320
  const rgb = new Uint8Array(W * H * 3)
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      const i = (y * W + x) * 3
      rgb[i] = Math.floor((255 * x) / W)
      rgb[i + 1] = Math.floor((255 * y) / H)
      rgb[i + 2] = 128
    }
  }
  writeFileSync(out('photo.jpg'), encodeJPEG(W, H, rgb, 85))
}

// ---------------------------------------------------------------------------
// workbook.xlsx — 3 sheets, to exercise the XlsxBody multi-sheet Tabs renderer.
// created/modified pinned to a constant so output is byte-deterministic.
// ---------------------------------------------------------------------------
async function genWorkbookXlsx() {
  const wb = new ExcelJS.Workbook()
  const FIXED = new Date('2026-01-01T00:00:00Z')
  wb.created = FIXED
  wb.modified = FIXED

  const s1 = wb.addWorksheet('Summary')
  s1.addRow(['Metric', 'Value', 'Delta'])
  s1.addRow(['Revenue', 128000, '+12%'])
  s1.addRow(['Costs', 74000, '-3%'])
  s1.addRow(['Margin', 54000, '+21%'])

  const s2 = wb.addWorksheet('Regions')
  s2.addRow(['Region', 'Q1', 'Q2', 'Q3', 'Q4'])
  for (const r of [
    ['NA', 40, 55, 60, 90],
    ['EU', 30, 45, 40, 70],
    ['APAC', 50, 60, 60, 80],
  ])
    s2.addRow(r)

  const s3 = wb.addWorksheet('Raw')
  s3.addRow(['id', 'ts', 'event', 'amount'])
  for (let i = 1; i <= 25; i++)
    s3.addRow([i, `2026-07-${String(i).padStart(2, '0')}`, 'purchase', i * 3.5])

  await wb.xlsx.writeFile(out('workbook.xlsx'))
}

// ---------------------------------------------------------------------------
// data.csv — plain CSV for the CSV renderer.
// ---------------------------------------------------------------------------
function genDataCsv() {
  const rows = [
    'gene,chromosome,expression,p_value',
    'TP53,17,8.42,0.0001',
    'EGFR,7,6.10,0.0034',
    'BRCA1,17,4.75,0.0210',
    'MYC,8,9.88,0.0000',
    'PTEN,10,3.21,0.0450',
  ]
  writeFileSync(out('data.csv'), `${rows.join('\n')}\n`)
}

// ---------------------------------------------------------------------------
// report.pdf — a minimal but valid single-page PDF (hand-written, no deps).
// ---------------------------------------------------------------------------
function genReportPdf() {
  const text = 'Showcase Report (report.pdf) - renders in the PDF viewer.'
  const content = Buffer.from(`BT /F1 18 Tf 72 720 Td (${text}) Tj ET`, 'latin1')
  const objs = [
    Buffer.from('<< /Type /Catalog /Pages 2 0 R >>', 'latin1'),
    Buffer.from('<< /Type /Pages /Kids [3 0 R] /Count 1 >>', 'latin1'),
    Buffer.from(
      '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] ' +
        '/Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>',
      'latin1',
    ),
    Buffer.from('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>', 'latin1'),
    Buffer.concat([
      Buffer.from(`<< /Length ${content.length} >>\nstream\n`, 'latin1'),
      content,
      Buffer.from('\nendstream', 'latin1'),
    ]),
  ]
  let pdf = Buffer.from('%PDF-1.4\n', 'latin1')
  const offsets = []
  objs.forEach((o, idx) => {
    offsets.push(pdf.length)
    pdf = Buffer.concat([
      pdf,
      Buffer.from(`${idx + 1} 0 obj\n`, 'latin1'),
      o,
      Buffer.from('\nendobj\n', 'latin1'),
    ])
  })
  const xrefPos = pdf.length
  let xref = `xref\n0 ${objs.length + 1}\n0000000000 65535 f \n`
  for (const off of offsets) xref += `${String(off).padStart(10, '0')} 00000 n \n`
  const trailer = `trailer\n<< /Size ${objs.length + 1} /Root 1 0 R >>\nstartxref\n${xrefPos}\n%%EOF`
  pdf = Buffer.concat([pdf, Buffer.from(xref + trailer, 'latin1')])
  writeFileSync(out('report.pdf'), pdf)
}

// ---------------------------------------------------------------------------
// script.py — a code file (exercises the source-code file viewer).
// ---------------------------------------------------------------------------
function genScriptPy() {
  const src = `#!/usr/bin/env python3
"""Attached code file — exercises the source-code file viewer."""


def fib(n: int) -> int:
    a, b = 0, 1
    for _ in range(n):
        a, b = b, a + b
    return a


if __name__ == "__main__":
    print([fib(i) for i in range(10)])
`
  writeFileSync(out('script.py'), src)
}

// ---------------------------------------------------------------------------
// notes.md — a markdown file attachment.
// ---------------------------------------------------------------------------
function genNotesMd() {
  const md = `# Project Notes (notes.md attachment)

- **Goal:** exercise the markdown *file* viewer (distinct from inline chat md).
- Supports \`inline code\`, [links](https://example.com), and tables:

| Step | Owner | Status |
|------|-------|--------|
| Spec | A     | done   |
| Impl | B     | wip    |
`
  writeFileSync(out('notes.md'), md)
}

// ---------------------------------------------------------------------------
// large.txt — a big-ish text blob to test scrolling / truncation.
// ---------------------------------------------------------------------------
function genLargeTxt() {
  const lines = []
  for (let i = 1; i <= 800; i++)
    lines.push(
      `${String(i).padStart(5, '0')}  Lorem ipsum dolor sit amet, consectetur adipiscing ` +
        'elit, sed do eiusmod tempor incididunt ut labore.',
    )
  writeFileSync(out('large.txt'), `${lines.join('\n')}\n`)
}

async function main() {
  genChartPng()
  genPhotoJpg()
  await genWorkbookXlsx()
  genDataCsv()
  genReportPdf()
  genScriptPy()
  genNotesMd()
  genLargeTxt()
  const { readdirSync, statSync } = await import('node:fs')
  console.log('Generated files in', OUT)
  for (const n of readdirSync(OUT).sort())
    console.log(`  ${n.padEnd(16)} ${String(statSync(join(OUT, n)).size).padStart(8)} bytes`)
}

main().catch((e) => {
  console.error(e)
  process.exit(1)
})
