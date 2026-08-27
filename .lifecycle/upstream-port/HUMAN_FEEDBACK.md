# HUMAN_FEEDBACK — upstream-port

- **FB-1** [status: resolved] — "All 10 (minus macOS-unverifiable)" — the owner's answer
  to an explicit option picker offering (a) all ten, (b) a Tier-1-only narrow diff,
  (c) all eleven including the macOS ggml shim. → Ten ported; the ggml symlink shim
  (`a9ab79375`) dropped and reported instead, as a workaround for a defect in the
  `ziee-ai/llama.cpp` release build that is unverifiable without a Darwin toolchain.
- **FB-2** [status: resolved] — "we just want to push our changes to sdk chat branch" —
  the owner's decision after being shown that sdk `main` is 135 commits behind `chat`
  and that `ziee-ai/ziee` pins `chat`. → The GPU/CUDA parser and the CORS union went to
  `ziee-ai/sdk` PR #5 (`chat`), not into this PR; this branch moves no gitlink.
  [generalizable: yes — before proposing a submodule branch as a target, check how far
  behind it is and which consumers actually pin it; "main" is not automatically the
  trunk in a repo with per-product-line branches]
- **FB-3** [status: resolved] — "Do NOT move the sdk submodule pointer in any PR"
  (standing instruction in the worker brief). → TEST-16 asserts mechanically that no
  gitlink moved. The two fixes that needed the sdk are the ONLY things excluded from an
  otherwise complete port, and both are named in PLAN's `## Out of scope`.
- **FB-4** [status: resolved] — "keep the diff clean enough to port to `ziee-ai/ziee`
  later" (the original `gpu-detect.md` and `realtime-sse.md` briefs, written when the
  paws fixes were authored). → Honoured in the direction it was meant: every item here
  is hand-written against upstream's shape rather than replayed as a paws commit, and
  DEC-3 records the concrete case where a blind file copy would have dragged paws
  product code (`repository.git_credential()`) into upstream.

No human feedback has been received on the running code — the branch has not yet been
reviewed by the owner. The four entries above are the instructions that shaped it,
recorded with their resolutions.

⚠ **One thing the owner should see before merging**, because a blind audit caught it and
it changes what this PR claims: the kill-switch commit originally asserted that an
ordinary user could `POST /api/run-js/mcp` and *execute arbitrary script*. That is
**false for `js_tool`**, whose `tools/call` arm refuses; it holds only for `web_search`
and `lit_search`, which do dispatch. The claim is corrected per module in the code,
the tests and the later commit message — but the earlier commit's message still carries
the original wording, and the paws commit being ported (`816aa6321`) makes the same
over-broad claim. Flagged here so the correction is not buried in a diff.
