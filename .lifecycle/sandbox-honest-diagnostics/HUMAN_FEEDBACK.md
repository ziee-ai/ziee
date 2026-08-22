# HUMAN_FEEDBACK — sandbox honest diagnostics

No human feedback received — this feature was implemented autonomously end-to-end
from the bug-report brief. The file is present as the deliberate absence claim
required by phase 9; it will be updated verbatim if a reviewer exercises the
running behavior and comments.

Owner sign-off is against the acceptance tests, not "all green":

- **INV-1** proven by TEST-1 (every `SandboxAvailability` variant explains itself
  honestly; the false "not yet booted" clause is gone) + TEST-2 (the
  `SANDBOX_NOT_INITIALIZED` error carries the real recorded reason). All 7
  producer sites route through `config::init_status().explain()`.
- **INV-2** proven by TEST-3 (the seccomp-write classifier quiets the
  routine EPIPE child-gone case and keeps a genuine truncation loud — incl. the
  "loud-stays-loud" trap).
- **INV-3** proven by TEST-4/TEST-5 (host bind-source paths — workspace, rootfs,
  and workflow/provider/caller mount sources — are scrubbed from the success-path
  result, including a simulated bwrap dead-mount stderr line).
