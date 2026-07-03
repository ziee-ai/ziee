import { isValidElement, type ReactNode } from 'react'

/** Flatten a React node tree to its text content (for deriving a heading slug). */
export function nodeToText(node: ReactNode): string {
  if (node == null || typeof node === 'boolean') return ''
  if (typeof node === 'string' || typeof node === 'number') return String(node)
  if (Array.isArray(node)) return node.map(nodeToText).join('')
  if (isValidElement(node)) {
    return nodeToText((node.props as { children?: ReactNode }).children)
  }
  return ''
}

/**
 * GitHub-style heading slug (approximation): lowercase, drop punctuation except
 * word chars / spaces / hyphens, spaces→hyphens, collapse + trim hyphens. Used
 * for BOTH the heading `id` and the hash-link target so `[Foo](#foo)` resolves.
 */
export function slugifyHeading(text: string): string {
  return text
    .trim()
    .toLowerCase()
    .replace(/[^\w\s-]/g, '')
    .replace(/\s+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-+|-+$/g, '')
}

/** decodeURIComponent that never throws on a malformed `%` sequence. */
export function safeDecode(s: string): string {
  try {
    return decodeURIComponent(s)
  } catch {
    return s
  }
}
