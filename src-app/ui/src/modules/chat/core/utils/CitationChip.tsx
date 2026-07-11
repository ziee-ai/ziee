import type { SyntheticEvent } from 'react'

/**
 * An inline numbered citation chip (`[n]`) in an assistant answer grounded on a
 * knowledge base. Clicking it reveals the retrieval-transparency panel in the
 * SAME message and scrolls to the n-th cited passage (whose row carries an
 * "Open source" jump). DOM-coupled by testid so it needs no data threading.
 * Rendered as an accessible `role="button"` span (inline, keyboard-operable) —
 * the same non-raw-`<button>` pattern the composer menu items use.
 */
export function CitationChip({ n }: { n: number }) {
  const activate = (e: SyntheticEvent<HTMLElement>) => {
    e.preventDefault()
    // Walk up until an ancestor also contains this message's transparency card.
    let node: HTMLElement | null = e.currentTarget
    let card: HTMLElement | null = null
    while (node && !card) {
      card = node.querySelector<HTMLElement>('[data-testid="kb-tool-result-card"]')
      node = node.parentElement
    }
    if (!card) return
    const toggle = card.querySelector<HTMLElement>('[data-testid="kb-tool-result-toggle"]')
    if (toggle?.getAttribute('aria-expanded') === 'false') toggle.click()
    card.scrollIntoView({ behavior: 'smooth', block: 'center' })
    // After the expand renders, bring the n-th passage into view.
    window.setTimeout(() => {
      card
        ?.querySelector<HTMLElement>(`[data-testid="kb-hit-source-${n - 1}"]`)
        ?.scrollIntoView({ behavior: 'smooth', block: 'center' })
    }, 80)
  }

  return (
    <span
      role="button"
      tabIndex={0}
      onClick={activate}
      onKeyDown={e => {
        if (e.key === 'Enter' || e.key === ' ') activate(e)
      }}
      data-testid={`kb-citation-chip-${n}`}
      aria-label={`Citation ${n} — show source`}
      className="mx-0.5 inline-flex cursor-pointer items-center rounded-sm bg-info/15 px-1 align-baseline text-[0.7em] font-medium leading-tight text-info hover:bg-info/25 focus-visible:outline focus-visible:outline-2"
    >
      {n}
    </span>
  )
}
