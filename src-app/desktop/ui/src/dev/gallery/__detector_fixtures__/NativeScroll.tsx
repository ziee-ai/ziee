/**
 * INTENTIONALLY DEFECTIVE source fixture for the native-scroll lint (taxonomy J8,
 * user miss #17: the conversation message list uses a raw native scrollbar on
 * desktop instead of the shared <DivScrollY>). NEVER rendered — lint fodder only.
 * Excluded from the repo-wide lint scan; the acceptance harness targets this dir.
 */
export function NativeScroll() {
  return (
    <div className="flex h-full flex-col overflow-y-auto p-4">
      <p>message 1</p>
      <p>message 2</p>
    </div>
  )
}
