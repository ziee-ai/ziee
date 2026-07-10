# Human Feedback — scheduled-tasks

Living ledger (Phase 9 gate). Each human feedback item is recorded VERBATIM as it
arrives, with a status and — where it generalizes beyond this feature — a candidate
fleet-wide rule. Any `[status: open]` fails the Phase 9 gate.

- **FB-1** [status: open] — "it did NOT reuse existing page/drawer layouts (chat page, project page, etc.) -- match those, do not invent new ones" [generalizable: yes — a new feature's page/drawer/settings surfaces MUST mirror the closest existing layout (chat page, project page, settings card/drawer); never invent a bespoke layout when an established one fits]
- **FB-2** [status: open] — "the Drawer asks the user to TYPE the ID of an assistant and the ID of a workflow to run -- users never see those IDs and never would; replace every raw-ID text input with a proper selection picker" [generalizable: yes — never ask a user to type a raw entity ID (assistant/workflow/model/provider/etc.); any field whose value is an entity reference MUST be a selection picker (dropdown/searchable list) populated from that entity's list endpoint, showing human-readable names]
