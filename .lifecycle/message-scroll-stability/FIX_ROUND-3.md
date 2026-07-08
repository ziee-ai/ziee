# FIX_ROUND-3 — message-scroll-stability (convergence)

Final blind re-audit of the three FIX_ROUND-2 fixes (faithful virtualizer
predicate, signal key-ownership, guarded `releasePointerCapture`), diff-only.

The agent verified each against the real sources:

- **Predicate faithful** — compared to the installed `virtual-core@3.17.3`
  `index.js:869`; same operands, same ordering, `+ scrollAdjustments` +
  `getScrollOffset()` both present; confirmed the object virtual-core passes the
  custom predicate carries `.key`/`.start` matching the default branch's
  `key`/`itemStart`; the parked-key short-circuit returns false ONLY for the one
  parked row.
- **Cross-row clobber closed** — `unparkIfOwned` gates every clear on
  `signal.key === key`; the surviving parked key is always owned by the hook that
  fires its own raf2 or its unmount `cancel()`; `pending.current` is set
  synchronously with the key before raf1, so an unmount between the sync body and
  raf1 still unparks. No forever-parked leak.
- **`releasePointerCapture` guarded** — `endDrag()` is outside the try block, so a
  `NotFoundError` is swallowed and the height commit still runs.

Verdict: "No new or real defects found. The 3 fixes are correct and introduce no
new defect."

**New confirmed findings:** 0
