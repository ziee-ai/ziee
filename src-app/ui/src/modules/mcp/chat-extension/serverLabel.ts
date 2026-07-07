/**
 * The tool-call header shows a human server name in parentheses after the tool
 * name (e.g. tool "get_weather" · "(Weather API)"). Two bugs this guards:
 *
 *  1. A raw UUID `server_id` (row not loaded) is meaningless to the user, so the
 *     caller suppresses it — see {@link looksLikeId}.
 *  2. A display name may ALREADY carry its own parenthetical qualifier
 *     ("Weather API (external)"). Blindly wrapping it in another paren layer
 *     produced the doubled "(Weather API (external))" (finding #7). Only add the
 *     decorative outer parens when the name has none of its own.
 */

/** A bare UUID `server_id` used as a fallback name — suppress it (meaningless). */
export const looksLikeId = (s?: string | null): boolean =>
  !!s && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-/i.test(s)

/**
 * Format a server display name for the parenthetical header slot. Returns the
 * name wrapped in a single pair of parens — unless the name already contains a
 * parenthetical of its own, in which case it is returned as-is (no double wrap).
 * Returns `null` when there's nothing worth showing (empty / a raw id).
 */
export function mcpServerParenLabel(name?: string | null): string | null {
  const trimmed = name?.trim()
  if (!trimmed || looksLikeId(trimmed)) return null
  return trimmed.includes('(') ? trimmed : `(${trimmed})`
}
