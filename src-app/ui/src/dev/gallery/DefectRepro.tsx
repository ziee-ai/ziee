import { Tag, Button, Card } from '@/components/ui'

/**
 * DEFECT-REPRO — the detection system's living known-positive fixture.
 *
 * Every geometry detector needs at least one KNOWN POSITIVE so a regression that
 * silently disables the detector is itself caught (the detector stops flagging its
 * own fixture). This surface reproduces the canonical human-caught misses whose
 * real occurrences were since fixed in-app, so the pattern stays permanently under
 * test. It is INTENTIONALLY defective; its findings are allow-listed for the
 * `--gate` (see geometry-allowlist.json) but still reported (acceptance proof).
 *
 * - `repro-a1-zero-gap` — the original hardware "Disconnected"/"Connect" pair:
 *   a status Tag immediately followed by a Button in a flex row with NO gap
 *   utility, so the two boxes touch (taxonomy A1, user miss #1). The Card body
 *   markup mirrors the pre-fix HardwareSettings connection card.
 */
export function DefectRepro() {
  return (
    <div className="flex flex-col gap-6 p-6" data-testid="defect-repro-root">
      <Card data-testid="defect-repro-a1-card" title="A1 · zero-gap adjacency (hardware Disconnected/Connect)">
        {/* `data-allow-adjacent` opts the SOURCE lint out (this defect is
            intentional); the RUNTIME geometry audit still measures the touching
            boxes and flags A1 — that's the point of the fixture. */}
        <div className="flex items-center flex-wrap" data-allow-adjacent data-testid="repro-a1-zero-gap">
          <Tag variant="outline" tone="error" data-testid="repro-a1-tag">
            Disconnected
          </Tag>
          <Button variant="default" data-testid="repro-a1-button">
            Connect
          </Button>
        </div>
      </Card>
    </div>
  )
}
