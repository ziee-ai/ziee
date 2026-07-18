# MIGRATE-squash — TRANSFORMS

Reconstruction transforms applied to reach the squashed, module-owned migration
set. Each is equivalence-preserving and gated by EA-schema + EA-seed + N9.

- **T-1** migration history: **squash** 147 numeric migrations → 91 module-owned
  `<YYYYMMDDNNNN>_<module>_<desc>.sql` baselines — **why:** N3.1/N7 (module-owned
  schema for the extract-to-crate future) + N8 (no deployed DBs → squash freely).
- **T-2** schema source: reconstruct table DDL from `pg_dump --schema-only` of the
  numeric-migrated catalog, split per owning table into module files, with **all
  foreign keys deferred** to a post-schema version band — **why:** pg_dump is a
  catalog-faithful source of truth; deferring FKs frees cross-module table order
  so no module needs another module's version number.
- **T-3** CHECK constraints: rewrite pg_dump's expanded VARCHAR
  `((col)::text = ANY ((ARRAY['x'::character varying,…])::text[]))` back to the
  original `col IN ('x',…)` form (27 constraints) — **why:** pg_dump round-trips
  that form **non-idempotently** on PG18 (re-normalizes to an element-cast variant
  that differs under `pg_get_constraintdef`); `IN (…)` is the proven fixed-point
  that stores the identical catalog form as baseline. This reproduces the original
  author DDL; it does NOT weaken the fingerprint. (Proof: GATE_PROOF.md.)
- **T-4** shared trigger fn `update_updated_at_column()`: define in the framework
  bootstrap (sorts first) AND repeat idempotently (`CREATE OR REPLACE`) in the
  auth schema — **why:** the auth-only build DB (`AUTH_MIGRATOR`, applied
  standalone by `ziee-auth/build.rs`) must be self-contained; its `updated_at`
  triggers need the fn without the app bootstrap present.
- **T-5** N9 domain-perm split: the auth seed creates system groups with a CLEAN
  base (`Administrators=['*']`, `Users=['profile::read','profile::edit']`); every
  domain permission (`chat::`,`files::`,`mcp_servers::`,`hub::`,`assistants::`,
  `conversations::`,`branches::`,`messages::`,`user_llm_providers::`, and the 15
  already-module-owned grants) moves to its owning module's
  `*_grant_permissions.sql` — **why:** N9 (a crate/module migration must not seed
  another module's domain data; the old `27_fix_default_user_permissions` leak).
  The add-then-remove churn (mig 27 grants `hub::models::*`, mig 37 removes them)
  is collapsed — the clean grants target the NET final set directly.
- **T-6** `build.rs::compose_merged_migrations()`: source globs widened from the
  two fixed dirs to `src/modules/*/migrations/ ∪ ../../sdk/crates/*/migrations/`,
  with a basename-collision panic guard — **why:** module-owned composition (N7).

## Table → module ownership map (H4)

| module (dir) | tables |
|---|---|
| ziee-auth | users, groups, user_groups, auth_providers, user_auth_links, oauth_sessions, refresh_tokens, pending_account_links, session_settings |
| app (bootstrap) | *(extensions + shared trigger fn only)* |
| assistant | assistants |
| assistant_core_memory | assistant_core_memory |
| chat | conversations, branches, branch_messages, messages, message_contents, message_assistant, message_mcp_servers, conversation_deliverables |
| skill | skills, group_skills, conversation_skill_overrides |
| summarization | conversation_summaries, conversation_summarization_settings, summarization_admin_settings |
| memory | user_memories, user_memory_settings, memory_admin_settings, memory_audit_log, conversation_memory_settings |
| knowledge_base | knowledge_bases, knowledge_base_documents, conversation_knowledge_bases, project_knowledge_bases |
| mcp | mcp_servers, mcp_settings, mcp_server_oauth_configs, mcp_user_policy, mcp_tool_calls, user_group_mcp_servers, user_mcp_defaults, tool_use_approvals |
| llm_provider | llm_providers, user_group_llm_providers, user_llm_provider_api_keys |
| llm_provider_files | llm_provider_files |
| llm_repository | llm_repositories |
| llm_model | llm_models, llm_model_files, download_instances |
| llm_local_runtime | llm_runtime_instances, llm_runtime_versions, llm_runtime_settings |
| file | files, file_versions |
| file_rag | file_chunks, file_index_state, file_rag_admin_settings |
| hub | hub_entities, hub_settings |
| js_tool | js_tool_settings |
| lit_search | lit_search_settings, lit_search_connectors, lit_fulltext_cache, user_lit_search_connector_keys |
| web_search | web_search_settings, web_search_providers, user_web_search_provider_keys |
| citations | bibliography_entries, project_bibliography |
| project | projects, project_files, project_conversations |
| code_sandbox | code_sandbox_settings, code_sandbox_rootfs_artifacts, sandbox_workspace_files |
| workflow | workflows, workflow_runs, group_workflows |
| scheduler | scheduled_tasks, scheduled_task_runs, scheduler_admin_settings |
| notification | notifications |
| voice | voice_models, voice_runtime_instance, voice_runtime_settings, voice_runtime_versions |
| onboarding | user_onboarding |

(Standalone functions: `enforce_system_scope_for_group_skills`→skill,
`enforce_system_scope_for_group_workflows`→workflow. Sequences
`lit_fulltext_cache_id_seq`→lit_search, `memory_audit_log_id_seq`→memory.)

## Decision

**Question:** what is the load-bearing equivalence relation for a squash, and how
are the pg_dump round-trip artifacts (CHECK non-idempotency, FK/attnum ordering,
auto-name suffixes) reconciled without weakening the gate?

**Resolution:** the equivalence relation is the **catalog-derived logical
fingerprint** (EA-schema, name/order-invariant, generated-expr + opclass +
constraint-def aware) plus the **business-key whole-DB seed image** (EA-seed) —
NOT byte-identical `pg_dump` (B1). Round-trip artifacts are reconciled at the
SOURCE (rewrite CHECKs to the original `IN(…)` fixed-point — T-3; defer FKs so
attnum order is irrelevant — T-2), never by special-casing the fingerprint. The
fingerprint script is re-run by the validator on both DBs and must diff empty.
Determinism + discrimination of the tooling proven in GATE_PROOF.md before any
squash. Auth-domain purity enforced by the N9 grep. Zero TBD.
