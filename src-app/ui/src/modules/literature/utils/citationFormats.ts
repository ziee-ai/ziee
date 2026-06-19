import { type LiteratureRecord, recordKey, type ScreeningDecision } from '../types'

/** Trigger a client-side download (Blob + objectURL + <a download>). Mirrors the
 *  chat export extension's mechanism. */
export function downloadText(filename: string, mime: string, content: string): void {
  const blob = new Blob([content], { type: mime })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
  URL.revokeObjectURL(url)
}

/** RIS — one block per record (JOUR, or UNPB for preprints). */
export function toRis(records: LiteratureRecord[]): string {
  return records
    .map(r => {
      const lines: string[] = []
      lines.push(`TY  - ${r.is_preprint ? 'UNPB' : 'JOUR'}`)
      lines.push(`TI  - ${r.title}`)
      for (const a of r.authors) lines.push(`AU  - ${a}`)
      if (r.year != null) lines.push(`PY  - ${r.year}`)
      if (r.venue) lines.push(`JO  - ${r.venue}`)
      if (r.doi) lines.push(`DO  - ${r.doi}`)
      if (r.url) lines.push(`UR  - ${r.url}`)
      if (r.abstract_text) lines.push(`AB  - ${r.abstract_text.replace(/\r?\n/g, ' ')}`)
      lines.push('ER  - ')
      return lines.join('\r\n')
    })
    .join('\r\n')
}

function bibEscape(s: string): string {
  return s.replace(/([{}%&$#_])/g, '\\$1')
}

/** BibTeX — `@article` (or `@misc` for preprints) with a stable cite key. */
export function toBibtex(records: LiteratureRecord[]): string {
  return records
    .map((r, i) => {
      const entryType = r.is_preprint ? 'misc' : 'article'
      const surname = (r.authors[0] ?? 'anon').split(/\s+/)[0].replace(/[^A-Za-z]/g, '')
      const titleWord = r.title.split(/\s+/)[0]?.replace(/[^A-Za-z0-9]/g, '') ?? 'ref'
      const key = `${surname}${r.year ?? ''}${titleWord}`.trim() || `ref${i + 1}`
      const fields: string[] = [`  title = {${bibEscape(r.title)}}`]
      if (r.authors.length) {
        fields.push(`  author = {${r.authors.map(bibEscape).join(' and ')}}`)
      }
      if (r.year != null) fields.push(`  year = {${r.year}}`)
      if (r.venue) fields.push(`  journal = {${bibEscape(r.venue)}}`)
      if (r.doi) fields.push(`  doi = {${bibEscape(r.doi)}}`)
      if (r.url) fields.push(`  url = {${bibEscape(r.url)}}`)
      return `@${entryType}{${key},\n${fields.join(',\n')}\n}`
    })
    .join('\n\n')
}

/** RFC-4180 cell escaping: quote when the value contains a comma, quote, or
 *  newline, doubling any embedded quotes. */
function csvCell(value: string): string {
  const needsQuote = /[",\r\n]/.test(value)
  const escaped = value.replace(/"/g, '""')
  return needsQuote ? `"${escaped}"` : escaped
}

export function toCsv(
  records: LiteratureRecord[],
  decisions: Record<string, ScreeningDecision>,
  reasons: Record<string, string>,
): string {
  const header = [
    'DOI',
    'PMID',
    'Title',
    'Authors',
    'Year',
    'Venue',
    'URL',
    'Source',
    'Decision',
    'Reason',
  ]
  const rows = records.map(r => {
    const key = recordKey(r)
    return [
      r.doi ?? '',
      r.pmid ?? '',
      r.title,
      r.authors.join('; '),
      r.year != null ? String(r.year) : '',
      r.venue ?? '',
      r.url ?? '',
      r.source,
      decisions[key] ?? 'unscreened',
      reasons[key] ?? '',
    ]
      .map(c => csvCell(String(c)))
      .join(',')
  })
  return [header.join(','), ...rows].join('\r\n')
}
