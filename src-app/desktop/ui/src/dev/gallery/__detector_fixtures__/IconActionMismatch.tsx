/**
 * INTENTIONALLY DEFECTIVE source fixture for the icon-action lint (taxonomy C11,
 * user miss #10b). NEVER rendered — lint fodder only. Each kit <Button>'s
 * accessible name maps to a known action whose conventional lucide glyph it does
 * NOT render. The lint only inspects capitalized `*Button` components, so this
 * uses the kit <Button> (not a raw <button>). Excluded from the repo-wide lint
 * scan; the acceptance harness targets this dir via --root.
 */
import { ArrowRight, Copy } from 'lucide-react'
import { Button } from '@/components/ui'

export function IconActionMismatch() {
  return (
    <div>
      {/* "open in new tab" should render ExternalLink — this bare arrow lies. */}
      <Button data-testid="fixture-c11-newtab" aria-label="open in new tab">
        <ArrowRight />
      </Button>
      {/* "download" should render Download — this Copy glyph lies. */}
      <Button data-testid="fixture-c11-download" aria-label="download">
        <Copy />
      </Button>
    </div>
  )
}
