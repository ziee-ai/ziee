# HUMAN_FEEDBACK

- **FB-1** [status: resolved] — "So no need to fix the ssh, please remove it" → Dropped the SSH-tunnel-lag symptom (symptom B) entirely: no SSH client-config guidance and no server-side keepalive-hardening for it. Scope narrowed to the Coder published-URL streaming fix only (A1: inner-nginx `X-Accel-Buffering: no`). [generalizable: yes — when a user explicitly narrows scope mid-plan, prune the dropped work from the plan/diff entirely rather than keeping it as "defense-in-depth".]
- **FB-2** [status: resolved] — Earlier scope choice "A1 + A2 keepalive hardening" via AskUserQuestion, then superseded by FB-1 (remove SSH). A2 was SSH-symptom hardening, so it was dropped with the SSH work; the shipped fix is A1 only. → Reconciled: final scope is the nginx header fix + its regression guard.

No further human feedback received on the running fix.
