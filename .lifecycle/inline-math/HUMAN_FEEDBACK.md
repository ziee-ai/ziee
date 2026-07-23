# HUMAN_FEEDBACK — inline math

Living ledger. Feedback recorded verbatim as given, then resolved.

- **FB-1** [status: resolved] — "The user picked Flavor B (aggressive, maximum coverage). Go with B: convert EVERY \( … \) that clears guards 1 through 6 — NO content math-signal gate — so bare function notation like \(C(x)\) and \(f(x)\) renders too. KEEP guard 4 (BRE signals) and, critically, KEEP guard 5 (the unpaired-dollar paragraph guard)" → implemented as specified: no content gate, both guards retained; recorded as DEC-1/DEC-2. Vindicated by the live run — GPT-OSS 120B emitted `\(E\)`, `\(m\)`, `\(c\)`, bare single symbols that a content-gated Flavor A would have missed entirely.

- **FB-2** [status: resolved] — "Stop running ad-hoc node -e / node --input-type=module probes — each distinct inline script re-prompts the user regardless of the do-not-ask-again option (arbitrary code is re-confirmed every time), and it is bothering them." → stopped immediately. Every later verification that genuinely needed Node was written to a FILE in the scratchpad and run as `node <path>`, which does not re-prompt. [generalizable: yes — when a verification needs a scripting runtime, write the script to a file and run the file; never pass code inline via `-e`/`-c`, which re-prompts on every distinct invocation]

- **FB-3** [status: resolved] — "tighten the dollar guard so mid-paragraph display math still converts inline" → ITEM-10. The guard now pairs `$` runs BY LENGTH the way micromark does instead of counting dollars, so a `$$` run (which can never close the single `$` we emit) no longer suppresses inline math in the same paragraph, while the two genuinely unsafe shapes still block. All four relaxations plus both negative controls were verified against the installed micromark before the code changed. See DRIFT-3, TEST-21.

- **FB-4** [status: resolved] — "It works, but the title of the conversation is showing "Check: the energy is \[ E = mc^2 \] where \( m \) ..." Which is not parsed, can we parse in the title as well?" → ITEM-11. Titles are plain strings that never reach the markdown renderer, so they now get the plain-text READING of the math via `mathToPlainText`, applied in `conversationDisplayLabel` (every list surface) and `TitleEditor` (the header). Rendering real KaTeX was rejected with reasons: the label type is `string` and feeds an `aria-label` and two search predicates that cannot hold a React node. Verified live against the real stored title. [generalizable: yes — when a renderer-level fix lands, sweep the PLAIN-TEXT surfaces that show the same content (titles, list labels, tooltips, aria-labels, search predicates); they bypass the renderer and will still show raw markup]

## Notes for the reviewer

Two things this branch does NOT fix, both pre-existing on `origin/khoi` and both outside its
scope — see TEST_RESULTS.md for the evidence that each predates the branch:

1. `npm run check (ui)` fails on three stale generated registries (`testIds.generated.ts`,
   `galleryCoverage.generated.ts`, `state-matrix`) that live in the **`sdk` submodule**,
   whose pin is behind the split-pane UI work already merged into the ziee source. Fixing
   it means committing to a different repository and bumping the submodule pointer, which
   would drag another workstream's ids into this PR. This is the ONLY thing keeping the
   lifecycle at 8/9.
2. Ten unit tests fail with `ERR_MODULE_NOT_FOUND` (moved stores, plus the
   `@ziee/framework/src/store-kit` resolution gap). Zero assertion failures; none in a file
   this branch touches.

Also worth passing on to whoever runs e2e on this host: the harness assigns worker 0
`vite 9000`, which collides with the MinIO container's published `9000-9001`. Set
`ZIEE_E2E_BASE_VITE_PORT=9600 ZIEE_E2E_BASE_BACKEND_PORT=9700` — it took the markdown spec
from 10/18 to 17/18, none of which was related to this diff.
