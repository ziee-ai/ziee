/**
 * Per-pane capture of the composer draft key at send-start (audit #4).
 *
 * The text extension captures the draft key in `beforeSendMessage` (before a
 * new-chat send creates the conversation and flips the composer's key) and reads
 * it back in `onMessageSent`. A single module-global `let` let two CONCURRENT
 * sends clobber each other — the second send's `beforeSendMessage` overwrote the
 * first's captured key before the first's `onMessageSent` consumed it, so a send
 * could clear/restore the WRONG conversation's draft. Keying the capture by the
 * SENDING pane makes the two independent.
 */

/** Map a pane id to its capture-key slot (`''` for single-pane / no pane). */
export function paneKeyOf(ownerPaneId?: string | null): string {
  return ownerPaneId ?? ''
}

/** A tiny per-pane store for the captured draft key — set at send-start, taken
 * (read-and-cleared) once when the send resolves. */
export class PaneDraftKeys {
  private keys = new Map<string, string>()

  /** Record the draft key for the SENDING pane (overwrites only that pane's slot). */
  set(ownerPaneId: string | null | undefined, draftKey: string): void {
    this.keys.set(paneKeyOf(ownerPaneId), draftKey)
  }

  /** Read AND clear the draft key for the OWNING pane (one-shot). */
  take(ownerPaneId: string | null | undefined): string | undefined {
    const k = paneKeyOf(ownerPaneId)
    const v = this.keys.get(k)
    this.keys.delete(k)
    return v
  }
}
