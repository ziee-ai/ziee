/** Generate a unique pane identifier. */
export default (): string =>
  typeof crypto !== 'undefined' && 'randomUUID' in crypto
    ? crypto.randomUUID()
    : `pane-${Date.now()}-${Math.floor(Math.random() * 1e6)}`
