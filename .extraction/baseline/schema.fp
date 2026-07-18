EXT	pgcrypto		
EXT	vector		
COL	assistant_core_memory.assistant_id	uuid null=false gen=	
COL	assistant_core_memory.block_label	text null=false gen=	
COL	assistant_core_memory.char_limit	integer null=false gen=	2000
COL	assistant_core_memory.content	text null=false gen=	
COL	assistant_core_memory.created_at	timestamp with time zone null=false gen=	now()
COL	assistant_core_memory.id	uuid null=false gen=	gen_random_uuid()
COL	assistant_core_memory.updated_at	timestamp with time zone null=false gen=	now()
COL	assistant_core_memory.user_id	uuid null=false gen=	
COL	assistants.created_at	timestamp with time zone null=false gen=	now()
COL	assistants.created_by	uuid null=true gen=	
COL	assistants.description	text null=true gen=	
COL	assistants.enabled	boolean null=false gen=	true
COL	assistants.id	uuid null=false gen=	gen_random_uuid()
COL	assistants.instructions	text null=true gen=	
COL	assistants.is_default	boolean null=false gen=	false
COL	assistants.is_template	boolean null=false gen=	false
COL	assistants.name	character varying(255) null=false gen=	
COL	assistants.parameters	jsonb null=true gen=	'{}'::jsonb
COL	assistants.updated_at	timestamp with time zone null=false gen=	now()
COL	auth_providers.client_secret_encrypted	bytea null=true gen=	
COL	auth_providers.config	jsonb null=false gen=	
COL	auth_providers.created_at	timestamp with time zone null=false gen=	now()
COL	auth_providers.enabled	boolean null=false gen=	true
COL	auth_providers.id	uuid null=false gen=	gen_random_uuid()
COL	auth_providers.last_test_at	timestamp with time zone null=true gen=	
COL	auth_providers.last_test_message	text null=true gen=	
COL	auth_providers.last_test_ok	boolean null=true gen=	
COL	auth_providers.name	character varying(100) null=false gen=	
COL	auth_providers.provider_type	character varying(50) null=false gen=	
COL	auth_providers.updated_at	timestamp with time zone null=false gen=	now()
COL	bibliography_entries.arxiv_id	text null=true gen=	
COL	bibliography_entries.citation_key	text null=false gen=	
COL	bibliography_entries.content_tsv	tsvector null=true gen=s	to_tsvector('english'::regconfig, COALESCE(title, ''::text))
COL	bibliography_entries.created_at	timestamp with time zone null=false gen=	now()
COL	bibliography_entries.csl_json	jsonb null=false gen=	
COL	bibliography_entries.dedup_fingerprint	text null=true gen=	
COL	bibliography_entries.doi	text null=true gen=	
COL	bibliography_entries.id	uuid null=false gen=	gen_random_uuid()
COL	bibliography_entries.pmcid	text null=true gen=	
COL	bibliography_entries.pmid	text null=true gen=	
COL	bibliography_entries.source	text null=true gen=	
COL	bibliography_entries.title	text null=true gen=	
COL	bibliography_entries.updated_at	timestamp with time zone null=false gen=	now()
COL	bibliography_entries.user_id	uuid null=false gen=	
COL	bibliography_entries.verification_status	text null=false gen=	'unverified'::text
COL	bibliography_entries.verified_at	timestamp with time zone null=true gen=	
COL	bibliography_entries.year	integer null=true gen=	
COL	branch_messages.branch_id	uuid null=false gen=	
COL	branch_messages.created_at	timestamp with time zone null=false gen=	CURRENT_TIMESTAMP
COL	branch_messages.id	uuid null=false gen=	gen_random_uuid()
COL	branch_messages.is_clone	boolean null=false gen=	false
COL	branch_messages.message_id	uuid null=false gen=	
COL	branches.conversation_id	uuid null=false gen=	
COL	branches.created_at	timestamp with time zone null=false gen=	now()
COL	branches.created_from_message_id	uuid null=true gen=	
COL	branches.fork_level	text null=false gen=	'user'::text
COL	branches.id	uuid null=false gen=	gen_random_uuid()
COL	branches.parent_branch_id	uuid null=true gen=	
COL	code_sandbox_rootfs_artifacts.arch	text null=false gen=	
COL	code_sandbox_rootfs_artifacts.artifact_path	text null=false gen=	
COL	code_sandbox_rootfs_artifacts.cosign_bundle	text null=true gen=	
COL	code_sandbox_rootfs_artifacts.downloaded_at	timestamp with time zone null=false gen=	now()
COL	code_sandbox_rootfs_artifacts.flavor	text null=false gen=	
COL	code_sandbox_rootfs_artifacts.id	uuid null=false gen=	gen_random_uuid()
COL	code_sandbox_rootfs_artifacts.last_used_at	timestamp with time zone null=true gen=	
COL	code_sandbox_rootfs_artifacts.package	text null=false gen=	
COL	code_sandbox_rootfs_artifacts.sha256	text null=false gen=	
COL	code_sandbox_rootfs_artifacts.status	text null=false gen=	'installed'::text
COL	code_sandbox_rootfs_artifacts.version	text null=false gen=	
COL	code_sandbox_settings.address_space_bytes	bigint null=false gen=	'4294967296'::bigint
COL	code_sandbox_settings.cpu_max	text null=false gen=	'100000 100000'::text
COL	code_sandbox_settings.cpu_secs_max	integer null=false gen=	1240
COL	code_sandbox_settings.created_at	timestamp with time zone null=false gen=	now()
COL	code_sandbox_settings.current_rootfs_version	text null=true gen=	
COL	code_sandbox_settings.fsize_bytes	bigint null=false gen=	268435456
COL	code_sandbox_settings.id	boolean null=false gen=	true
COL	code_sandbox_settings.mac_vm_ram_mib	integer null=false gen=	2048
COL	code_sandbox_settings.mac_vm_vcpus	integer null=false gen=	2
COL	code_sandbox_settings.memory_max_bytes	bigint null=false gen=	536870912
COL	code_sandbox_settings.memory_swap_max_bytes	bigint null=false gen=	0
COL	code_sandbox_settings.nofile_max	integer null=false gen=	1024
COL	code_sandbox_settings.nproc_max	integer null=false gen=	256
COL	code_sandbox_settings.pids_max	integer null=false gen=	256
COL	code_sandbox_settings.timeout_secs	integer null=false gen=	620
COL	code_sandbox_settings.updated_at	timestamp with time zone null=false gen=	now()
COL	code_sandbox_settings.vm_idle_evict_secs	integer null=false gen=	900
COL	code_sandbox_settings.vm_max_concurrent_execs	integer null=false gen=	3
COL	conversation_deliverables.conversation_id	uuid null=false gen=	
COL	conversation_deliverables.created_at	timestamp with time zone null=false gen=	now()
COL	conversation_deliverables.file_id	uuid null=false gen=	
COL	conversation_deliverables.pinned	boolean null=false gen=	true
COL	conversation_deliverables.title	text null=true gen=	
COL	conversation_knowledge_bases.added_at	timestamp with time zone null=false gen=	now()
COL	conversation_knowledge_bases.conversation_id	uuid null=false gen=	
COL	conversation_knowledge_bases.knowledge_base_id	uuid null=false gen=	
COL	conversation_memory_settings.conversation_id	uuid null=false gen=	
COL	conversation_memory_settings.memory_mode	text null=false gen=	
COL	conversation_skill_overrides.conversation_id	uuid null=false gen=	
COL	conversation_skill_overrides.created_at	timestamp with time zone null=false gen=	now()
COL	conversation_skill_overrides.hidden	boolean null=false gen=	true
COL	conversation_skill_overrides.skill_id	uuid null=false gen=	
COL	conversation_summaries.branch_id	uuid null=false gen=	
COL	conversation_summaries.created_at	timestamp with time zone null=false gen=	now()
COL	conversation_summaries.message_count	integer null=false gen=	0
COL	conversation_summaries.model_used	text null=true gen=	
COL	conversation_summaries.summarized_up_to_id	uuid null=true gen=	
COL	conversation_summaries.summary_text	text null=false gen=	
COL	conversation_summaries.updated_at	timestamp with time zone null=false gen=	now()
COL	conversation_summarization_settings.conversation_id	uuid null=false gen=	
COL	conversation_summarization_settings.summarization_mode	text null=false gen=	'inherit'::text
COL	conversation_summarization_settings.updated_at	timestamp with time zone null=false gen=	now()
COL	conversations.active_branch_id	uuid null=true gen=	
COL	conversations.created_at	timestamp with time zone null=false gen=	now()
COL	conversations.id	uuid null=false gen=	gen_random_uuid()
COL	conversations.model_id	uuid null=true gen=	
COL	conversations.title	character varying(500) null=true gen=	
COL	conversations.updated_at	timestamp with time zone null=false gen=	now()
COL	conversations.user_id	uuid null=false gen=	
COL	download_instances.completed_at	timestamp with time zone null=true gen=	
COL	download_instances.created_at	timestamp with time zone null=false gen=	CURRENT_TIMESTAMP
COL	download_instances.error_message	text null=true gen=	
COL	download_instances.id	uuid null=false gen=	gen_random_uuid()
COL	download_instances.model_id	uuid null=true gen=	
COL	download_instances.progress_data	jsonb null=true gen=	'{}'::jsonb
COL	download_instances.provider_id	uuid null=false gen=	
COL	download_instances.repository_id	uuid null=false gen=	
COL	download_instances.request_data	jsonb null=false gen=	
COL	download_instances.started_at	timestamp with time zone null=false gen=	CURRENT_TIMESTAMP
COL	download_instances.status	character varying(50) null=false gen=	
COL	download_instances.updated_at	timestamp with time zone null=false gen=	CURRENT_TIMESTAMP
COL	file_chunks.blob_version_id	uuid null=false gen=	
COL	file_chunks.char_end	integer null=false gen=	
COL	file_chunks.char_start	integer null=false gen=	
COL	file_chunks.chunk_index	integer null=false gen=	
COL	file_chunks.content	text null=false gen=	
COL	file_chunks.content_tsv	tsvector null=true gen=s	to_tsvector('simple'::regconfig, content)
COL	file_chunks.created_at	timestamp with time zone null=false gen=	now()
COL	file_chunks.embedding	halfvec(768) null=true gen=	
COL	file_chunks.embedding_model	text null=true gen=	
COL	file_chunks.file_id	uuid null=false gen=	
COL	file_chunks.id	uuid null=false gen=	gen_random_uuid()
COL	file_chunks.page_number	integer null=false gen=	
COL	file_chunks.user_id	uuid null=false gen=	
COL	file_chunks.version	integer null=false gen=	
COL	file_index_state.chunk_count	integer null=false gen=	0
COL	file_index_state.error	text null=true gen=	
COL	file_index_state.file_id	uuid null=false gen=	
COL	file_index_state.status	text null=false gen=	'pending'::text
COL	file_index_state.updated_at	timestamp with time zone null=false gen=	now()
COL	file_index_state.user_id	uuid null=false gen=	
COL	file_rag_admin_settings.chunk_chars	integer null=false gen=	1200
COL	file_rag_admin_settings.chunk_overlap_chars	integer null=false gen=	200
COL	file_rag_admin_settings.cosine_threshold	real null=false gen=	0.6
COL	file_rag_admin_settings.default_top_k	smallint null=false gen=	8
COL	file_rag_admin_settings.embedding_dimensions	integer null=false gen=	768
COL	file_rag_admin_settings.embedding_model_id	uuid null=true gen=	
COL	file_rag_admin_settings.enabled	boolean null=false gen=	true
COL	file_rag_admin_settings.fts_candidate_multiplier	integer null=false gen=	4
COL	file_rag_admin_settings.fts_dictionary	text null=false gen=	'simple'::text
COL	file_rag_admin_settings.fts_enabled	boolean null=false gen=	true
COL	file_rag_admin_settings.fts_min_rank	real null=false gen=	0.0
COL	file_rag_admin_settings.fts_rrf_k	integer null=false gen=	60
COL	file_rag_admin_settings.id	smallint null=false gen=	1
COL	file_rag_admin_settings.kb_max_documents	integer null=false gen=	2000
COL	file_rag_admin_settings.max_chunks_per_file	integer null=false gen=	5000
COL	file_rag_admin_settings.rerank_candidate_k	integer null=false gen=	30
COL	file_rag_admin_settings.rerank_enabled	boolean null=false gen=	false
COL	file_rag_admin_settings.reranker_model_id	uuid null=true gen=	
COL	file_rag_admin_settings.search_max_hit_chars	integer null=false gen=	2000
COL	file_rag_admin_settings.search_max_top_k	smallint null=false gen=	50
COL	file_rag_admin_settings.search_snippet_chars	integer null=false gen=	160
COL	file_rag_admin_settings.semantic_enabled	boolean null=false gen=	true
COL	file_rag_admin_settings.updated_at	timestamp with time zone null=false gen=	now()
COL	file_versions.blob_version_id	uuid null=false gen=	
COL	file_versions.checksum	character varying(64) null=true gen=	
COL	file_versions.created_at	timestamp with time zone null=false gen=	now()
COL	file_versions.created_by	character varying(10) null=false gen=	
COL	file_versions.file_id	uuid null=false gen=	
COL	file_versions.file_size	bigint null=false gen=	
COL	file_versions.has_thumbnail	boolean null=false gen=	false
COL	file_versions.id	uuid null=false gen=	
COL	file_versions.is_head	boolean null=false gen=	false
COL	file_versions.mime_type	character varying(100) null=true gen=	
COL	file_versions.preview_page_count	integer null=false gen=	0
COL	file_versions.processing_metadata	jsonb null=false gen=	'{}'::jsonb
COL	file_versions.source_message_id	uuid null=true gen=	
COL	file_versions.text_page_count	integer null=false gen=	0
COL	file_versions.version	integer null=false gen=	
COL	files.checksum	character varying(64) null=true gen=	
COL	files.created_at	timestamp with time zone null=false gen=	now()
COL	files.created_by	character varying(10) null=false gen=	'user'::character varying
COL	files.current_version_id	uuid null=false gen=	
COL	files.file_size	bigint null=false gen=	
COL	files.filename	character varying(255) null=false gen=	
COL	files.has_thumbnail	boolean null=false gen=	false
COL	files.id	uuid null=false gen=	gen_random_uuid()
COL	files.mime_type	character varying(100) null=true gen=	
COL	files.preview_page_count	integer null=false gen=	0
COL	files.processing_metadata	jsonb null=true gen=	'{}'::jsonb
COL	files.text_page_count	integer null=false gen=	0
COL	files.updated_at	timestamp with time zone null=false gen=	now()
COL	files.user_id	uuid null=false gen=	
COL	files.workflow_run_id	uuid null=true gen=	
COL	group_skills.assigned_at	timestamp with time zone null=false gen=	now()
COL	group_skills.group_id	uuid null=false gen=	
COL	group_skills.skill_id	uuid null=false gen=	
COL	group_workflows.assigned_at	timestamp with time zone null=false gen=	now()
COL	group_workflows.group_id	uuid null=false gen=	
COL	group_workflows.workflow_id	uuid null=false gen=	
COL	groups.created_at	timestamp with time zone null=false gen=	now()
COL	groups.description	text null=true gen=	
COL	groups.id	uuid null=false gen=	gen_random_uuid()
COL	groups.is_active	boolean null=false gen=	true
COL	groups.is_default	boolean null=false gen=	false
COL	groups.is_system	boolean null=false gen=	false
COL	groups.name	character varying(100) null=false gen=	
COL	groups.permissions	text[] null=false gen=	'{}'::text[]
COL	groups.updated_at	timestamp with time zone null=false gen=	now()
COL	hub_entities.created_at	timestamp with time zone null=false gen=	now()
COL	hub_entities.created_by	uuid null=true gen=	
COL	hub_entities.entity_id	uuid null=false gen=	
COL	hub_entities.entity_type	character varying(50) null=false gen=	
COL	hub_entities.hub_category	character varying(50) null=false gen=	
COL	hub_entities.hub_id	character varying(255) null=false gen=	
COL	hub_entities.hub_version	character varying(32) null=true gen=	
COL	hub_entities.id	uuid null=false gen=	gen_random_uuid()
COL	hub_settings.id	boolean null=false gen=	true
COL	hub_settings.pinned_version	character varying(32) null=true gen=	
COL	hub_settings.updated_at	timestamp with time zone null=false gen=	now()
COL	js_tool_settings.approval_timeout_secs	integer null=false gen=	300
COL	js_tool_settings.created_at	timestamp with time zone null=false gen=	now()
COL	js_tool_settings.id	boolean null=false gen=	true
COL	js_tool_settings.max_concurrent_dispatch	integer null=false gen=	6
COL	js_tool_settings.max_concurrent_runs	integer null=false gen=	8
COL	js_tool_settings.max_stack_bytes	bigint null=false gen=	524288
COL	js_tool_settings.max_trace_entries	integer null=false gen=	256
COL	js_tool_settings.memory_bytes	bigint null=false gen=	134217728
COL	js_tool_settings.updated_at	timestamp with time zone null=false gen=	now()
COL	js_tool_settings.wall_secs	integer null=false gen=	300
COL	knowledge_base_documents.added_at	timestamp with time zone null=false gen=	now()
COL	knowledge_base_documents.file_id	uuid null=false gen=	
COL	knowledge_base_documents.knowledge_base_id	uuid null=false gen=	
COL	knowledge_bases.created_at	timestamp with time zone null=false gen=	now()
COL	knowledge_bases.description	text null=true gen=	
COL	knowledge_bases.id	uuid null=false gen=	gen_random_uuid()
COL	knowledge_bases.name	text null=false gen=	
COL	knowledge_bases.updated_at	timestamp with time zone null=false gen=	now()
COL	knowledge_bases.user_id	uuid null=false gen=	
COL	lit_fulltext_cache.arxiv_id	text null=true gen=	
COL	lit_fulltext_cache.byte_size	bigint null=false gen=	0
COL	lit_fulltext_cache.content_hash	text null=true gen=	
COL	lit_fulltext_cache.doi	text null=true gen=	
COL	lit_fulltext_cache.fetched_at	timestamp with time zone null=false gen=	now()
COL	lit_fulltext_cache.id	bigint null=false gen=	nextval('lit_fulltext_cache_id_seq'::regclass)
COL	lit_fulltext_cache.last_accessed_at	timestamp with time zone null=false gen=	now()
COL	lit_fulltext_cache.license	text null=true gen=	
COL	lit_fulltext_cache.pmcid	text null=true gen=	
COL	lit_fulltext_cache.pmid	text null=true gen=	
COL	lit_fulltext_cache.source	text null=true gen=	
COL	lit_fulltext_cache.status	text null=false gen=	
COL	lit_fulltext_cache.version	text null=true gen=	
COL	lit_search_connectors.api_key	text null=true gen=	
COL	lit_search_connectors.api_key_encrypted	bytea null=true gen=	
COL	lit_search_connectors.config	jsonb null=false gen=	'{}'::jsonb
COL	lit_search_connectors.connector	text null=false gen=	
COL	lit_search_connectors.created_at	timestamp with time zone null=false gen=	now()
COL	lit_search_connectors.updated_at	timestamp with time zone null=false gen=	now()
COL	lit_search_settings.completeness_estimate_enabled	boolean null=false gen=	true
COL	lit_search_settings.created_at	timestamp with time zone null=false gen=	now()
COL	lit_search_settings.enabled	boolean null=false gen=	true
COL	lit_search_settings.enabled_connectors	text[] null=false gen=	ARRAY['europepmc'::text, 'crossref'::text, 'semanticscholar'::text, 'pubmed'::text, 'arxiv'::text]
COL	lit_search_settings.id	boolean null=false gen=	true
COL	lit_search_settings.max_results	integer null=false gen=	25
COL	lit_search_settings.per_source_limit	integer null=false gen=	50
COL	lit_search_settings.request_timeout_secs	integer null=false gen=	30
COL	lit_search_settings.updated_at	timestamp with time zone null=false gen=	now()
COL	llm_model_files.file_path	character varying(1000) null=false gen=	
COL	llm_model_files.file_size_bytes	bigint null=false gen=	
COL	llm_model_files.file_type	character varying(50) null=false gen=	
COL	llm_model_files.filename	character varying(500) null=false gen=	
COL	llm_model_files.id	uuid null=false gen=	gen_random_uuid()
COL	llm_model_files.model_id	uuid null=false gen=	
COL	llm_model_files.upload_status	character varying(50) null=false gen=	'pending'::character varying
COL	llm_model_files.uploaded_at	timestamp with time zone null=false gen=	now()
COL	llm_models.capabilities	jsonb null=true gen=	'{}'::jsonb
COL	llm_models.created_at	timestamp with time zone null=false gen=	CURRENT_TIMESTAMP
COL	llm_models.description	text null=true gen=	
COL	llm_models.display_name	character varying(255) null=false gen=	
COL	llm_models.enabled	boolean null=false gen=	true
COL	llm_models.engine_settings	jsonb null=true gen=	
COL	llm_models.engine_type	character varying(50) null=false gen=	'mistralrs'::character varying
COL	llm_models.file_format	character varying(20) null=false gen=	'safetensors'::character varying
COL	llm_models.file_size_bytes	bigint null=true gen=	
COL	llm_models.id	uuid null=false gen=	gen_random_uuid()
COL	llm_models.is_active	boolean null=false gen=	false
COL	llm_models.is_deprecated	boolean null=false gen=	false
COL	llm_models.name	character varying(255) null=false gen=	
COL	llm_models.parameters	jsonb null=true gen=	'{}'::jsonb
COL	llm_models.pid	integer null=true gen=	
COL	llm_models.port	integer null=true gen=	
COL	llm_models.provider_id	uuid null=false gen=	
COL	llm_models.required_runtime_version_id	uuid null=true gen=	
COL	llm_models.updated_at	timestamp with time zone null=false gen=	CURRENT_TIMESTAMP
COL	llm_models.validation_issues	jsonb null=true gen=	
COL	llm_models.validation_status	character varying(50) null=true gen=	
COL	llm_provider_files.created_at	timestamp with time zone null=false gen=	now()
COL	llm_provider_files.file_id	uuid null=false gen=	
COL	llm_provider_files.id	uuid null=false gen=	gen_random_uuid()
COL	llm_provider_files.provider_file_id	character varying(512) null=true gen=	
COL	llm_provider_files.provider_id	uuid null=false gen=	
COL	llm_provider_files.provider_metadata	jsonb null=false gen=	'{}'::jsonb
COL	llm_provider_files.updated_at	timestamp with time zone null=false gen=	now()
COL	llm_provider_files.upload_status	character varying(50) null=false gen=	'pending'::character varying
COL	llm_providers.api_key	text null=true gen=	
COL	llm_providers.api_key_encrypted	bytea null=true gen=	
COL	llm_providers.base_url	character varying(512) null=true gen=	
COL	llm_providers.built_in	boolean null=false gen=	false
COL	llm_providers.created_at	timestamp with time zone null=false gen=	CURRENT_TIMESTAMP
COL	llm_providers.default_runtime_version_id	uuid null=true gen=	
COL	llm_providers.deployment_config	jsonb null=true gen=	'{"type": "local", "binary_path": null}'::jsonb
COL	llm_providers.enabled	boolean null=false gen=	false
COL	llm_providers.id	uuid null=false gen=	gen_random_uuid()
COL	llm_providers.name	character varying(255) null=false gen=	
COL	llm_providers.provider_type	character varying(50) null=false gen=	
COL	llm_providers.proxy_settings	jsonb null=true gen=	'{}'::jsonb
COL	llm_providers.updated_at	timestamp with time zone null=false gen=	CURRENT_TIMESTAMP
COL	llm_repositories.auth_config	jsonb null=true gen=	'{}'::jsonb
COL	llm_repositories.auth_config_encrypted	bytea null=true gen=	
COL	llm_repositories.auth_type	character varying(50) null=false gen=	
COL	llm_repositories.built_in	boolean null=false gen=	false
COL	llm_repositories.created_at	timestamp with time zone null=false gen=	CURRENT_TIMESTAMP
COL	llm_repositories.enabled	boolean null=false gen=	true
COL	llm_repositories.id	uuid null=false gen=	gen_random_uuid()
COL	llm_repositories.last_health_check_at	timestamp with time zone null=true gen=	
COL	llm_repositories.last_health_check_reason	text null=true gen=	
COL	llm_repositories.last_health_check_status	text null=false gen=	'untested'::text
COL	llm_repositories.name	character varying(255) null=false gen=	
COL	llm_repositories.updated_at	timestamp with time zone null=false gen=	CURRENT_TIMESTAMP
COL	llm_repositories.url	character varying(512) null=false gen=	
COL	llm_runtime_instances.base_url	character varying(512) null=false gen=	
COL	llm_runtime_instances.error_message	text null=true gen=	
COL	llm_runtime_instances.id	uuid null=false gen=	gen_random_uuid()
COL	llm_runtime_instances.last_failure_reason	text null=true gen=	
COL	llm_runtime_instances.last_health_check	timestamp with time zone null=true gen=	
COL	llm_runtime_instances.last_used_at	timestamp with time zone null=false gen=	now()
COL	llm_runtime_instances.local_port	integer null=false gen=	
COL	llm_runtime_instances.model_id	uuid null=false gen=	
COL	llm_runtime_instances.provider_id	uuid null=false gen=	
COL	llm_runtime_instances.restart_attempts	integer null=false gen=	0
COL	llm_runtime_instances.runtime_version_id	uuid null=true gen=	
COL	llm_runtime_instances.started_at	timestamp with time zone null=false gen=	now()
COL	llm_runtime_instances.state	character varying(50) null=false gen=	'starting'::character varying
COL	llm_runtime_instances.state_changed_at	timestamp with time zone null=false gen=	now()
COL	llm_runtime_instances.status	character varying(50) null=false gen=	
COL	llm_runtime_instances.stopped_at	timestamp with time zone null=true gen=	
COL	llm_runtime_settings.auto_start_timeout_secs	integer null=false gen=	30
COL	llm_runtime_settings.created_at	timestamp with time zone null=false gen=	now()
COL	llm_runtime_settings.drain_timeout_secs	integer null=false gen=	30
COL	llm_runtime_settings.id	boolean null=false gen=	true
COL	llm_runtime_settings.idle_unload_secs	integer null=false gen=	1800
COL	llm_runtime_settings.updated_at	timestamp with time zone null=false gen=	now()
COL	llm_runtime_versions.arch	character varying(50) null=false gen=	
COL	llm_runtime_versions.backend	character varying(50) null=false gen=	
COL	llm_runtime_versions.binary_path	text null=false gen=	
COL	llm_runtime_versions.created_at	timestamp with time zone null=false gen=	now()
COL	llm_runtime_versions.engine	character varying(50) null=false gen=	
COL	llm_runtime_versions.id	uuid null=false gen=	gen_random_uuid()
COL	llm_runtime_versions.is_system_default	boolean null=false gen=	false
COL	llm_runtime_versions.platform	character varying(50) null=false gen=	
COL	llm_runtime_versions.version	character varying(100) null=false gen=	
COL	mcp_server_oauth_configs.client_id	text null=false gen=	
COL	mcp_server_oauth_configs.client_secret	text null=true gen=	
COL	mcp_server_oauth_configs.client_secret_encrypted	bytea null=true gen=	
COL	mcp_server_oauth_configs.created_at	timestamp with time zone null=false gen=	now()
COL	mcp_server_oauth_configs.resource	text null=true gen=	
COL	mcp_server_oauth_configs.scopes	text null=true gen=	
COL	mcp_server_oauth_configs.server_id	uuid null=false gen=	
COL	mcp_server_oauth_configs.updated_at	timestamp with time zone null=false gen=	now()
COL	mcp_servers.args	jsonb null=true gen=	'[]'::jsonb
COL	mcp_servers.command	text null=true gen=	
COL	mcp_servers.created_at	timestamp with time zone null=false gen=	now()
COL	mcp_servers.description	text null=true gen=	
COL	mcp_servers.display_name	character varying(255) null=false gen=	
COL	mcp_servers.enabled	boolean null=false gen=	true
COL	mcp_servers.environment_variables	jsonb null=true gen=	'{}'::jsonb
COL	mcp_servers.environment_variables_encrypted	jsonb null=false gen=	'{}'::jsonb
COL	mcp_servers.environment_variables_secret_keys	text[] null=false gen=	'{}'::text[]
COL	mcp_servers.headers	jsonb null=true gen=	'{}'::jsonb
COL	mcp_servers.headers_encrypted	jsonb null=false gen=	'{}'::jsonb
COL	mcp_servers.headers_secret_keys	text[] null=false gen=	'{}'::text[]
COL	mcp_servers.id	uuid null=false gen=	gen_random_uuid()
COL	mcp_servers.is_built_in	boolean null=false gen=	false
COL	mcp_servers.is_system	boolean null=false gen=	false
COL	mcp_servers.last_health_check_at	timestamp with time zone null=true gen=	
COL	mcp_servers.last_health_check_reason	text null=true gen=	
COL	mcp_servers.last_health_check_status	text null=false gen=	'untested'::text
COL	mcp_servers.max_concurrent_sessions	integer null=true gen=	
COL	mcp_servers.name	character varying(255) null=false gen=	
COL	mcp_servers.run_in_sandbox	boolean null=false gen=	false
COL	mcp_servers.sandbox_flavor	character varying(32) null=false gen=	'full'::character varying
COL	mcp_servers.supports_sampling	boolean null=false gen=	false
COL	mcp_servers.timeout_seconds	integer null=false gen=	30
COL	mcp_servers.transport_type	character varying(50) null=false gen=	'stdio'::character varying
COL	mcp_servers.updated_at	timestamp with time zone null=false gen=	now()
COL	mcp_servers.url	text null=true gen=	
COL	mcp_servers.usage_mode	character varying(50) null=false gen=	'auto'::character varying
COL	mcp_servers.user_id	uuid null=true gen=	
COL	mcp_settings.approval_mode	character varying(50) null=false gen=	'manual_approve'::character varying
COL	mcp_settings.auto_approved_tools	jsonb null=false gen=	'[]'::jsonb
COL	mcp_settings.conversation_id	uuid null=true gen=	
COL	mcp_settings.created_at	timestamp with time zone null=false gen=	now()
COL	mcp_settings.disabled_servers	jsonb null=false gen=	'[]'::jsonb
COL	mcp_settings.id	uuid null=false gen=	gen_random_uuid()
COL	mcp_settings.loop_settings	jsonb null=true gen=	
COL	mcp_settings.project_id	uuid null=true gen=	
COL	mcp_settings.updated_at	timestamp with time zone null=false gen=	now()
COL	mcp_settings.user_id	uuid null=false gen=	
COL	mcp_tool_calls.arguments_json	jsonb null=false gen=	'{}'::jsonb
COL	mcp_tool_calls.branch_id	uuid null=true gen=	
COL	mcp_tool_calls.content_kinds	text[] null=false gen=	'{}'::text[]
COL	mcp_tool_calls.conversation_id	uuid null=true gen=	
COL	mcp_tool_calls.created_at	timestamp with time zone null=false gen=	now()
COL	mcp_tool_calls.duration_ms	bigint null=true gen=	
COL	mcp_tool_calls.error_message	text null=true gen=	
COL	mcp_tool_calls.finished_at	timestamp with time zone null=true gen=	
COL	mcp_tool_calls.id	uuid null=false gen=	gen_random_uuid()
COL	mcp_tool_calls.is_built_in	boolean null=false gen=	false
COL	mcp_tool_calls.is_error	boolean null=false gen=	false
COL	mcp_tool_calls.message_id	uuid null=true gen=	
COL	mcp_tool_calls.result_bytes	bigint null=false gen=	0
COL	mcp_tool_calls.result_json	jsonb null=true gen=	
COL	mcp_tool_calls.server_id	uuid null=true gen=	
COL	mcp_tool_calls.server_name	character varying(255) null=false gen=	
COL	mcp_tool_calls.source	character varying(20) null=false gen=	'chat'::character varying
COL	mcp_tool_calls.started_at	timestamp with time zone null=false gen=	now()
COL	mcp_tool_calls.status	character varying(20) null=false gen=	'completed'::character varying
COL	mcp_tool_calls.tool_name	character varying(255) null=false gen=	
COL	mcp_tool_calls.tool_use_id	character varying(255) null=true gen=	
COL	mcp_tool_calls.updated_at	timestamp with time zone null=false gen=	now()
COL	mcp_tool_calls.user_id	uuid null=false gen=	
COL	mcp_tool_calls.workflow_run_id	uuid null=true gen=	
COL	mcp_user_policy.allowed_transports	text[] null=false gen=	ARRAY['http'::text, 'stdio'::text]
COL	mcp_user_policy.id	integer null=false gen=	1
COL	mcp_user_policy.tool_call_retention_days	integer null=false gen=	90
COL	mcp_user_policy.updated_at	timestamp with time zone null=false gen=	now()
COL	mcp_user_policy.updated_by	uuid null=true gen=	
COL	mcp_user_policy.user_stdio_sandbox_flavor	text null=true gen=	'full'::text
COL	memory_admin_settings.cosine_threshold	real null=false gen=	0.6
COL	memory_admin_settings.daily_extraction_quota	integer null=false gen=	200
COL	memory_admin_settings.default_extraction_model_id	uuid null=true gen=	
COL	memory_admin_settings.default_top_k	smallint null=false gen=	8
COL	memory_admin_settings.embedding_dimensions	integer null=false gen=	768
COL	memory_admin_settings.embedding_model_id	uuid null=true gen=	
COL	memory_admin_settings.enabled	boolean null=false gen=	true
COL	memory_admin_settings.fts_candidate_multiplier	integer null=false gen=	4
COL	memory_admin_settings.fts_dictionary	text null=false gen=	'simple'::text
COL	memory_admin_settings.fts_enabled	boolean null=false gen=	true
COL	memory_admin_settings.fts_min_rank	real null=false gen=	0.0
COL	memory_admin_settings.fts_rebuild_completed_at	timestamp with time zone null=true gen=	
COL	memory_admin_settings.fts_rebuild_started_at	timestamp with time zone null=true gen=	
COL	memory_admin_settings.fts_rrf_k	integer null=false gen=	60
COL	memory_admin_settings.id	smallint null=false gen=	1
COL	memory_admin_settings.semantic_enabled	boolean null=false gen=	true
COL	memory_admin_settings.soft_delete_grace_days	integer null=false gen=	30
COL	memory_admin_settings.updated_at	timestamp with time zone null=false gen=	now()
COL	memory_audit_log.actor_kind	text null=false gen=	'user'::text
COL	memory_audit_log.content_snapshot	text null=true gen=	
COL	memory_audit_log.created_at	timestamp with time zone null=false gen=	now()
COL	memory_audit_log.id	bigint null=false gen=	nextval('memory_audit_log_id_seq'::regclass)
COL	memory_audit_log.memory_id	uuid null=true gen=	
COL	memory_audit_log.metadata	jsonb null=false gen=	'{}'::jsonb
COL	memory_audit_log.op	text null=false gen=	
COL	memory_audit_log.source	text null=false gen=	
COL	memory_audit_log.user_id	uuid null=false gen=	
COL	message_assistant.assistant_id	uuid null=false gen=	
COL	message_assistant.message_id	uuid null=false gen=	
COL	message_contents.content	jsonb null=false gen=	
COL	message_contents.content_type	character varying(50) null=false gen=	
COL	message_contents.created_at	timestamp with time zone null=false gen=	now()
COL	message_contents.id	uuid null=false gen=	gen_random_uuid()
COL	message_contents.message_id	uuid null=false gen=	
COL	message_contents.sequence_order	integer null=false gen=	0
COL	message_contents.updated_at	timestamp with time zone null=false gen=	now()
COL	message_mcp_servers.message_id	uuid null=false gen=	
COL	message_mcp_servers.server_id	uuid null=false gen=	
COL	messages.created_at	timestamp with time zone null=false gen=	now()
COL	messages.edit_count	integer null=false gen=	0
COL	messages.id	uuid null=false gen=	gen_random_uuid()
COL	messages.model_id	uuid null=true gen=	
COL	messages.originated_from_id	uuid null=false gen=	
COL	messages.role	character varying(20) null=false gen=	
COL	notifications.body	text null=false gen=	''::text
COL	notifications.conversation_id	uuid null=true gen=	
COL	notifications.created_at	timestamp with time zone null=false gen=	now()
COL	notifications.id	uuid null=false gen=	gen_random_uuid()
COL	notifications.interrupt	boolean null=false gen=	true
COL	notifications.kind	text null=false gen=	
COL	notifications.read_at	timestamp with time zone null=true gen=	
COL	notifications.scheduled_task_id	uuid null=true gen=	
COL	notifications.title	text null=false gen=	
COL	notifications.user_id	uuid null=false gen=	
COL	notifications.workflow_run_id	uuid null=true gen=	
COL	oauth_sessions.created_at	timestamp with time zone null=false gen=	now()
COL	oauth_sessions.expires_at	timestamp with time zone null=false gen=	
COL	oauth_sessions.id	uuid null=false gen=	gen_random_uuid()
COL	oauth_sessions.nonce	character varying(255) null=true gen=	
COL	oauth_sessions.pkce_verifier	character varying(255) null=true gen=	
COL	oauth_sessions.provider_id	uuid null=false gen=	
COL	oauth_sessions.redirect_uri	text null=false gen=	
COL	oauth_sessions.return_to	text null=true gen=	
COL	oauth_sessions.state	character varying(255) null=false gen=	
COL	pending_account_links.attempts	integer null=false gen=	0
COL	pending_account_links.created_at	timestamp with time zone null=false gen=	now()
COL	pending_account_links.expires_at	timestamp with time zone null=false gen=	
COL	pending_account_links.external_data	jsonb null=true gen=	
COL	pending_account_links.external_email	character varying(255) null=true gen=	
COL	pending_account_links.external_id	character varying(255) null=false gen=	
COL	pending_account_links.link_token	character varying(255) null=false gen=	
COL	pending_account_links.provider_id	uuid null=false gen=	
COL	pending_account_links.target_user_id	uuid null=false gen=	
COL	project_bibliography.added_at	timestamp with time zone null=false gen=	now()
COL	project_bibliography.entry_id	uuid null=false gen=	
COL	project_bibliography.project_id	uuid null=false gen=	
COL	project_conversations.attached_at	timestamp with time zone null=false gen=	now()
COL	project_conversations.conversation_id	uuid null=false gen=	
COL	project_conversations.project_id	uuid null=false gen=	
COL	project_files.added_at	timestamp with time zone null=false gen=	now()
COL	project_files.file_id	uuid null=false gen=	
COL	project_files.project_id	uuid null=false gen=	
COL	project_knowledge_bases.added_at	timestamp with time zone null=false gen=	now()
COL	project_knowledge_bases.knowledge_base_id	uuid null=false gen=	
COL	project_knowledge_bases.project_id	uuid null=false gen=	
COL	projects.created_at	timestamp with time zone null=false gen=	now()
COL	projects.default_assistant_id	uuid null=true gen=	
COL	projects.default_model_id	uuid null=true gen=	
COL	projects.description	text null=true gen=	
COL	projects.id	uuid null=false gen=	gen_random_uuid()
COL	projects.instructions	text null=true gen=	
COL	projects.name	character varying(255) null=false gen=	
COL	projects.updated_at	timestamp with time zone null=false gen=	now()
COL	projects.user_id	uuid null=false gen=	
COL	refresh_tokens.expires_at	timestamp with time zone null=false gen=	
COL	refresh_tokens.issued_at	timestamp with time zone null=false gen=	now()
COL	refresh_tokens.jti	uuid null=false gen=	
COL	refresh_tokens.revoked_at	timestamp with time zone null=true gen=	
COL	refresh_tokens.rotated_to	uuid null=true gen=	
COL	refresh_tokens.user_id	uuid null=false gen=	
COL	sandbox_workspace_files.base_version_id	uuid null=false gen=	
COL	sandbox_workspace_files.conversation_id	uuid null=false gen=	
COL	sandbox_workspace_files.file_id	uuid null=false gen=	
COL	sandbox_workspace_files.workspace_relpath	text null=false gen=	
COL	scheduled_task_runs.change_summary_json	jsonb null=true gen=	
COL	scheduled_task_runs.conversation_id	uuid null=true gen=	
COL	scheduled_task_runs.error_class	text null=true gen=	
COL	scheduled_task_runs.error_message	text null=true gen=	
COL	scheduled_task_runs.finished_at	timestamp with time zone null=true gen=	
COL	scheduled_task_runs.fired_at	timestamp with time zone null=false gen=	now()
COL	scheduled_task_runs.id	uuid null=false gen=	gen_random_uuid()
COL	scheduled_task_runs.notification_id	uuid null=true gen=	
COL	scheduled_task_runs.result_preview	text null=true gen=	
COL	scheduled_task_runs.scheduled_task_id	uuid null=false gen=	
COL	scheduled_task_runs.skipped_tools	jsonb null=false gen=	'[]'::jsonb
COL	scheduled_task_runs.status	text null=false gen=	
COL	scheduled_task_runs.trigger	text null=false gen=	'schedule'::text
COL	scheduled_task_runs.user_id	uuid null=false gen=	
COL	scheduled_task_runs.workflow_run_id	uuid null=true gen=	
COL	scheduled_tasks.allowed_unattended_tools	jsonb null=false gen=	'[]'::jsonb
COL	scheduled_tasks.assistant_id	uuid null=true gen=	
COL	scheduled_tasks.bound_conversation_id	uuid null=true gen=	
COL	scheduled_tasks.consecutive_failures	integer null=false gen=	0
COL	scheduled_tasks.created_at	timestamp with time zone null=false gen=	now()
COL	scheduled_tasks.cron_expr	text null=true gen=	
COL	scheduled_tasks.enabled	boolean null=false gen=	true
COL	scheduled_tasks.id	uuid null=false gen=	gen_random_uuid()
COL	scheduled_tasks.inputs_json	jsonb null=false gen=	'{}'::jsonb
COL	scheduled_tasks.last_result_fingerprint	text null=true gen=	
COL	scheduled_tasks.last_result_signature_json	jsonb null=true gen=	
COL	scheduled_tasks.last_run_at	timestamp with time zone null=true gen=	
COL	scheduled_tasks.last_status	text null=true gen=	
COL	scheduled_tasks.model_id	uuid null=true gen=	
COL	scheduled_tasks.name	character varying(255) null=false gen=	
COL	scheduled_tasks.next_run_at	timestamp with time zone null=true gen=	
COL	scheduled_tasks.notify_mode	text null=false gen=	'always'::text
COL	scheduled_tasks.notify_on	text null=false gen=	'always'::text
COL	scheduled_tasks.paused_reason	text null=true gen=	
COL	scheduled_tasks.prompt	text null=true gen=	
COL	scheduled_tasks.run_at	timestamp with time zone null=true gen=	
COL	scheduled_tasks.schedule_kind	text null=false gen=	
COL	scheduled_tasks.target_kind	text null=false gen=	
COL	scheduled_tasks.timezone	text null=false gen=	'UTC'::text
COL	scheduled_tasks.updated_at	timestamp with time zone null=false gen=	now()
COL	scheduled_tasks.user_id	uuid null=false gen=	
COL	scheduled_tasks.workflow_id	uuid null=true gen=	
COL	scheduler_admin_settings.id	boolean null=false gen=	true
COL	scheduler_admin_settings.max_active_tasks_per_user	integer null=false gen=	20
COL	scheduler_admin_settings.max_consecutive_failures	integer null=false gen=	5
COL	scheduler_admin_settings.min_interval_seconds	integer null=false gen=	300
COL	scheduler_admin_settings.notification_retention_days	integer null=false gen=	30
COL	scheduler_admin_settings.updated_at	timestamp with time zone null=false gen=	now()
COL	session_settings.access_token_expiry_hours	integer null=false gen=	24
COL	session_settings.id	boolean null=false gen=	true
COL	session_settings.refresh_token_expiry_days	integer null=false gen=	30
COL	session_settings.seeded_from_config	boolean null=false gen=	false
COL	session_settings.updated_at	timestamp with time zone null=false gen=	now()
COL	skills.bundle_sha256	text null=false gen=	
COL	skills.bundle_size_bytes	bigint null=false gen=	
COL	skills.created_at	timestamp with time zone null=false gen=	now()
COL	skills.created_by	uuid null=true gen=	
COL	skills.description	text null=true gen=	
COL	skills.display_name	text null=true gen=	
COL	skills.enabled	boolean null=false gen=	true
COL	skills.entry_point	text null=false gen=	
COL	skills.extracted_path	text null=false gen=	
COL	skills.file_count	integer null=false gen=	
COL	skills.frontmatter_json	jsonb null=false gen=	'{}'::jsonb
COL	skills.id	uuid null=false gen=	gen_random_uuid()
COL	skills.is_dev	boolean null=false gen=	false
COL	skills.name	text null=false gen=	
COL	skills.owner_user_id	uuid null=true gen=	
COL	skills.scope	character varying(10) null=false gen=	'user'::character varying
COL	skills.tags	jsonb null=false gen=	'[]'::jsonb
COL	skills.updated_at	timestamp with time zone null=false gen=	now()
COL	skills.version	text null=true gen=	
COL	skills.when_to_use	text null=true gen=	
COL	summarization_admin_settings.default_summarization_model_id	uuid null=true gen=	
COL	summarization_admin_settings.enabled	boolean null=false gen=	true
COL	summarization_admin_settings.full_summary_prompt	text null=true gen=	
COL	summarization_admin_settings.id	smallint null=false gen=	1
COL	summarization_admin_settings.incremental_summary_prompt	text null=true gen=	
COL	summarization_admin_settings.summarize_after_tokens	integer null=false gen=	12000
COL	summarization_admin_settings.summarizer_keep_recent_tokens	integer null=false gen=	3000
COL	summarization_admin_settings.updated_at	timestamp with time zone null=false gen=	now()
COL	tool_use_approvals.approval_note	text null=true gen=	
COL	tool_use_approvals.approved_at	timestamp with time zone null=true gen=	
COL	tool_use_approvals.approved_by	uuid null=true gen=	
COL	tool_use_approvals.branch_id	uuid null=false gen=	
COL	tool_use_approvals.conversation_id	uuid null=false gen=	
COL	tool_use_approvals.created_at	timestamp with time zone null=false gen=	now()
COL	tool_use_approvals.id	uuid null=false gen=	gen_random_uuid()
COL	tool_use_approvals.message_id	uuid null=false gen=	
COL	tool_use_approvals.server_id	uuid null=true gen=	
COL	tool_use_approvals.server_name	character varying(255) null=false gen=	
COL	tool_use_approvals.status	character varying(50) null=false gen=	'pending'::character varying
COL	tool_use_approvals.tool_input	jsonb null=false gen=	
COL	tool_use_approvals.tool_name	character varying(255) null=false gen=	
COL	tool_use_approvals.tool_use_id	character varying(255) null=false gen=	
COL	tool_use_approvals.updated_at	timestamp with time zone null=false gen=	now()
COL	tool_use_approvals.user_id	uuid null=false gen=	
COL	user_auth_links.created_at	timestamp with time zone null=false gen=	now()
COL	user_auth_links.external_data	jsonb null=true gen=	
COL	user_auth_links.external_email	character varying(255) null=true gen=	
COL	user_auth_links.external_id	character varying(255) null=false gen=	
COL	user_auth_links.id	uuid null=false gen=	gen_random_uuid()
COL	user_auth_links.last_login_at	timestamp with time zone null=true gen=	
COL	user_auth_links.provider_id	uuid null=false gen=	
COL	user_auth_links.updated_at	timestamp with time zone null=false gen=	now()
COL	user_auth_links.user_id	uuid null=false gen=	
COL	user_group_llm_providers.assigned_at	timestamp with time zone null=false gen=	now()
COL	user_group_llm_providers.group_id	uuid null=false gen=	
COL	user_group_llm_providers.id	uuid null=false gen=	gen_random_uuid()
COL	user_group_llm_providers.provider_id	uuid null=false gen=	
COL	user_group_mcp_servers.assigned_at	timestamp with time zone null=false gen=	now()
COL	user_group_mcp_servers.group_id	uuid null=false gen=	
COL	user_group_mcp_servers.mcp_server_id	uuid null=false gen=	
COL	user_groups.assigned_at	timestamp with time zone null=false gen=	now()
COL	user_groups.assigned_by	uuid null=true gen=	
COL	user_groups.group_id	uuid null=false gen=	
COL	user_groups.user_id	uuid null=false gen=	
COL	user_lit_search_connector_keys.api_key	text null=true gen=	
COL	user_lit_search_connector_keys.api_key_encrypted	bytea null=true gen=	
COL	user_lit_search_connector_keys.connector	text null=false gen=	
COL	user_lit_search_connector_keys.created_at	timestamp with time zone null=false gen=	now()
COL	user_lit_search_connector_keys.id	uuid null=false gen=	gen_random_uuid()
COL	user_lit_search_connector_keys.updated_at	timestamp with time zone null=false gen=	now()
COL	user_lit_search_connector_keys.user_id	uuid null=false gen=	
COL	user_llm_provider_api_keys.api_key	text null=true gen=	
COL	user_llm_provider_api_keys.api_key_encrypted	bytea null=true gen=	
COL	user_llm_provider_api_keys.created_at	timestamp with time zone null=false gen=	now()
COL	user_llm_provider_api_keys.id	uuid null=false gen=	gen_random_uuid()
COL	user_llm_provider_api_keys.provider_id	uuid null=false gen=	
COL	user_llm_provider_api_keys.updated_at	timestamp with time zone null=false gen=	now()
COL	user_llm_provider_api_keys.user_id	uuid null=false gen=	
COL	user_mcp_defaults.approval_mode	character varying(50) null=false gen=	'manual_approve'::character varying
COL	user_mcp_defaults.auto_approved_tools	jsonb null=false gen=	'[]'::jsonb
COL	user_mcp_defaults.created_at	timestamp with time zone null=false gen=	now()
COL	user_mcp_defaults.disabled_servers	jsonb null=false gen=	'[]'::jsonb
COL	user_mcp_defaults.id	uuid null=false gen=	gen_random_uuid()
COL	user_mcp_defaults.loop_settings	jsonb null=true gen=	
COL	user_mcp_defaults.updated_at	timestamp with time zone null=false gen=	now()
COL	user_mcp_defaults.user_id	uuid null=false gen=	
COL	user_memories.confidence	smallint null=false gen=	80
COL	user_memories.content	text null=false gen=	
COL	user_memories.content_tsv	tsvector null=true gen=s	to_tsvector('simple'::regconfig, content)
COL	user_memories.conversation_id	uuid null=true gen=	
COL	user_memories.created_at	timestamp with time zone null=false gen=	now()
COL	user_memories.deleted_at	timestamp with time zone null=true gen=	
COL	user_memories.embedding	halfvec(768) null=true gen=	
COL	user_memories.embedding_model	text null=true gen=	
COL	user_memories.id	uuid null=false gen=	gen_random_uuid()
COL	user_memories.importance	smallint null=false gen=	50
COL	user_memories.kind	text null=false gen=	'fact'::text
COL	user_memories.last_recalled_at	timestamp with time zone null=true gen=	
COL	user_memories.metadata	jsonb null=false gen=	'{}'::jsonb
COL	user_memories.project_id	uuid null=true gen=	
COL	user_memories.recall_count	integer null=false gen=	0
COL	user_memories.scope	text null=false gen=	'user'::text
COL	user_memories.source	text null=false gen=	
COL	user_memories.source_message_id	uuid null=true gen=	
COL	user_memories.updated_at	timestamp with time zone null=false gen=	now()
COL	user_memories.user_id	uuid null=false gen=	
COL	user_memory_settings.created_at	timestamp with time zone null=false gen=	now()
COL	user_memory_settings.extraction_enabled	boolean null=false gen=	false
COL	user_memory_settings.extraction_model_id	uuid null=true gen=	
COL	user_memory_settings.max_memories	integer null=false gen=	1000
COL	user_memory_settings.retention_days	integer null=true gen=	
COL	user_memory_settings.retrieval_enabled	boolean null=false gen=	false
COL	user_memory_settings.updated_at	timestamp with time zone null=false gen=	now()
COL	user_memory_settings.user_id	uuid null=false gen=	
COL	user_onboarding.completed_guide_ids	text[] null=false gen=	'{}'::text[]
COL	user_onboarding.completed_step_ids	text[] null=false gen=	'{}'::text[]
COL	user_onboarding.created_at	timestamp with time zone null=false gen=	now()
COL	user_onboarding.updated_at	timestamp with time zone null=false gen=	now()
COL	user_onboarding.user_id	uuid null=false gen=	
COL	user_web_search_provider_keys.api_key	text null=true gen=	
COL	user_web_search_provider_keys.api_key_encrypted	bytea null=true gen=	
COL	user_web_search_provider_keys.created_at	timestamp with time zone null=false gen=	now()
COL	user_web_search_provider_keys.id	uuid null=false gen=	gen_random_uuid()
COL	user_web_search_provider_keys.provider	text null=false gen=	
COL	user_web_search_provider_keys.updated_at	timestamp with time zone null=false gen=	now()
COL	user_web_search_provider_keys.user_id	uuid null=false gen=	
COL	users.avatar_url	text null=true gen=	
COL	users.created_at	timestamp with time zone null=false gen=	now()
COL	users.display_name	character varying(255) null=true gen=	
COL	users.email	character varying(255) null=false gen=	
COL	users.email_verified	boolean null=false gen=	false
COL	users.id	uuid null=false gen=	gen_random_uuid()
COL	users.is_active	boolean null=false gen=	true
COL	users.is_admin	boolean null=false gen=	false
COL	users.last_login_at	timestamp with time zone null=true gen=	
COL	users.password_changed_at	timestamp with time zone null=true gen=	
COL	users.password_hash	character varying(255) null=true gen=	
COL	users.permissions	text[] null=false gen=	'{}'::text[]
COL	users.updated_at	timestamp with time zone null=false gen=	now()
COL	users.username	character varying(100) null=false gen=	
COL	voice_models.created_at	timestamp with time zone null=false gen=	now()
COL	voice_models.filename	character varying(200) null=false gen=	
COL	voice_models.id	uuid null=false gen=	gen_random_uuid()
COL	voice_models.name	character varying(50) null=false gen=	
COL	voice_models.sha256	character(64) null=true gen=	
COL	voice_models.size_bytes	bigint null=false gen=	0
COL	voice_models.source	character varying(20) null=false gen=	'catalog'::character varying
COL	voice_models.source_url	text null=true gen=	
COL	voice_models.verified	boolean null=false gen=	false
COL	voice_runtime_instance.active_model	character varying(100) null=true gen=	
COL	voice_runtime_instance.base_url	text null=true gen=	
COL	voice_runtime_instance.created_at	timestamp with time zone null=false gen=	now()
COL	voice_runtime_instance.id	boolean null=false gen=	true
COL	voice_runtime_instance.last_failure_reason	text null=true gen=	
COL	voice_runtime_instance.last_used_at	timestamp with time zone null=true gen=	
COL	voice_runtime_instance.local_port	integer null=true gen=	
COL	voice_runtime_instance.restart_attempts	integer null=false gen=	0
COL	voice_runtime_instance.runtime_version_id	uuid null=true gen=	
COL	voice_runtime_instance.state	character varying(30) null=false gen=	'stopped'::character varying
COL	voice_runtime_instance.state_changed_at	timestamp with time zone null=false gen=	now()
COL	voice_runtime_instance.status	character varying(20) null=false gen=	'stopped'::character varying
COL	voice_runtime_instance.updated_at	timestamp with time zone null=false gen=	now()
COL	voice_runtime_settings.auto_start_timeout_secs	integer null=false gen=	60
COL	voice_runtime_settings.created_at	timestamp with time zone null=false gen=	now()
COL	voice_runtime_settings.drain_timeout_secs	integer null=false gen=	30
COL	voice_runtime_settings.enabled	boolean null=false gen=	true
COL	voice_runtime_settings.id	boolean null=false gen=	true
COL	voice_runtime_settings.idle_unload_secs	integer null=false gen=	1800
COL	voice_runtime_settings.language	character varying(20) null=false gen=	'auto'::character varying
COL	voice_runtime_settings.max_clip_seconds	integer null=false gen=	120
COL	voice_runtime_settings.max_upload_bytes	bigint null=false gen=	33554432
COL	voice_runtime_settings.model	character varying(50) null=false gen=	'base'::character varying
COL	voice_runtime_settings.model_source_repo	character varying(200) null=false gen=	'ggerganov/whisper.cpp'::character varying
COL	voice_runtime_settings.stream_interval_ms	integer null=false gen=	1000
COL	voice_runtime_settings.stream_max_decode_secs	integer null=false gen=	30
COL	voice_runtime_settings.streaming_enabled	boolean null=false gen=	true
COL	voice_runtime_settings.updated_at	timestamp with time zone null=false gen=	now()
COL	voice_runtime_versions.arch	character varying(50) null=false gen=	
COL	voice_runtime_versions.backend	character varying(50) null=false gen=	
COL	voice_runtime_versions.binary_path	text null=false gen=	
COL	voice_runtime_versions.created_at	timestamp with time zone null=false gen=	now()
COL	voice_runtime_versions.id	uuid null=false gen=	gen_random_uuid()
COL	voice_runtime_versions.is_system_default	boolean null=false gen=	false
COL	voice_runtime_versions.platform	character varying(50) null=false gen=	
COL	voice_runtime_versions.version	character varying(100) null=false gen=	
COL	web_search_providers.api_key	text null=true gen=	
COL	web_search_providers.api_key_encrypted	bytea null=true gen=	
COL	web_search_providers.config	jsonb null=false gen=	'{}'::jsonb
COL	web_search_providers.created_at	timestamp with time zone null=false gen=	now()
COL	web_search_providers.provider	text null=false gen=	
COL	web_search_providers.updated_at	timestamp with time zone null=false gen=	now()
COL	web_search_settings.created_at	timestamp with time zone null=false gen=	now()
COL	web_search_settings.enabled	boolean null=false gen=	true
COL	web_search_settings.fetch_max_bytes	bigint null=false gen=	5242880
COL	web_search_settings.fetch_max_chars	integer null=false gen=	40000
COL	web_search_settings.id	boolean null=false gen=	true
COL	web_search_settings.max_results	integer null=false gen=	5
COL	web_search_settings.provider_chain	text[] null=false gen=	ARRAY['searxng'::text, 'brave'::text]
COL	web_search_settings.request_timeout_secs	integer null=false gen=	20
COL	web_search_settings.updated_at	timestamp with time zone null=false gen=	now()
COL	workflow_runs.conversation_id	uuid null=true gen=	
COL	workflow_runs.created_at	timestamp with time zone null=false gen=	now()
COL	workflow_runs.current_step	text null=true gen=	
COL	workflow_runs.elicit_response_json	jsonb null=true gen=	
COL	workflow_runs.error_message	text null=true gen=	
COL	workflow_runs.final_output_json	jsonb null=true gen=	
COL	workflow_runs.id	uuid null=false gen=	gen_random_uuid()
COL	workflow_runs.inputs_json	jsonb null=false gen=	'{}'::jsonb
COL	workflow_runs.invocation_source	character varying(20) null=false gen=	'manual'::character varying
COL	workflow_runs.model_id	uuid null=true gen=	
COL	workflow_runs.pending_elicitation_json	jsonb null=true gen=	
COL	workflow_runs.run_kind	character varying(10) null=false gen=	'normal'::character varying
COL	workflow_runs.sandbox_flavor	text null=true gen=	
COL	workflow_runs.status	character varying(50) null=false gen=	'pending'::character varying
COL	workflow_runs.step_artifacts_json	jsonb null=false gen=	'{}'::jsonb
COL	workflow_runs.step_item_progress_json	jsonb null=false gen=	'{}'::jsonb
COL	workflow_runs.step_logs_json	jsonb null=false gen=	'{}'::jsonb
COL	workflow_runs.step_outputs_json	jsonb null=false gen=	'{}'::jsonb
COL	workflow_runs.step_progress_json	jsonb null=true gen=	
COL	workflow_runs.total_tokens	bigint null=false gen=	0
COL	workflow_runs.updated_at	timestamp with time zone null=false gen=	now()
COL	workflow_runs.user_id	uuid null=false gen=	
COL	workflow_runs.workflow_id	uuid null=false gen=	
COL	workflows.bundle_sha256	text null=false gen=	
COL	workflows.bundle_size_bytes	bigint null=false gen=	
COL	workflows.compiled_ir_json	jsonb null=true gen=	
COL	workflows.conversation_id	uuid null=true gen=	
COL	workflows.created_at	timestamp with time zone null=false gen=	now()
COL	workflows.created_by	uuid null=true gen=	
COL	workflows.description	text null=true gen=	
COL	workflows.display_name	text null=true gen=	
COL	workflows.enabled	boolean null=false gen=	true
COL	workflows.entry_point	text null=false gen=	
COL	workflows.ephemeral	boolean null=false gen=	false
COL	workflows.extracted_path	text null=false gen=	
COL	workflows.file_count	integer null=false gen=	
COL	workflows.id	uuid null=false gen=	gen_random_uuid()
COL	workflows.is_dev	boolean null=false gen=	false
COL	workflows.name	text null=false gen=	
COL	workflows.owner_user_id	uuid null=true gen=	
COL	workflows.scope	character varying(10) null=false gen=	'user'::character varying
COL	workflows.tags	jsonb null=false gen=	'[]'::jsonb
COL	workflows.updated_at	timestamp with time zone null=false gen=	now()
COL	workflows.version	text null=true gen=	
CON	assistant_core_memory	c	CHECK (char_limit > 0 AND char_limit <= 50000)
CON	assistant_core_memory	f	FOREIGN KEY (assistant_id) REFERENCES assistants(id) ON DELETE CASCADE
CON	assistant_core_memory	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	assistant_core_memory	n	NOT NULL assistant_id
CON	assistant_core_memory	n	NOT NULL block_label
CON	assistant_core_memory	n	NOT NULL char_limit
CON	assistant_core_memory	n	NOT NULL content
CON	assistant_core_memory	n	NOT NULL created_at
CON	assistant_core_memory	n	NOT NULL id
CON	assistant_core_memory	n	NOT NULL updated_at
CON	assistant_core_memory	n	NOT NULL user_id
CON	assistant_core_memory	p	PRIMARY KEY (id)
CON	assistant_core_memory	u	UNIQUE (assistant_id, user_id, block_label)
CON	assistants	c	CHECK (is_template = true AND created_by IS NULL OR is_template = false)
CON	assistants	f	FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE CASCADE
CON	assistants	n	NOT NULL created_at
CON	assistants	n	NOT NULL enabled
CON	assistants	n	NOT NULL id
CON	assistants	n	NOT NULL is_default
CON	assistants	n	NOT NULL is_template
CON	assistants	n	NOT NULL name
CON	assistants	n	NOT NULL updated_at
CON	assistants	p	PRIMARY KEY (id)
CON	auth_providers	n	NOT NULL config
CON	auth_providers	n	NOT NULL created_at
CON	auth_providers	n	NOT NULL enabled
CON	auth_providers	n	NOT NULL id
CON	auth_providers	n	NOT NULL name
CON	auth_providers	n	NOT NULL provider_type
CON	auth_providers	n	NOT NULL updated_at
CON	auth_providers	p	PRIMARY KEY (id)
CON	auth_providers	u	UNIQUE (name)
CON	bibliography_entries	c	CHECK (verification_status = ANY (ARRAY['unverified'::text, 'verified'::text, 'mismatch'::text, 'not_found'::text]))
CON	bibliography_entries	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	bibliography_entries	n	NOT NULL citation_key
CON	bibliography_entries	n	NOT NULL created_at
CON	bibliography_entries	n	NOT NULL csl_json
CON	bibliography_entries	n	NOT NULL id
CON	bibliography_entries	n	NOT NULL updated_at
CON	bibliography_entries	n	NOT NULL user_id
CON	bibliography_entries	n	NOT NULL verification_status
CON	bibliography_entries	p	PRIMARY KEY (id)
CON	branch_messages	f	FOREIGN KEY (branch_id) REFERENCES branches(id) ON DELETE CASCADE
CON	branch_messages	f	FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
CON	branch_messages	n	NOT NULL branch_id
CON	branch_messages	n	NOT NULL created_at
CON	branch_messages	n	NOT NULL id
CON	branch_messages	n	NOT NULL is_clone
CON	branch_messages	n	NOT NULL message_id
CON	branch_messages	p	PRIMARY KEY (id)
CON	branch_messages	u	UNIQUE (branch_id, message_id)
CON	branches	c	CHECK (fork_level = ANY (ARRAY['user'::text, 'assistant'::text]))
CON	branches	f	FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
CON	branches	f	FOREIGN KEY (parent_branch_id) REFERENCES branches(id) ON DELETE SET NULL
CON	branches	n	NOT NULL conversation_id
CON	branches	n	NOT NULL created_at
CON	branches	n	NOT NULL fork_level
CON	branches	n	NOT NULL id
CON	branches	p	PRIMARY KEY (id)
CON	code_sandbox_rootfs_artifacts	n	NOT NULL arch
CON	code_sandbox_rootfs_artifacts	n	NOT NULL artifact_path
CON	code_sandbox_rootfs_artifacts	n	NOT NULL downloaded_at
CON	code_sandbox_rootfs_artifacts	n	NOT NULL flavor
CON	code_sandbox_rootfs_artifacts	n	NOT NULL id
CON	code_sandbox_rootfs_artifacts	n	NOT NULL package
CON	code_sandbox_rootfs_artifacts	n	NOT NULL sha256
CON	code_sandbox_rootfs_artifacts	n	NOT NULL status
CON	code_sandbox_rootfs_artifacts	n	NOT NULL version
CON	code_sandbox_rootfs_artifacts	p	PRIMARY KEY (id)
CON	code_sandbox_rootfs_artifacts	u	UNIQUE (version, arch, flavor, package)
CON	code_sandbox_settings	c	CHECK (address_space_bytes >= 16777216)
CON	code_sandbox_settings	c	CHECK (cpu_max ~ '^[0-9]+ [0-9]+$'::text)
CON	code_sandbox_settings	c	CHECK (cpu_secs_max >= 10 AND cpu_secs_max <= 86400)
CON	code_sandbox_settings	c	CHECK (fsize_bytes >= 1048576)
CON	code_sandbox_settings	c	CHECK (id = true)
CON	code_sandbox_settings	c	CHECK (mac_vm_ram_mib >= 256 AND mac_vm_ram_mib <= 262144)
CON	code_sandbox_settings	c	CHECK (mac_vm_vcpus >= 1 AND mac_vm_vcpus <= 128)
CON	code_sandbox_settings	c	CHECK (memory_max_bytes >= 16777216)
CON	code_sandbox_settings	c	CHECK (memory_swap_max_bytes >= 0)
CON	code_sandbox_settings	c	CHECK (nofile_max >= 64 AND nofile_max <= 1048576)
CON	code_sandbox_settings	c	CHECK (nproc_max >= 8 AND nproc_max <= 100000)
CON	code_sandbox_settings	c	CHECK (pids_max >= 8 AND pids_max <= 100000)
CON	code_sandbox_settings	c	CHECK (timeout_secs >= 5 AND timeout_secs <= 86400)
CON	code_sandbox_settings	c	CHECK (vm_idle_evict_secs >= 0)
CON	code_sandbox_settings	c	CHECK (vm_max_concurrent_execs >= 1 AND vm_max_concurrent_execs <= 1000)
CON	code_sandbox_settings	n	NOT NULL address_space_bytes
CON	code_sandbox_settings	n	NOT NULL cpu_max
CON	code_sandbox_settings	n	NOT NULL cpu_secs_max
CON	code_sandbox_settings	n	NOT NULL created_at
CON	code_sandbox_settings	n	NOT NULL fsize_bytes
CON	code_sandbox_settings	n	NOT NULL id
CON	code_sandbox_settings	n	NOT NULL mac_vm_ram_mib
CON	code_sandbox_settings	n	NOT NULL mac_vm_vcpus
CON	code_sandbox_settings	n	NOT NULL memory_max_bytes
CON	code_sandbox_settings	n	NOT NULL memory_swap_max_bytes
CON	code_sandbox_settings	n	NOT NULL nofile_max
CON	code_sandbox_settings	n	NOT NULL nproc_max
CON	code_sandbox_settings	n	NOT NULL pids_max
CON	code_sandbox_settings	n	NOT NULL timeout_secs
CON	code_sandbox_settings	n	NOT NULL updated_at
CON	code_sandbox_settings	n	NOT NULL vm_idle_evict_secs
CON	code_sandbox_settings	n	NOT NULL vm_max_concurrent_execs
CON	code_sandbox_settings	p	PRIMARY KEY (id)
CON	conversation_deliverables	f	FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
CON	conversation_deliverables	f	FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
CON	conversation_deliverables	n	NOT NULL conversation_id
CON	conversation_deliverables	n	NOT NULL created_at
CON	conversation_deliverables	n	NOT NULL file_id
CON	conversation_deliverables	n	NOT NULL pinned
CON	conversation_deliverables	p	PRIMARY KEY (conversation_id, file_id)
CON	conversation_knowledge_bases	f	FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
CON	conversation_knowledge_bases	f	FOREIGN KEY (knowledge_base_id) REFERENCES knowledge_bases(id) ON DELETE CASCADE
CON	conversation_knowledge_bases	n	NOT NULL added_at
CON	conversation_knowledge_bases	n	NOT NULL conversation_id
CON	conversation_knowledge_bases	n	NOT NULL knowledge_base_id
CON	conversation_knowledge_bases	p	PRIMARY KEY (conversation_id, knowledge_base_id)
CON	conversation_memory_settings	c	CHECK (memory_mode = ANY (ARRAY['inherit'::text, 'on'::text, 'off'::text]))
CON	conversation_memory_settings	f	FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
CON	conversation_memory_settings	n	NOT NULL conversation_id
CON	conversation_memory_settings	n	NOT NULL memory_mode
CON	conversation_memory_settings	p	PRIMARY KEY (conversation_id)
CON	conversation_skill_overrides	f	FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
CON	conversation_skill_overrides	f	FOREIGN KEY (skill_id) REFERENCES skills(id) ON DELETE CASCADE
CON	conversation_skill_overrides	n	NOT NULL conversation_id
CON	conversation_skill_overrides	n	NOT NULL created_at
CON	conversation_skill_overrides	n	NOT NULL hidden
CON	conversation_skill_overrides	n	NOT NULL skill_id
CON	conversation_skill_overrides	p	PRIMARY KEY (conversation_id, skill_id)
CON	conversation_summaries	f	FOREIGN KEY (branch_id) REFERENCES branches(id) ON DELETE CASCADE
CON	conversation_summaries	f	FOREIGN KEY (summarized_up_to_id) REFERENCES messages(id) ON DELETE SET NULL
CON	conversation_summaries	n	NOT NULL branch_id
CON	conversation_summaries	n	NOT NULL created_at
CON	conversation_summaries	n	NOT NULL message_count
CON	conversation_summaries	n	NOT NULL summary_text
CON	conversation_summaries	n	NOT NULL updated_at
CON	conversation_summaries	p	PRIMARY KEY (branch_id)
CON	conversation_summarization_settings	c	CHECK (summarization_mode = ANY (ARRAY['inherit'::text, 'on'::text, 'off'::text]))
CON	conversation_summarization_settings	f	FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
CON	conversation_summarization_settings	n	NOT NULL conversation_id
CON	conversation_summarization_settings	n	NOT NULL summarization_mode
CON	conversation_summarization_settings	n	NOT NULL updated_at
CON	conversation_summarization_settings	p	PRIMARY KEY (conversation_id)
CON	conversations	f	FOREIGN KEY (active_branch_id) REFERENCES branches(id) ON DELETE SET NULL
CON	conversations	f	FOREIGN KEY (model_id) REFERENCES llm_models(id) ON DELETE SET NULL
CON	conversations	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	conversations	n	NOT NULL created_at
CON	conversations	n	NOT NULL id
CON	conversations	n	NOT NULL updated_at
CON	conversations	n	NOT NULL user_id
CON	conversations	p	PRIMARY KEY (id)
CON	download_instances	c	CHECK (status::text = ANY (ARRAY['pending'::character varying, 'downloading'::character varying, 'completed'::character varying, 'failed'::character varying, 'cancelled'::character varying]::text[]))
CON	download_instances	f	FOREIGN KEY (model_id) REFERENCES llm_models(id) ON DELETE SET NULL
CON	download_instances	f	FOREIGN KEY (provider_id) REFERENCES llm_providers(id) ON DELETE CASCADE
CON	download_instances	f	FOREIGN KEY (repository_id) REFERENCES llm_repositories(id) ON DELETE CASCADE
CON	download_instances	n	NOT NULL created_at
CON	download_instances	n	NOT NULL id
CON	download_instances	n	NOT NULL provider_id
CON	download_instances	n	NOT NULL repository_id
CON	download_instances	n	NOT NULL request_data
CON	download_instances	n	NOT NULL started_at
CON	download_instances	n	NOT NULL status
CON	download_instances	n	NOT NULL updated_at
CON	download_instances	p	PRIMARY KEY (id)
CON	file_chunks	f	FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
CON	file_chunks	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	file_chunks	n	NOT NULL blob_version_id
CON	file_chunks	n	NOT NULL char_end
CON	file_chunks	n	NOT NULL char_start
CON	file_chunks	n	NOT NULL chunk_index
CON	file_chunks	n	NOT NULL content
CON	file_chunks	n	NOT NULL created_at
CON	file_chunks	n	NOT NULL file_id
CON	file_chunks	n	NOT NULL id
CON	file_chunks	n	NOT NULL page_number
CON	file_chunks	n	NOT NULL user_id
CON	file_chunks	n	NOT NULL version
CON	file_chunks	p	PRIMARY KEY (id)
CON	file_index_state	c	CHECK (status = ANY (ARRAY['pending'::text, 'indexing'::text, 'indexed'::text, 'failed'::text, 'no_text'::text]))
CON	file_index_state	f	FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
CON	file_index_state	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	file_index_state	n	NOT NULL chunk_count
CON	file_index_state	n	NOT NULL file_id
CON	file_index_state	n	NOT NULL status
CON	file_index_state	n	NOT NULL updated_at
CON	file_index_state	n	NOT NULL user_id
CON	file_index_state	p	PRIMARY KEY (file_id)
CON	file_rag_admin_settings	c	CHECK (chunk_chars >= 200 AND chunk_chars <= 8000)
CON	file_rag_admin_settings	c	CHECK (chunk_overlap_chars >= 0 AND chunk_overlap_chars < chunk_chars)
CON	file_rag_admin_settings	c	CHECK (cosine_threshold >= 0.0::double precision AND cosine_threshold <= 2.0::double precision)
CON	file_rag_admin_settings	c	CHECK (default_top_k > 0 AND default_top_k <= 50)
CON	file_rag_admin_settings	c	CHECK (embedding_dimensions > 0 AND embedding_dimensions <= 4000)
CON	file_rag_admin_settings	c	CHECK (fts_candidate_multiplier >= 1 AND fts_candidate_multiplier <= 20)
CON	file_rag_admin_settings	c	CHECK (fts_dictionary = ANY (ARRAY['simple'::text, 'english'::text, 'french'::text, 'german'::text, 'spanish'::text, 'italian'::text, 'portuguese'::text, 'russian'::text, 'dutch'::text, 'norwegian'::text, 'swedish'::text, 'danish'::text, 'finnish'::text, 'hungarian'::text, 'turkish'::text]))
CON	file_rag_admin_settings	c	CHECK (fts_min_rank >= 0.0::double precision AND fts_min_rank <= 1.0::double precision)
CON	file_rag_admin_settings	c	CHECK (fts_rrf_k >= 1 AND fts_rrf_k <= 1000)
CON	file_rag_admin_settings	c	CHECK (id = 1)
CON	file_rag_admin_settings	c	CHECK (kb_max_documents >= 1 AND kb_max_documents <= 100000)
CON	file_rag_admin_settings	c	CHECK (max_chunks_per_file > 0)
CON	file_rag_admin_settings	c	CHECK (rerank_candidate_k >= 1 AND rerank_candidate_k <= 200)
CON	file_rag_admin_settings	c	CHECK (search_max_hit_chars >= 100 AND search_max_hit_chars <= 100000)
CON	file_rag_admin_settings	c	CHECK (search_max_top_k >= 1 AND search_max_top_k <= 500)
CON	file_rag_admin_settings	c	CHECK (search_snippet_chars >= 20 AND search_snippet_chars <= 4000)
CON	file_rag_admin_settings	f	FOREIGN KEY (embedding_model_id) REFERENCES llm_models(id) ON DELETE SET NULL
CON	file_rag_admin_settings	f	FOREIGN KEY (reranker_model_id) REFERENCES llm_models(id) ON DELETE SET NULL
CON	file_rag_admin_settings	n	NOT NULL chunk_chars
CON	file_rag_admin_settings	n	NOT NULL chunk_overlap_chars
CON	file_rag_admin_settings	n	NOT NULL cosine_threshold
CON	file_rag_admin_settings	n	NOT NULL default_top_k
CON	file_rag_admin_settings	n	NOT NULL embedding_dimensions
CON	file_rag_admin_settings	n	NOT NULL enabled
CON	file_rag_admin_settings	n	NOT NULL fts_candidate_multiplier
CON	file_rag_admin_settings	n	NOT NULL fts_dictionary
CON	file_rag_admin_settings	n	NOT NULL fts_enabled
CON	file_rag_admin_settings	n	NOT NULL fts_min_rank
CON	file_rag_admin_settings	n	NOT NULL fts_rrf_k
CON	file_rag_admin_settings	n	NOT NULL id
CON	file_rag_admin_settings	n	NOT NULL kb_max_documents
CON	file_rag_admin_settings	n	NOT NULL max_chunks_per_file
CON	file_rag_admin_settings	n	NOT NULL rerank_candidate_k
CON	file_rag_admin_settings	n	NOT NULL rerank_enabled
CON	file_rag_admin_settings	n	NOT NULL search_max_hit_chars
CON	file_rag_admin_settings	n	NOT NULL search_max_top_k
CON	file_rag_admin_settings	n	NOT NULL search_snippet_chars
CON	file_rag_admin_settings	n	NOT NULL semantic_enabled
CON	file_rag_admin_settings	n	NOT NULL updated_at
CON	file_rag_admin_settings	p	PRIMARY KEY (id)
CON	file_versions	f	FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
CON	file_versions	n	NOT NULL blob_version_id
CON	file_versions	n	NOT NULL created_at
CON	file_versions	n	NOT NULL created_by
CON	file_versions	n	NOT NULL file_id
CON	file_versions	n	NOT NULL file_size
CON	file_versions	n	NOT NULL has_thumbnail
CON	file_versions	n	NOT NULL id
CON	file_versions	n	NOT NULL is_head
CON	file_versions	n	NOT NULL preview_page_count
CON	file_versions	n	NOT NULL processing_metadata
CON	file_versions	n	NOT NULL text_page_count
CON	file_versions	n	NOT NULL version
CON	file_versions	p	PRIMARY KEY (id)
CON	file_versions	u	UNIQUE (file_id, version)
CON	files	f	FOREIGN KEY (current_version_id) REFERENCES file_versions(id) DEFERRABLE INITIALLY DEFERRED
CON	files	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	files	f	FOREIGN KEY (workflow_run_id) REFERENCES workflow_runs(id) ON DELETE SET NULL
CON	files	n	NOT NULL created_at
CON	files	n	NOT NULL created_by
CON	files	n	NOT NULL current_version_id
CON	files	n	NOT NULL filename
CON	files	n	NOT NULL file_size
CON	files	n	NOT NULL has_thumbnail
CON	files	n	NOT NULL id
CON	files	n	NOT NULL preview_page_count
CON	files	n	NOT NULL text_page_count
CON	files	n	NOT NULL updated_at
CON	files	n	NOT NULL user_id
CON	files	p	PRIMARY KEY (id)
CON	group_skills	f	FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE
CON	group_skills	f	FOREIGN KEY (skill_id) REFERENCES skills(id) ON DELETE CASCADE
CON	group_skills	n	NOT NULL assigned_at
CON	group_skills	n	NOT NULL group_id
CON	group_skills	n	NOT NULL skill_id
CON	group_skills	p	PRIMARY KEY (group_id, skill_id)
CON	group_workflows	f	FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE
CON	group_workflows	f	FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
CON	group_workflows	n	NOT NULL assigned_at
CON	group_workflows	n	NOT NULL group_id
CON	group_workflows	n	NOT NULL workflow_id
CON	group_workflows	p	PRIMARY KEY (group_id, workflow_id)
CON	groups	n	NOT NULL created_at
CON	groups	n	NOT NULL id
CON	groups	n	NOT NULL is_active
CON	groups	n	NOT NULL is_default
CON	groups	n	NOT NULL is_system
CON	groups	n	NOT NULL name
CON	groups	n	NOT NULL permissions
CON	groups	n	NOT NULL updated_at
CON	groups	p	PRIMARY KEY (id)
CON	groups	u	UNIQUE (name)
CON	hub_entities	c	CHECK (entity_type::text = ANY (ARRAY['assistant'::character varying, 'mcp_server'::character varying, 'llm_model'::character varying, 'skill'::character varying, 'workflow'::character varying]::text[]))
CON	hub_entities	c	CHECK (hub_category::text = ANY (ARRAY['assistant'::character varying, 'mcp_server'::character varying, 'model'::character varying, 'skill'::character varying, 'workflow'::character varying]::text[]))
CON	hub_entities	f	FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL
CON	hub_entities	n	NOT NULL created_at
CON	hub_entities	n	NOT NULL entity_id
CON	hub_entities	n	NOT NULL entity_type
CON	hub_entities	n	NOT NULL hub_category
CON	hub_entities	n	NOT NULL hub_id
CON	hub_entities	n	NOT NULL id
CON	hub_entities	p	PRIMARY KEY (id)
CON	hub_entities	u	UNIQUE (entity_type, entity_id)
CON	hub_settings	c	CHECK (id = true)
CON	hub_settings	n	NOT NULL id
CON	hub_settings	n	NOT NULL updated_at
CON	hub_settings	p	PRIMARY KEY (id)
CON	js_tool_settings	c	CHECK (approval_timeout_secs >= 5 AND approval_timeout_secs <= 3600)
CON	js_tool_settings	c	CHECK (id = true)
CON	js_tool_settings	c	CHECK (max_concurrent_dispatch >= 1 AND max_concurrent_dispatch <= 64)
CON	js_tool_settings	c	CHECK (max_concurrent_runs >= 1 AND max_concurrent_runs <= 256)
CON	js_tool_settings	c	CHECK (max_stack_bytes >= 65536 AND max_stack_bytes <= 67108864)
CON	js_tool_settings	c	CHECK (max_trace_entries >= 1 AND max_trace_entries <= 10000)
CON	js_tool_settings	c	CHECK (memory_bytes >= 16777216 AND memory_bytes <= '4294967296'::bigint)
CON	js_tool_settings	c	CHECK (wall_secs >= 1 AND wall_secs <= 3600)
CON	js_tool_settings	n	NOT NULL approval_timeout_secs
CON	js_tool_settings	n	NOT NULL created_at
CON	js_tool_settings	n	NOT NULL id
CON	js_tool_settings	n	NOT NULL max_concurrent_dispatch
CON	js_tool_settings	n	NOT NULL max_concurrent_runs
CON	js_tool_settings	n	NOT NULL max_stack_bytes
CON	js_tool_settings	n	NOT NULL max_trace_entries
CON	js_tool_settings	n	NOT NULL memory_bytes
CON	js_tool_settings	n	NOT NULL updated_at
CON	js_tool_settings	n	NOT NULL wall_secs
CON	js_tool_settings	p	PRIMARY KEY (id)
CON	knowledge_base_documents	f	FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
CON	knowledge_base_documents	f	FOREIGN KEY (knowledge_base_id) REFERENCES knowledge_bases(id) ON DELETE CASCADE
CON	knowledge_base_documents	n	NOT NULL added_at
CON	knowledge_base_documents	n	NOT NULL file_id
CON	knowledge_base_documents	n	NOT NULL knowledge_base_id
CON	knowledge_base_documents	p	PRIMARY KEY (knowledge_base_id, file_id)
CON	knowledge_bases	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	knowledge_bases	n	NOT NULL created_at
CON	knowledge_bases	n	NOT NULL id
CON	knowledge_bases	n	NOT NULL name
CON	knowledge_bases	n	NOT NULL updated_at
CON	knowledge_bases	n	NOT NULL user_id
CON	knowledge_bases	p	PRIMARY KEY (id)
CON	lit_fulltext_cache	c	CHECK (doi IS NOT NULL OR pmid IS NOT NULL OR pmcid IS NOT NULL OR arxiv_id IS NOT NULL)
CON	lit_fulltext_cache	n	NOT NULL byte_size
CON	lit_fulltext_cache	n	NOT NULL fetched_at
CON	lit_fulltext_cache	n	NOT NULL id
CON	lit_fulltext_cache	n	NOT NULL last_accessed_at
CON	lit_fulltext_cache	n	NOT NULL status
CON	lit_fulltext_cache	p	PRIMARY KEY (id)
CON	lit_search_connectors	c	CHECK (connector <> ''::text)
CON	lit_search_connectors	n	NOT NULL config
CON	lit_search_connectors	n	NOT NULL connector
CON	lit_search_connectors	n	NOT NULL created_at
CON	lit_search_connectors	n	NOT NULL updated_at
CON	lit_search_connectors	p	PRIMARY KEY (connector)
CON	lit_search_settings	c	CHECK (id = true)
CON	lit_search_settings	c	CHECK (max_results >= 1 AND max_results <= 200)
CON	lit_search_settings	c	CHECK (per_source_limit >= 1 AND per_source_limit <= 100)
CON	lit_search_settings	c	CHECK (request_timeout_secs >= 1 AND request_timeout_secs <= 120)
CON	lit_search_settings	n	NOT NULL completeness_estimate_enabled
CON	lit_search_settings	n	NOT NULL created_at
CON	lit_search_settings	n	NOT NULL enabled
CON	lit_search_settings	n	NOT NULL enabled_connectors
CON	lit_search_settings	n	NOT NULL id
CON	lit_search_settings	n	NOT NULL max_results
CON	lit_search_settings	n	NOT NULL per_source_limit
CON	lit_search_settings	n	NOT NULL request_timeout_secs
CON	lit_search_settings	n	NOT NULL updated_at
CON	lit_search_settings	p	PRIMARY KEY (id)
CON	llm_model_files	c	CHECK (upload_status::text = ANY (ARRAY['pending'::character varying, 'uploading'::character varying, 'completed'::character varying, 'failed'::character varying]::text[]))
CON	llm_model_files	f	FOREIGN KEY (model_id) REFERENCES llm_models(id) ON DELETE CASCADE
CON	llm_model_files	n	NOT NULL filename
CON	llm_model_files	n	NOT NULL file_path
CON	llm_model_files	n	NOT NULL file_size_bytes
CON	llm_model_files	n	NOT NULL file_type
CON	llm_model_files	n	NOT NULL id
CON	llm_model_files	n	NOT NULL model_id
CON	llm_model_files	n	NOT NULL uploaded_at
CON	llm_model_files	n	NOT NULL upload_status
CON	llm_model_files	p	PRIMARY KEY (id)
CON	llm_model_files	u	UNIQUE (model_id, filename)
CON	llm_models	c	CHECK (display_name::text <> ''::text)
CON	llm_models	c	CHECK (engine_type::text = ANY (ARRAY['mistralrs'::character varying, 'llamacpp'::character varying, 'none'::character varying]::text[]))
CON	llm_models	c	CHECK (file_format::text = ANY (ARRAY['safetensors'::character varying, 'pytorch'::character varying, 'gguf'::character varying]::text[]))
CON	llm_models	c	CHECK (validation_status::text = ANY (ARRAY['pending'::character varying, 'await_upload'::character varying, 'downloading'::character varying, 'processing'::character varying, 'completed'::character varying, 'failed'::character varying, 'valid'::character varying, 'invalid'::character varying, 'error'::character varying, 'validation_warning'::character varying]::text[]))
CON	llm_models	f	FOREIGN KEY (provider_id) REFERENCES llm_providers(id) ON DELETE CASCADE
CON	llm_models	f	FOREIGN KEY (required_runtime_version_id) REFERENCES llm_runtime_versions(id) ON DELETE SET NULL
CON	llm_models	n	NOT NULL created_at
CON	llm_models	n	NOT NULL display_name
CON	llm_models	n	NOT NULL enabled
CON	llm_models	n	NOT NULL engine_type
CON	llm_models	n	NOT NULL file_format
CON	llm_models	n	NOT NULL id
CON	llm_models	n	NOT NULL is_active
CON	llm_models	n	NOT NULL is_deprecated
CON	llm_models	n	NOT NULL name
CON	llm_models	n	NOT NULL provider_id
CON	llm_models	n	NOT NULL updated_at
CON	llm_models	p	PRIMARY KEY (id)
CON	llm_models	u	UNIQUE (provider_id, name)
CON	llm_provider_files	c	CHECK (upload_status::text = ANY (ARRAY['pending'::character varying, 'uploading'::character varying, 'completed'::character varying, 'failed'::character varying, 'expired'::character varying]::text[]))
CON	llm_provider_files	f	FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
CON	llm_provider_files	f	FOREIGN KEY (provider_id) REFERENCES llm_providers(id) ON DELETE CASCADE
CON	llm_provider_files	n	NOT NULL created_at
CON	llm_provider_files	n	NOT NULL file_id
CON	llm_provider_files	n	NOT NULL id
CON	llm_provider_files	n	NOT NULL provider_id
CON	llm_provider_files	n	NOT NULL provider_metadata
CON	llm_provider_files	n	NOT NULL updated_at
CON	llm_provider_files	n	NOT NULL upload_status
CON	llm_provider_files	p	PRIMARY KEY (id)
CON	llm_provider_files	u	UNIQUE (file_id, provider_id)
CON	llm_providers	c	CHECK (provider_type::text = ANY (ARRAY['local'::character varying, 'openai'::character varying, 'anthropic'::character varying, 'groq'::character varying, 'gemini'::character varying, 'mistral'::character varying, 'deepseek'::character varying, 'huggingface'::character varying, 'custom'::character varying, 'openrouter'::character varying]::text[]))
CON	llm_providers	f	FOREIGN KEY (default_runtime_version_id) REFERENCES llm_runtime_versions(id) ON DELETE SET NULL
CON	llm_providers	n	NOT NULL built_in
CON	llm_providers	n	NOT NULL created_at
CON	llm_providers	n	NOT NULL enabled
CON	llm_providers	n	NOT NULL id
CON	llm_providers	n	NOT NULL name
CON	llm_providers	n	NOT NULL provider_type
CON	llm_providers	n	NOT NULL updated_at
CON	llm_providers	p	PRIMARY KEY (id)
CON	llm_repositories	c	CHECK (auth_type::text = ANY (ARRAY['none'::character varying, 'api_key'::character varying, 'basic_auth'::character varying, 'bearer_token'::character varying]::text[]))
CON	llm_repositories	c	CHECK (last_health_check_status = ANY (ARRAY['untested'::text, 'healthy'::text, 'unhealthy'::text]))
CON	llm_repositories	n	NOT NULL auth_type
CON	llm_repositories	n	NOT NULL built_in
CON	llm_repositories	n	NOT NULL created_at
CON	llm_repositories	n	NOT NULL enabled
CON	llm_repositories	n	NOT NULL id
CON	llm_repositories	n	NOT NULL last_health_check_status
CON	llm_repositories	n	NOT NULL name
CON	llm_repositories	n	NOT NULL updated_at
CON	llm_repositories	n	NOT NULL url
CON	llm_repositories	p	PRIMARY KEY (id)
CON	llm_repositories	u	UNIQUE (name)
CON	llm_repositories	u	UNIQUE (url)
CON	llm_runtime_instances	c	CHECK (state::text = ANY (ARRAY['starting'::character varying, 'healthy'::character varying, 'unhealthy'::character varying, 'crashed'::character varying, 'restarting'::character varying, 'failed'::character varying, 'stopped'::character varying]::text[]))
CON	llm_runtime_instances	c	CHECK (status::text = ANY (ARRAY['starting'::character varying, 'running'::character varying, 'stopping'::character varying, 'stopped'::character varying, 'failed'::character varying]::text[]))
CON	llm_runtime_instances	f	FOREIGN KEY (model_id) REFERENCES llm_models(id) ON DELETE CASCADE
CON	llm_runtime_instances	f	FOREIGN KEY (provider_id) REFERENCES llm_providers(id) ON DELETE CASCADE
CON	llm_runtime_instances	f	FOREIGN KEY (runtime_version_id) REFERENCES llm_runtime_versions(id) ON DELETE SET NULL
CON	llm_runtime_instances	n	NOT NULL base_url
CON	llm_runtime_instances	n	NOT NULL id
CON	llm_runtime_instances	n	NOT NULL last_used_at
CON	llm_runtime_instances	n	NOT NULL local_port
CON	llm_runtime_instances	n	NOT NULL model_id
CON	llm_runtime_instances	n	NOT NULL provider_id
CON	llm_runtime_instances	n	NOT NULL restart_attempts
CON	llm_runtime_instances	n	NOT NULL started_at
CON	llm_runtime_instances	n	NOT NULL state
CON	llm_runtime_instances	n	NOT NULL state_changed_at
CON	llm_runtime_instances	n	NOT NULL status
CON	llm_runtime_instances	p	PRIMARY KEY (id)
CON	llm_runtime_instances	u	UNIQUE (model_id)
CON	llm_runtime_settings	c	CHECK (auto_start_timeout_secs >= 1 AND auto_start_timeout_secs <= 600)
CON	llm_runtime_settings	c	CHECK (drain_timeout_secs >= 1 AND drain_timeout_secs <= 600)
CON	llm_runtime_settings	c	CHECK (idle_unload_secs >= 0 AND idle_unload_secs <= 86400)
CON	llm_runtime_settings	c	CHECK (id = true)
CON	llm_runtime_settings	n	NOT NULL auto_start_timeout_secs
CON	llm_runtime_settings	n	NOT NULL created_at
CON	llm_runtime_settings	n	NOT NULL drain_timeout_secs
CON	llm_runtime_settings	n	NOT NULL id
CON	llm_runtime_settings	n	NOT NULL idle_unload_secs
CON	llm_runtime_settings	n	NOT NULL updated_at
CON	llm_runtime_settings	p	PRIMARY KEY (id)
CON	llm_runtime_versions	c	CHECK (engine::text = ANY (ARRAY['llamacpp'::character varying, 'mistralrs'::character varying]::text[]))
CON	llm_runtime_versions	n	NOT NULL arch
CON	llm_runtime_versions	n	NOT NULL backend
CON	llm_runtime_versions	n	NOT NULL binary_path
CON	llm_runtime_versions	n	NOT NULL created_at
CON	llm_runtime_versions	n	NOT NULL engine
CON	llm_runtime_versions	n	NOT NULL id
CON	llm_runtime_versions	n	NOT NULL is_system_default
CON	llm_runtime_versions	n	NOT NULL platform
CON	llm_runtime_versions	n	NOT NULL version
CON	llm_runtime_versions	p	PRIMARY KEY (id)
CON	llm_runtime_versions	u	UNIQUE (engine, version, platform, arch, backend)
CON	mcp_server_oauth_configs	f	FOREIGN KEY (server_id) REFERENCES mcp_servers(id) ON DELETE CASCADE
CON	mcp_server_oauth_configs	n	NOT NULL client_id
CON	mcp_server_oauth_configs	n	NOT NULL created_at
CON	mcp_server_oauth_configs	n	NOT NULL server_id
CON	mcp_server_oauth_configs	n	NOT NULL updated_at
CON	mcp_server_oauth_configs	p	PRIMARY KEY (server_id)
CON	mcp_servers	c	CHECK (is_system = true AND user_id IS NULL OR is_system = false AND user_id IS NOT NULL)
CON	mcp_servers	c	CHECK (last_health_check_status = ANY (ARRAY['untested'::text, 'healthy'::text, 'unhealthy'::text]))
CON	mcp_servers	c	CHECK (transport_type::text = 'stdio'::text AND command IS NOT NULL OR (transport_type::text = ANY (ARRAY['http'::character varying, 'sse'::character varying]::text[])) AND url IS NOT NULL)
CON	mcp_servers	c	CHECK (usage_mode::text = ANY (ARRAY['auto'::character varying, 'always'::character varying]::text[]))
CON	mcp_servers	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	mcp_servers	n	NOT NULL created_at
CON	mcp_servers	n	NOT NULL display_name
CON	mcp_servers	n	NOT NULL enabled
CON	mcp_servers	n	NOT NULL environment_variables_encrypted
CON	mcp_servers	n	NOT NULL environment_variables_secret_keys
CON	mcp_servers	n	NOT NULL headers_encrypted
CON	mcp_servers	n	NOT NULL headers_secret_keys
CON	mcp_servers	n	NOT NULL id
CON	mcp_servers	n	NOT NULL is_built_in
CON	mcp_servers	n	NOT NULL is_system
CON	mcp_servers	n	NOT NULL last_health_check_status
CON	mcp_servers	n	NOT NULL name
CON	mcp_servers	n	NOT NULL run_in_sandbox
CON	mcp_servers	n	NOT NULL sandbox_flavor
CON	mcp_servers	n	NOT NULL supports_sampling
CON	mcp_servers	n	NOT NULL timeout_seconds
CON	mcp_servers	n	NOT NULL transport_type
CON	mcp_servers	n	NOT NULL updated_at
CON	mcp_servers	n	NOT NULL usage_mode
CON	mcp_servers	p	PRIMARY KEY (id)
CON	mcp_settings	c	CHECK ((conversation_id IS NULL) <> (project_id IS NULL))
CON	mcp_settings	f	FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
CON	mcp_settings	f	FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
CON	mcp_settings	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	mcp_settings	n	NOT NULL approval_mode
CON	mcp_settings	n	NOT NULL auto_approved_tools
CON	mcp_settings	n	NOT NULL created_at
CON	mcp_settings	n	NOT NULL disabled_servers
CON	mcp_settings	n	NOT NULL id
CON	mcp_settings	n	NOT NULL updated_at
CON	mcp_settings	n	NOT NULL user_id
CON	mcp_settings	p	PRIMARY KEY (id)
CON	mcp_settings	u	UNIQUE (conversation_id)
CON	mcp_settings	u	UNIQUE (project_id)
CON	mcp_tool_calls	c	CHECK (source::text = ANY (ARRAY['chat'::character varying, 'rest'::character varying, 'always'::character varying, 'sampling'::character varying, 'approval'::character varying, 'workflow'::character varying, 'script'::character varying]::text[]))
CON	mcp_tool_calls	c	CHECK (status::text = ANY (ARRAY['completed'::character varying, 'failed'::character varying, 'timeout'::character varying, 'cancelled'::character varying]::text[]))
CON	mcp_tool_calls	f	FOREIGN KEY (branch_id) REFERENCES branches(id) ON DELETE SET NULL
CON	mcp_tool_calls	f	FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE SET NULL
CON	mcp_tool_calls	f	FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE SET NULL
CON	mcp_tool_calls	f	FOREIGN KEY (server_id) REFERENCES mcp_servers(id) ON DELETE SET NULL
CON	mcp_tool_calls	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	mcp_tool_calls	f	FOREIGN KEY (workflow_run_id) REFERENCES workflow_runs(id) ON DELETE SET NULL
CON	mcp_tool_calls	n	NOT NULL arguments_json
CON	mcp_tool_calls	n	NOT NULL content_kinds
CON	mcp_tool_calls	n	NOT NULL created_at
CON	mcp_tool_calls	n	NOT NULL id
CON	mcp_tool_calls	n	NOT NULL is_built_in
CON	mcp_tool_calls	n	NOT NULL is_error
CON	mcp_tool_calls	n	NOT NULL result_bytes
CON	mcp_tool_calls	n	NOT NULL server_name
CON	mcp_tool_calls	n	NOT NULL source
CON	mcp_tool_calls	n	NOT NULL started_at
CON	mcp_tool_calls	n	NOT NULL status
CON	mcp_tool_calls	n	NOT NULL tool_name
CON	mcp_tool_calls	n	NOT NULL updated_at
CON	mcp_tool_calls	n	NOT NULL user_id
CON	mcp_tool_calls	p	PRIMARY KEY (id)
CON	mcp_user_policy	c	CHECK (id = 1)
CON	mcp_user_policy	f	FOREIGN KEY (updated_by) REFERENCES users(id) ON DELETE SET NULL
CON	mcp_user_policy	n	NOT NULL allowed_transports
CON	mcp_user_policy	n	NOT NULL id
CON	mcp_user_policy	n	NOT NULL tool_call_retention_days
CON	mcp_user_policy	n	NOT NULL updated_at
CON	mcp_user_policy	p	PRIMARY KEY (id)
CON	memory_admin_settings	c	CHECK (cosine_threshold >= 0.0::double precision AND cosine_threshold <= 2.0::double precision)
CON	memory_admin_settings	c	CHECK (daily_extraction_quota >= 1 AND daily_extraction_quota <= 10000)
CON	memory_admin_settings	c	CHECK (default_top_k > 0 AND default_top_k <= 100)
CON	memory_admin_settings	c	CHECK (embedding_dimensions > 0 AND embedding_dimensions <= 16000)
CON	memory_admin_settings	c	CHECK (fts_candidate_multiplier >= 1 AND fts_candidate_multiplier <= 20)
CON	memory_admin_settings	c	CHECK (fts_dictionary = ANY (ARRAY['simple'::text, 'english'::text, 'french'::text, 'german'::text, 'spanish'::text, 'italian'::text, 'portuguese'::text, 'russian'::text, 'dutch'::text, 'norwegian'::text, 'swedish'::text, 'danish'::text, 'finnish'::text, 'hungarian'::text, 'turkish'::text]))
CON	memory_admin_settings	c	CHECK (fts_min_rank >= 0.0::double precision AND fts_min_rank <= 1.0::double precision)
CON	memory_admin_settings	c	CHECK (fts_rrf_k >= 1 AND fts_rrf_k <= 1000)
CON	memory_admin_settings	c	CHECK (id = 1)
CON	memory_admin_settings	c	CHECK (soft_delete_grace_days >= 1 AND soft_delete_grace_days <= 365)
CON	memory_admin_settings	f	FOREIGN KEY (default_extraction_model_id) REFERENCES llm_models(id) ON DELETE SET NULL
CON	memory_admin_settings	f	FOREIGN KEY (embedding_model_id) REFERENCES llm_models(id) ON DELETE SET NULL
CON	memory_admin_settings	n	NOT NULL cosine_threshold
CON	memory_admin_settings	n	NOT NULL daily_extraction_quota
CON	memory_admin_settings	n	NOT NULL default_top_k
CON	memory_admin_settings	n	NOT NULL embedding_dimensions
CON	memory_admin_settings	n	NOT NULL enabled
CON	memory_admin_settings	n	NOT NULL fts_candidate_multiplier
CON	memory_admin_settings	n	NOT NULL fts_dictionary
CON	memory_admin_settings	n	NOT NULL fts_enabled
CON	memory_admin_settings	n	NOT NULL fts_min_rank
CON	memory_admin_settings	n	NOT NULL fts_rrf_k
CON	memory_admin_settings	n	NOT NULL id
CON	memory_admin_settings	n	NOT NULL semantic_enabled
CON	memory_admin_settings	n	NOT NULL soft_delete_grace_days
CON	memory_admin_settings	n	NOT NULL updated_at
CON	memory_admin_settings	p	PRIMARY KEY (id)
CON	memory_audit_log	c	CHECK (actor_kind = ANY (ARRAY['user'::text, 'assistant'::text, 'admin'::text, 'system'::text]))
CON	memory_audit_log	c	CHECK (op = ANY (ARRAY['ADD'::text, 'UPDATE'::text, 'DELETE'::text, 'BULK_DELETE'::text]))
CON	memory_audit_log	c	CHECK (source = ANY (ARRAY['extraction'::text, 'mcp_tool'::text, 'manual'::text, 'admin'::text]))
CON	memory_audit_log	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	memory_audit_log	n	NOT NULL actor_kind
CON	memory_audit_log	n	NOT NULL created_at
CON	memory_audit_log	n	NOT NULL id
CON	memory_audit_log	n	NOT NULL metadata
CON	memory_audit_log	n	NOT NULL op
CON	memory_audit_log	n	NOT NULL source
CON	memory_audit_log	n	NOT NULL user_id
CON	memory_audit_log	p	PRIMARY KEY (id)
CON	message_assistant	f	FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
CON	message_assistant	n	NOT NULL assistant_id
CON	message_assistant	n	NOT NULL message_id
CON	message_assistant	p	PRIMARY KEY (message_id)
CON	message_contents	f	FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
CON	message_contents	n	NOT NULL content
CON	message_contents	n	NOT NULL content_type
CON	message_contents	n	NOT NULL created_at
CON	message_contents	n	NOT NULL id
CON	message_contents	n	NOT NULL message_id
CON	message_contents	n	NOT NULL sequence_order
CON	message_contents	n	NOT NULL updated_at
CON	message_contents	p	PRIMARY KEY (id)
CON	message_contents	u	UNIQUE (message_id, sequence_order)
CON	message_mcp_servers	f	FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
CON	message_mcp_servers	n	NOT NULL message_id
CON	message_mcp_servers	n	NOT NULL server_id
CON	message_mcp_servers	p	PRIMARY KEY (message_id, server_id)
CON	messages	n	NOT NULL created_at
CON	messages	n	NOT NULL edit_count
CON	messages	n	NOT NULL id
CON	messages	n	NOT NULL originated_from_id
CON	messages	n	NOT NULL role
CON	messages	p	PRIMARY KEY (id)
CON	notifications	f	FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE SET NULL
CON	notifications	f	FOREIGN KEY (scheduled_task_id) REFERENCES scheduled_tasks(id) ON DELETE SET NULL
CON	notifications	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	notifications	f	FOREIGN KEY (workflow_run_id) REFERENCES workflow_runs(id) ON DELETE SET NULL
CON	notifications	n	NOT NULL body
CON	notifications	n	NOT NULL created_at
CON	notifications	n	NOT NULL id
CON	notifications	n	NOT NULL interrupt
CON	notifications	n	NOT NULL kind
CON	notifications	n	NOT NULL title
CON	notifications	n	NOT NULL user_id
CON	notifications	p	PRIMARY KEY (id)
CON	oauth_sessions	f	FOREIGN KEY (provider_id) REFERENCES auth_providers(id) ON DELETE CASCADE
CON	oauth_sessions	n	NOT NULL created_at
CON	oauth_sessions	n	NOT NULL expires_at
CON	oauth_sessions	n	NOT NULL id
CON	oauth_sessions	n	NOT NULL provider_id
CON	oauth_sessions	n	NOT NULL redirect_uri
CON	oauth_sessions	n	NOT NULL state
CON	oauth_sessions	p	PRIMARY KEY (id)
CON	oauth_sessions	u	UNIQUE (state)
CON	pending_account_links	f	FOREIGN KEY (provider_id) REFERENCES auth_providers(id) ON DELETE CASCADE
CON	pending_account_links	f	FOREIGN KEY (target_user_id) REFERENCES users(id) ON DELETE CASCADE
CON	pending_account_links	n	NOT NULL attempts
CON	pending_account_links	n	NOT NULL created_at
CON	pending_account_links	n	NOT NULL expires_at
CON	pending_account_links	n	NOT NULL external_id
CON	pending_account_links	n	NOT NULL link_token
CON	pending_account_links	n	NOT NULL provider_id
CON	pending_account_links	n	NOT NULL target_user_id
CON	pending_account_links	p	PRIMARY KEY (link_token)
CON	project_bibliography	f	FOREIGN KEY (entry_id) REFERENCES bibliography_entries(id) ON DELETE CASCADE
CON	project_bibliography	f	FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
CON	project_bibliography	n	NOT NULL added_at
CON	project_bibliography	n	NOT NULL entry_id
CON	project_bibliography	n	NOT NULL project_id
CON	project_bibliography	p	PRIMARY KEY (project_id, entry_id)
CON	project_conversations	f	FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
CON	project_conversations	f	FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
CON	project_conversations	n	NOT NULL attached_at
CON	project_conversations	n	NOT NULL conversation_id
CON	project_conversations	n	NOT NULL project_id
CON	project_conversations	p	PRIMARY KEY (conversation_id)
CON	project_files	f	FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
CON	project_files	f	FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
CON	project_files	n	NOT NULL added_at
CON	project_files	n	NOT NULL file_id
CON	project_files	n	NOT NULL project_id
CON	project_files	p	PRIMARY KEY (project_id, file_id)
CON	project_knowledge_bases	f	FOREIGN KEY (knowledge_base_id) REFERENCES knowledge_bases(id) ON DELETE CASCADE
CON	project_knowledge_bases	f	FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
CON	project_knowledge_bases	n	NOT NULL added_at
CON	project_knowledge_bases	n	NOT NULL knowledge_base_id
CON	project_knowledge_bases	n	NOT NULL project_id
CON	project_knowledge_bases	p	PRIMARY KEY (project_id, knowledge_base_id)
CON	projects	f	FOREIGN KEY (default_assistant_id) REFERENCES assistants(id) ON DELETE SET NULL
CON	projects	f	FOREIGN KEY (default_model_id) REFERENCES llm_models(id) ON DELETE SET NULL
CON	projects	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	projects	n	NOT NULL created_at
CON	projects	n	NOT NULL id
CON	projects	n	NOT NULL name
CON	projects	n	NOT NULL updated_at
CON	projects	n	NOT NULL user_id
CON	projects	p	PRIMARY KEY (id)
CON	projects	u	UNIQUE (user_id, name)
CON	refresh_tokens	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	refresh_tokens	n	NOT NULL expires_at
CON	refresh_tokens	n	NOT NULL issued_at
CON	refresh_tokens	n	NOT NULL jti
CON	refresh_tokens	n	NOT NULL user_id
CON	refresh_tokens	p	PRIMARY KEY (jti)
CON	sandbox_workspace_files	f	FOREIGN KEY (base_version_id) REFERENCES file_versions(id) ON DELETE CASCADE
CON	sandbox_workspace_files	f	FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
CON	sandbox_workspace_files	f	FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
CON	sandbox_workspace_files	n	NOT NULL base_version_id
CON	sandbox_workspace_files	n	NOT NULL conversation_id
CON	sandbox_workspace_files	n	NOT NULL file_id
CON	sandbox_workspace_files	n	NOT NULL workspace_relpath
CON	sandbox_workspace_files	p	PRIMARY KEY (conversation_id, workspace_relpath)
CON	scheduled_task_runs	c	CHECK (status = ANY (ARRAY['completed'::text, 'no_change'::text, 'failed'::text]))
CON	scheduled_task_runs	c	CHECK (trigger = ANY (ARRAY['schedule'::text, 'run_now'::text, 'catchup'::text]))
CON	scheduled_task_runs	f	FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE SET NULL
CON	scheduled_task_runs	f	FOREIGN KEY (notification_id) REFERENCES notifications(id) ON DELETE SET NULL
CON	scheduled_task_runs	f	FOREIGN KEY (scheduled_task_id) REFERENCES scheduled_tasks(id) ON DELETE CASCADE
CON	scheduled_task_runs	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	scheduled_task_runs	f	FOREIGN KEY (workflow_run_id) REFERENCES workflow_runs(id) ON DELETE SET NULL
CON	scheduled_task_runs	n	NOT NULL fired_at
CON	scheduled_task_runs	n	NOT NULL id
CON	scheduled_task_runs	n	NOT NULL scheduled_task_id
CON	scheduled_task_runs	n	NOT NULL skipped_tools
CON	scheduled_task_runs	n	NOT NULL status
CON	scheduled_task_runs	n	NOT NULL trigger
CON	scheduled_task_runs	n	NOT NULL user_id
CON	scheduled_task_runs	p	PRIMARY KEY (id)
CON	scheduled_tasks	c	CHECK (notify_mode = ANY (ARRAY['always'::text, 'silent'::text]))
CON	scheduled_tasks	c	CHECK (notify_on = ANY (ARRAY['always'::text, 'on_change'::text]))
CON	scheduled_tasks	c	CHECK (schedule_kind = ANY (ARRAY['once'::text, 'recurring'::text]))
CON	scheduled_tasks	c	CHECK (schedule_kind = 'once'::text AND run_at IS NOT NULL OR schedule_kind = 'recurring'::text AND cron_expr IS NOT NULL)
CON	scheduled_tasks	c	CHECK (target_kind = ANY (ARRAY['workflow'::text, 'prompt'::text]))
CON	scheduled_tasks	c	CHECK (target_kind = 'workflow'::text AND workflow_id IS NOT NULL OR target_kind = 'prompt'::text AND prompt IS NOT NULL)
CON	scheduled_tasks	f	FOREIGN KEY (assistant_id) REFERENCES assistants(id) ON DELETE SET NULL
CON	scheduled_tasks	f	FOREIGN KEY (bound_conversation_id) REFERENCES conversations(id) ON DELETE SET NULL
CON	scheduled_tasks	f	FOREIGN KEY (model_id) REFERENCES llm_models(id) ON DELETE SET NULL
CON	scheduled_tasks	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	scheduled_tasks	f	FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE SET NULL
CON	scheduled_tasks	n	NOT NULL allowed_unattended_tools
CON	scheduled_tasks	n	NOT NULL consecutive_failures
CON	scheduled_tasks	n	NOT NULL created_at
CON	scheduled_tasks	n	NOT NULL enabled
CON	scheduled_tasks	n	NOT NULL id
CON	scheduled_tasks	n	NOT NULL inputs_json
CON	scheduled_tasks	n	NOT NULL name
CON	scheduled_tasks	n	NOT NULL notify_mode
CON	scheduled_tasks	n	NOT NULL notify_on
CON	scheduled_tasks	n	NOT NULL schedule_kind
CON	scheduled_tasks	n	NOT NULL target_kind
CON	scheduled_tasks	n	NOT NULL timezone
CON	scheduled_tasks	n	NOT NULL updated_at
CON	scheduled_tasks	n	NOT NULL user_id
CON	scheduled_tasks	p	PRIMARY KEY (id)
CON	scheduler_admin_settings	c	CHECK (id = true)
CON	scheduler_admin_settings	c	CHECK (max_active_tasks_per_user >= 1 AND max_active_tasks_per_user <= 1000)
CON	scheduler_admin_settings	c	CHECK (max_consecutive_failures >= 1 AND max_consecutive_failures <= 100)
CON	scheduler_admin_settings	c	CHECK (min_interval_seconds >= 60 AND min_interval_seconds <= 86400)
CON	scheduler_admin_settings	c	CHECK (notification_retention_days >= 0 AND notification_retention_days <= 3650)
CON	scheduler_admin_settings	n	NOT NULL id
CON	scheduler_admin_settings	n	NOT NULL max_active_tasks_per_user
CON	scheduler_admin_settings	n	NOT NULL max_consecutive_failures
CON	scheduler_admin_settings	n	NOT NULL min_interval_seconds
CON	scheduler_admin_settings	n	NOT NULL notification_retention_days
CON	scheduler_admin_settings	n	NOT NULL updated_at
CON	scheduler_admin_settings	p	PRIMARY KEY (id)
CON	session_settings	c	CHECK (access_token_expiry_hours >= 1 AND access_token_expiry_hours <= 8760)
CON	session_settings	c	CHECK (id = true)
CON	session_settings	c	CHECK (refresh_token_expiry_days >= 1 AND refresh_token_expiry_days <= 3650)
CON	session_settings	n	NOT NULL access_token_expiry_hours
CON	session_settings	n	NOT NULL id
CON	session_settings	n	NOT NULL refresh_token_expiry_days
CON	session_settings	n	NOT NULL seeded_from_config
CON	session_settings	n	NOT NULL updated_at
CON	session_settings	p	PRIMARY KEY (id)
CON	skills	c	CHECK (scope::text = ANY (ARRAY['user'::character varying, 'system'::character varying, 'built_in'::character varying]::text[]))
CON	skills	c	CHECK (scope::text = 'user'::text AND owner_user_id IS NOT NULL OR (scope::text = ANY (ARRAY['system'::character varying, 'built_in'::character varying]::text[])) AND owner_user_id IS NULL)
CON	skills	f	FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL
CON	skills	f	FOREIGN KEY (owner_user_id) REFERENCES users(id) ON DELETE CASCADE
CON	skills	n	NOT NULL bundle_sha256
CON	skills	n	NOT NULL bundle_size_bytes
CON	skills	n	NOT NULL created_at
CON	skills	n	NOT NULL enabled
CON	skills	n	NOT NULL entry_point
CON	skills	n	NOT NULL extracted_path
CON	skills	n	NOT NULL file_count
CON	skills	n	NOT NULL frontmatter_json
CON	skills	n	NOT NULL id
CON	skills	n	NOT NULL is_dev
CON	skills	n	NOT NULL name
CON	skills	n	NOT NULL scope
CON	skills	n	NOT NULL tags
CON	skills	n	NOT NULL updated_at
CON	skills	p	PRIMARY KEY (id)
CON	summarization_admin_settings	c	CHECK (id = 1)
CON	summarization_admin_settings	c	CHECK (summarize_after_tokens >= 500 AND summarize_after_tokens <= 1000000)
CON	summarization_admin_settings	c	CHECK (summarizer_keep_recent_tokens >= 100)
CON	summarization_admin_settings	c	CHECK (summarizer_keep_recent_tokens < summarize_after_tokens)
CON	summarization_admin_settings	f	FOREIGN KEY (default_summarization_model_id) REFERENCES llm_models(id) ON DELETE SET NULL
CON	summarization_admin_settings	n	NOT NULL enabled
CON	summarization_admin_settings	n	NOT NULL id
CON	summarization_admin_settings	n	NOT NULL summarize_after_tokens
CON	summarization_admin_settings	n	NOT NULL summarizer_keep_recent_tokens
CON	summarization_admin_settings	n	NOT NULL updated_at
CON	summarization_admin_settings	p	PRIMARY KEY (id)
CON	tool_use_approvals	f	FOREIGN KEY (approved_by) REFERENCES users(id) ON DELETE SET NULL
CON	tool_use_approvals	f	FOREIGN KEY (branch_id) REFERENCES branches(id) ON DELETE CASCADE
CON	tool_use_approvals	f	FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
CON	tool_use_approvals	f	FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
CON	tool_use_approvals	f	FOREIGN KEY (server_id) REFERENCES mcp_servers(id) ON DELETE CASCADE
CON	tool_use_approvals	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	tool_use_approvals	n	NOT NULL branch_id
CON	tool_use_approvals	n	NOT NULL conversation_id
CON	tool_use_approvals	n	NOT NULL created_at
CON	tool_use_approvals	n	NOT NULL id
CON	tool_use_approvals	n	NOT NULL message_id
CON	tool_use_approvals	n	NOT NULL server_name
CON	tool_use_approvals	n	NOT NULL status
CON	tool_use_approvals	n	NOT NULL tool_input
CON	tool_use_approvals	n	NOT NULL tool_name
CON	tool_use_approvals	n	NOT NULL tool_use_id
CON	tool_use_approvals	n	NOT NULL updated_at
CON	tool_use_approvals	n	NOT NULL user_id
CON	tool_use_approvals	p	PRIMARY KEY (id)
CON	tool_use_approvals	u	UNIQUE (message_id, tool_use_id)
CON	user_auth_links	f	FOREIGN KEY (provider_id) REFERENCES auth_providers(id) ON DELETE CASCADE
CON	user_auth_links	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	user_auth_links	n	NOT NULL created_at
CON	user_auth_links	n	NOT NULL external_id
CON	user_auth_links	n	NOT NULL id
CON	user_auth_links	n	NOT NULL provider_id
CON	user_auth_links	n	NOT NULL updated_at
CON	user_auth_links	n	NOT NULL user_id
CON	user_auth_links	p	PRIMARY KEY (id)
CON	user_auth_links	u	UNIQUE (provider_id, external_id)
CON	user_group_llm_providers	f	FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE
CON	user_group_llm_providers	f	FOREIGN KEY (provider_id) REFERENCES llm_providers(id) ON DELETE CASCADE
CON	user_group_llm_providers	n	NOT NULL assigned_at
CON	user_group_llm_providers	n	NOT NULL group_id
CON	user_group_llm_providers	n	NOT NULL id
CON	user_group_llm_providers	n	NOT NULL provider_id
CON	user_group_llm_providers	p	PRIMARY KEY (id)
CON	user_group_llm_providers	u	UNIQUE (group_id, provider_id)
CON	user_group_mcp_servers	f	FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE
CON	user_group_mcp_servers	f	FOREIGN KEY (mcp_server_id) REFERENCES mcp_servers(id) ON DELETE CASCADE
CON	user_group_mcp_servers	n	NOT NULL assigned_at
CON	user_group_mcp_servers	n	NOT NULL group_id
CON	user_group_mcp_servers	n	NOT NULL mcp_server_id
CON	user_group_mcp_servers	p	PRIMARY KEY (group_id, mcp_server_id)
CON	user_groups	f	FOREIGN KEY (assigned_by) REFERENCES users(id) ON DELETE SET NULL
CON	user_groups	f	FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE
CON	user_groups	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	user_groups	n	NOT NULL assigned_at
CON	user_groups	n	NOT NULL group_id
CON	user_groups	n	NOT NULL user_id
CON	user_groups	p	PRIMARY KEY (user_id, group_id)
CON	user_lit_search_connector_keys	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	user_lit_search_connector_keys	n	NOT NULL connector
CON	user_lit_search_connector_keys	n	NOT NULL created_at
CON	user_lit_search_connector_keys	n	NOT NULL id
CON	user_lit_search_connector_keys	n	NOT NULL updated_at
CON	user_lit_search_connector_keys	n	NOT NULL user_id
CON	user_lit_search_connector_keys	p	PRIMARY KEY (id)
CON	user_lit_search_connector_keys	u	UNIQUE (user_id, connector)
CON	user_llm_provider_api_keys	f	FOREIGN KEY (provider_id) REFERENCES llm_providers(id) ON DELETE CASCADE
CON	user_llm_provider_api_keys	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	user_llm_provider_api_keys	n	NOT NULL created_at
CON	user_llm_provider_api_keys	n	NOT NULL id
CON	user_llm_provider_api_keys	n	NOT NULL provider_id
CON	user_llm_provider_api_keys	n	NOT NULL updated_at
CON	user_llm_provider_api_keys	n	NOT NULL user_id
CON	user_llm_provider_api_keys	p	PRIMARY KEY (id)
CON	user_llm_provider_api_keys	u	UNIQUE (user_id, provider_id)
CON	user_mcp_defaults	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	user_mcp_defaults	n	NOT NULL approval_mode
CON	user_mcp_defaults	n	NOT NULL auto_approved_tools
CON	user_mcp_defaults	n	NOT NULL created_at
CON	user_mcp_defaults	n	NOT NULL disabled_servers
CON	user_mcp_defaults	n	NOT NULL id
CON	user_mcp_defaults	n	NOT NULL updated_at
CON	user_mcp_defaults	n	NOT NULL user_id
CON	user_mcp_defaults	p	PRIMARY KEY (id)
CON	user_mcp_defaults	u	UNIQUE (user_id)
CON	user_memories	c	CHECK (confidence >= 0 AND confidence <= 100)
CON	user_memories	c	CHECK (importance >= 0 AND importance <= 100)
CON	user_memories	c	CHECK (kind = ANY (ARRAY['preference'::text, 'fact'::text, 'goal'::text, 'relationship'::text, 'other'::text]))
CON	user_memories	c	CHECK (scope = ANY (ARRAY['user'::text, 'project'::text, 'conversation'::text]))
CON	user_memories	c	CHECK (scope = 'user'::text AND project_id IS NULL AND conversation_id IS NULL OR scope = 'project'::text AND project_id IS NOT NULL AND conversation_id IS NULL OR scope = 'conversation'::text AND project_id IS NULL AND conversation_id IS NOT NULL)
CON	user_memories	c	CHECK (source = ANY (ARRAY['extraction'::text, 'mcp_tool'::text, 'manual'::text]))
CON	user_memories	f	FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
CON	user_memories	f	FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
CON	user_memories	f	FOREIGN KEY (source_message_id) REFERENCES messages(id) ON DELETE SET NULL
CON	user_memories	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	user_memories	n	NOT NULL confidence
CON	user_memories	n	NOT NULL content
CON	user_memories	n	NOT NULL created_at
CON	user_memories	n	NOT NULL id
CON	user_memories	n	NOT NULL importance
CON	user_memories	n	NOT NULL kind
CON	user_memories	n	NOT NULL metadata
CON	user_memories	n	NOT NULL recall_count
CON	user_memories	n	NOT NULL scope
CON	user_memories	n	NOT NULL source
CON	user_memories	n	NOT NULL updated_at
CON	user_memories	n	NOT NULL user_id
CON	user_memories	p	PRIMARY KEY (id)
CON	user_memory_settings	c	CHECK (max_memories > 0 AND max_memories <= 100000)
CON	user_memory_settings	f	FOREIGN KEY (extraction_model_id) REFERENCES llm_models(id) ON DELETE SET NULL
CON	user_memory_settings	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	user_memory_settings	n	NOT NULL created_at
CON	user_memory_settings	n	NOT NULL extraction_enabled
CON	user_memory_settings	n	NOT NULL max_memories
CON	user_memory_settings	n	NOT NULL retrieval_enabled
CON	user_memory_settings	n	NOT NULL updated_at
CON	user_memory_settings	n	NOT NULL user_id
CON	user_memory_settings	p	PRIMARY KEY (user_id)
CON	user_onboarding	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	user_onboarding	n	NOT NULL completed_guide_ids
CON	user_onboarding	n	NOT NULL completed_step_ids
CON	user_onboarding	n	NOT NULL created_at
CON	user_onboarding	n	NOT NULL updated_at
CON	user_onboarding	n	NOT NULL user_id
CON	user_onboarding	p	PRIMARY KEY (user_id)
CON	user_web_search_provider_keys	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	user_web_search_provider_keys	n	NOT NULL created_at
CON	user_web_search_provider_keys	n	NOT NULL id
CON	user_web_search_provider_keys	n	NOT NULL provider
CON	user_web_search_provider_keys	n	NOT NULL updated_at
CON	user_web_search_provider_keys	n	NOT NULL user_id
CON	user_web_search_provider_keys	p	PRIMARY KEY (id)
CON	user_web_search_provider_keys	u	UNIQUE (user_id, provider)
CON	users	n	NOT NULL created_at
CON	users	n	NOT NULL email
CON	users	n	NOT NULL email_verified
CON	users	n	NOT NULL id
CON	users	n	NOT NULL is_active
CON	users	n	NOT NULL is_admin
CON	users	n	NOT NULL permissions
CON	users	n	NOT NULL updated_at
CON	users	n	NOT NULL username
CON	users	p	PRIMARY KEY (id)
CON	users	u	UNIQUE (email)
CON	users	u	UNIQUE (username)
CON	voice_models	c	CHECK (source::text = ANY (ARRAY['catalog'::character varying, 'url'::character varying, 'upload'::character varying]::text[]))
CON	voice_models	n	NOT NULL created_at
CON	voice_models	n	NOT NULL filename
CON	voice_models	n	NOT NULL id
CON	voice_models	n	NOT NULL name
CON	voice_models	n	NOT NULL size_bytes
CON	voice_models	n	NOT NULL source
CON	voice_models	n	NOT NULL verified
CON	voice_models	p	PRIMARY KEY (id)
CON	voice_models	u	UNIQUE (filename)
CON	voice_runtime_instance	c	CHECK (id = true)
CON	voice_runtime_instance	c	CHECK (state::text = ANY (ARRAY['starting'::character varying, 'healthy'::character varying, 'unhealthy'::character varying, 'crashed'::character varying, 'restarting'::character varying, 'failed'::character varying, 'stopped'::character varying]::text[]))
CON	voice_runtime_instance	c	CHECK (status::text = ANY (ARRAY['stopped'::character varying, 'running'::character varying]::text[]))
CON	voice_runtime_instance	f	FOREIGN KEY (runtime_version_id) REFERENCES voice_runtime_versions(id) ON DELETE SET NULL
CON	voice_runtime_instance	n	NOT NULL created_at
CON	voice_runtime_instance	n	NOT NULL id
CON	voice_runtime_instance	n	NOT NULL restart_attempts
CON	voice_runtime_instance	n	NOT NULL state
CON	voice_runtime_instance	n	NOT NULL state_changed_at
CON	voice_runtime_instance	n	NOT NULL status
CON	voice_runtime_instance	n	NOT NULL updated_at
CON	voice_runtime_instance	p	PRIMARY KEY (id)
CON	voice_runtime_settings	c	CHECK (auto_start_timeout_secs >= 1 AND auto_start_timeout_secs <= 600)
CON	voice_runtime_settings	c	CHECK (drain_timeout_secs >= 1 AND drain_timeout_secs <= 600)
CON	voice_runtime_settings	c	CHECK (idle_unload_secs >= 0 AND idle_unload_secs <= 86400)
CON	voice_runtime_settings	c	CHECK (id = true)
CON	voice_runtime_settings	c	CHECK (max_clip_seconds >= 1 AND max_clip_seconds <= 3600)
CON	voice_runtime_settings	c	CHECK (max_upload_bytes >= 1024 AND max_upload_bytes <= 67108864)
CON	voice_runtime_settings	c	CHECK (stream_interval_ms >= 300 AND stream_interval_ms <= 10000)
CON	voice_runtime_settings	c	CHECK (stream_max_decode_secs >= 5 AND stream_max_decode_secs <= 600)
CON	voice_runtime_settings	n	NOT NULL auto_start_timeout_secs
CON	voice_runtime_settings	n	NOT NULL created_at
CON	voice_runtime_settings	n	NOT NULL drain_timeout_secs
CON	voice_runtime_settings	n	NOT NULL enabled
CON	voice_runtime_settings	n	NOT NULL id
CON	voice_runtime_settings	n	NOT NULL idle_unload_secs
CON	voice_runtime_settings	n	NOT NULL language
CON	voice_runtime_settings	n	NOT NULL max_clip_seconds
CON	voice_runtime_settings	n	NOT NULL max_upload_bytes
CON	voice_runtime_settings	n	NOT NULL model
CON	voice_runtime_settings	n	NOT NULL model_source_repo
CON	voice_runtime_settings	n	NOT NULL streaming_enabled
CON	voice_runtime_settings	n	NOT NULL stream_interval_ms
CON	voice_runtime_settings	n	NOT NULL stream_max_decode_secs
CON	voice_runtime_settings	n	NOT NULL updated_at
CON	voice_runtime_settings	p	PRIMARY KEY (id)
CON	voice_runtime_versions	n	NOT NULL arch
CON	voice_runtime_versions	n	NOT NULL backend
CON	voice_runtime_versions	n	NOT NULL binary_path
CON	voice_runtime_versions	n	NOT NULL created_at
CON	voice_runtime_versions	n	NOT NULL id
CON	voice_runtime_versions	n	NOT NULL is_system_default
CON	voice_runtime_versions	n	NOT NULL platform
CON	voice_runtime_versions	n	NOT NULL version
CON	voice_runtime_versions	p	PRIMARY KEY (id)
CON	voice_runtime_versions	u	UNIQUE (version, platform, arch, backend)
CON	web_search_providers	c	CHECK (provider <> ''::text)
CON	web_search_providers	n	NOT NULL config
CON	web_search_providers	n	NOT NULL created_at
CON	web_search_providers	n	NOT NULL provider
CON	web_search_providers	n	NOT NULL updated_at
CON	web_search_providers	p	PRIMARY KEY (provider)
CON	web_search_settings	c	CHECK (fetch_max_bytes >= 65536 AND fetch_max_bytes <= 104857600)
CON	web_search_settings	c	CHECK (fetch_max_chars >= 1000 AND fetch_max_chars <= 500000)
CON	web_search_settings	c	CHECK (id = true)
CON	web_search_settings	c	CHECK (max_results >= 1 AND max_results <= 20)
CON	web_search_settings	c	CHECK (request_timeout_secs >= 1 AND request_timeout_secs <= 120)
CON	web_search_settings	n	NOT NULL created_at
CON	web_search_settings	n	NOT NULL enabled
CON	web_search_settings	n	NOT NULL fetch_max_bytes
CON	web_search_settings	n	NOT NULL fetch_max_chars
CON	web_search_settings	n	NOT NULL id
CON	web_search_settings	n	NOT NULL max_results
CON	web_search_settings	n	NOT NULL provider_chain
CON	web_search_settings	n	NOT NULL request_timeout_secs
CON	web_search_settings	n	NOT NULL updated_at
CON	web_search_settings	p	PRIMARY KEY (id)
CON	workflow_runs	c	CHECK (invocation_source::text = ANY (ARRAY['manual'::character varying, 'conversation'::character varying, 'agent'::character varying, 'mcp_tool'::character varying, 'scheduled'::character varying]::text[]))
CON	workflow_runs	c	CHECK (run_kind::text = ANY (ARRAY['normal'::character varying, 'test'::character varying, 'dry_run'::character varying]::text[]))
CON	workflow_runs	c	CHECK (status::text = ANY (ARRAY['pending'::character varying, 'running'::character varying, 'waiting'::character varying, 'completed'::character varying, 'failed'::character varying, 'cancelled'::character varying]::text[]))
CON	workflow_runs	f	FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE SET NULL
CON	workflow_runs	f	FOREIGN KEY (model_id) REFERENCES llm_models(id) ON DELETE SET NULL
CON	workflow_runs	f	FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
CON	workflow_runs	f	FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
CON	workflow_runs	n	NOT NULL created_at
CON	workflow_runs	n	NOT NULL id
CON	workflow_runs	n	NOT NULL inputs_json
CON	workflow_runs	n	NOT NULL invocation_source
CON	workflow_runs	n	NOT NULL run_kind
CON	workflow_runs	n	NOT NULL status
CON	workflow_runs	n	NOT NULL step_artifacts_json
CON	workflow_runs	n	NOT NULL step_item_progress_json
CON	workflow_runs	n	NOT NULL step_logs_json
CON	workflow_runs	n	NOT NULL step_outputs_json
CON	workflow_runs	n	NOT NULL total_tokens
CON	workflow_runs	n	NOT NULL updated_at
CON	workflow_runs	n	NOT NULL user_id
CON	workflow_runs	n	NOT NULL workflow_id
CON	workflow_runs	p	PRIMARY KEY (id)
CON	workflows	c	CHECK (scope::text = ANY (ARRAY['user'::character varying, 'system'::character varying]::text[]))
CON	workflows	c	CHECK (scope::text = 'user'::text AND owner_user_id IS NOT NULL OR scope::text = 'system'::text AND owner_user_id IS NULL)
CON	workflows	f	FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
CON	workflows	f	FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL
CON	workflows	f	FOREIGN KEY (owner_user_id) REFERENCES users(id) ON DELETE CASCADE
CON	workflows	n	NOT NULL bundle_sha256
CON	workflows	n	NOT NULL bundle_size_bytes
CON	workflows	n	NOT NULL created_at
CON	workflows	n	NOT NULL enabled
CON	workflows	n	NOT NULL entry_point
CON	workflows	n	NOT NULL ephemeral
CON	workflows	n	NOT NULL extracted_path
CON	workflows	n	NOT NULL file_count
CON	workflows	n	NOT NULL id
CON	workflows	n	NOT NULL is_dev
CON	workflows	n	NOT NULL name
CON	workflows	n	NOT NULL scope
CON	workflows	n	NOT NULL tags
CON	workflows	n	NOT NULL updated_at
CON	workflows	p	PRIMARY KEY (id)
IDX	assistant_core_memory		CREATE INDEX ON public.assistant_core_memory USING btree (user_id, assistant_id)
IDX	assistants		CREATE INDEX ON public.assistants USING btree (created_by)
IDX	assistants		CREATE INDEX ON public.assistants USING btree (created_by) WHERE ((is_default = true) AND (enabled = true))
IDX	assistants		CREATE INDEX ON public.assistants USING btree (enabled)
IDX	assistants		CREATE INDEX ON public.assistants USING btree (is_default)
IDX	assistants		CREATE INDEX ON public.assistants USING btree (is_template)
IDX	assistants		CREATE INDEX ON public.assistants USING btree (name)
IDX	auth_providers		CREATE INDEX ON public.auth_providers USING btree (enabled)
IDX	bibliography_entries		CREATE INDEX ON public.bibliography_entries USING btree (user_id)
IDX	bibliography_entries		CREATE INDEX ON public.bibliography_entries USING gin (content_tsv)
IDX	bibliography_entries		CREATE UNIQUE INDEX ON public.bibliography_entries USING btree (user_id, citation_key)
IDX	bibliography_entries		CREATE UNIQUE INDEX ON public.bibliography_entries USING btree (user_id, dedup_fingerprint) WHERE ((doi IS NULL) AND (pmid IS NULL) AND (dedup_fingerprint IS NOT NULL))
IDX	bibliography_entries		CREATE UNIQUE INDEX ON public.bibliography_entries USING btree (user_id, lower(doi)) WHERE (doi IS NOT NULL)
IDX	bibliography_entries		CREATE UNIQUE INDEX ON public.bibliography_entries USING btree (user_id, pmid) WHERE (pmid IS NOT NULL)
IDX	branch_messages		CREATE INDEX ON public.branch_messages USING btree (branch_id, created_at)
IDX	branch_messages		CREATE INDEX ON public.branch_messages USING btree (message_id)
IDX	branches		CREATE INDEX ON public.branches USING btree (conversation_id)
IDX	branches		CREATE INDEX ON public.branches USING btree (created_from_message_id)
IDX	branches		CREATE INDEX ON public.branches USING btree (parent_branch_id)
IDX	code_sandbox_rootfs_artifacts		CREATE INDEX ON public.code_sandbox_rootfs_artifacts USING btree (arch, flavor)
IDX	code_sandbox_rootfs_artifacts		CREATE INDEX ON public.code_sandbox_rootfs_artifacts USING btree (version)
IDX	conversation_deliverables		CREATE INDEX ON public.conversation_deliverables USING btree (file_id)
IDX	conversation_knowledge_bases		CREATE INDEX ON public.conversation_knowledge_bases USING btree (knowledge_base_id)
IDX	conversation_skill_overrides		CREATE INDEX ON public.conversation_skill_overrides USING btree (conversation_id)
IDX	conversations		CREATE INDEX ON public.conversations USING btree (created_at DESC)
IDX	conversations		CREATE INDEX ON public.conversations USING btree (model_id)
IDX	conversations		CREATE INDEX ON public.conversations USING btree (user_id)
IDX	download_instances		CREATE INDEX ON public.download_instances USING btree (created_at DESC)
IDX	download_instances		CREATE INDEX ON public.download_instances USING btree (provider_id)
IDX	download_instances		CREATE INDEX ON public.download_instances USING btree (repository_id)
IDX	download_instances		CREATE INDEX ON public.download_instances USING btree (status)
IDX	download_instances		CREATE UNIQUE INDEX ON public.download_instances USING btree (repository_id, provider_id, ((request_data ->> 'repository_path'::text)), ((request_data ->> 'main_filename'::text))) WHERE ((status)::text = ANY ((ARRAY['pending'::character varying, 'downloading'::character varying])::text[]))
IDX	file_chunks		CREATE INDEX ON public.file_chunks USING btree (file_id)
IDX	file_chunks		CREATE INDEX ON public.file_chunks USING gin (content_tsv)
IDX	file_chunks		CREATE INDEX ON public.file_chunks USING hnsw (embedding halfvec_cosine_ops)
IDX	file_index_state		CREATE INDEX ON public.file_index_state USING btree (status)
IDX	file_index_state		CREATE INDEX ON public.file_index_state USING btree (user_id)
IDX	file_versions		CREATE INDEX ON public.file_versions USING btree (blob_version_id)
IDX	file_versions		CREATE INDEX ON public.file_versions USING btree (file_id, version DESC)
IDX	file_versions		CREATE UNIQUE INDEX ON public.file_versions USING btree (file_id) WHERE is_head
IDX	files		CREATE INDEX ON public.files USING btree (checksum)
IDX	files		CREATE INDEX ON public.files USING btree (created_at DESC)
IDX	files		CREATE INDEX ON public.files USING btree (file_size)
IDX	files		CREATE INDEX ON public.files USING btree (mime_type)
IDX	files		CREATE INDEX ON public.files USING btree (user_id)
IDX	files		CREATE INDEX ON public.files USING btree (workflow_run_id) WHERE (workflow_run_id IS NOT NULL)
IDX	files		CREATE INDEX ON public.files USING gin (processing_metadata)
IDX	group_skills		CREATE INDEX ON public.group_skills USING btree (skill_id)
IDX	group_workflows		CREATE INDEX ON public.group_workflows USING btree (workflow_id)
IDX	groups		CREATE INDEX ON public.groups USING btree (name)
IDX	groups		CREATE INDEX ON public.groups USING gin (permissions)
IDX	hub_entities		CREATE INDEX ON public.hub_entities USING btree (created_by) WHERE (created_by IS NOT NULL)
IDX	hub_entities		CREATE INDEX ON public.hub_entities USING btree (entity_type, entity_id)
IDX	hub_entities		CREATE INDEX ON public.hub_entities USING btree (hub_id, entity_type)
IDX	hub_entities		CREATE UNIQUE INDEX ON public.hub_entities USING btree (hub_id) WHERE (((entity_type)::text = 'assistant'::text) AND (created_by IS NULL))
IDX	hub_entities		CREATE UNIQUE INDEX ON public.hub_entities USING btree (hub_id) WHERE (((entity_type)::text = 'mcp_server'::text) AND (created_by IS NULL))
IDX	knowledge_base_documents		CREATE INDEX ON public.knowledge_base_documents USING btree (file_id)
IDX	knowledge_bases		CREATE INDEX ON public.knowledge_bases USING btree (user_id)
IDX	knowledge_bases		CREATE UNIQUE INDEX ON public.knowledge_bases USING btree (user_id, lower(name))
IDX	lit_fulltext_cache		CREATE INDEX ON public.lit_fulltext_cache USING btree (last_accessed_at)
IDX	lit_fulltext_cache		CREATE UNIQUE INDEX ON public.lit_fulltext_cache USING btree (arxiv_id) WHERE (arxiv_id IS NOT NULL)
IDX	lit_fulltext_cache		CREATE UNIQUE INDEX ON public.lit_fulltext_cache USING btree (doi) WHERE (doi IS NOT NULL)
IDX	lit_fulltext_cache		CREATE UNIQUE INDEX ON public.lit_fulltext_cache USING btree (pmcid) WHERE (pmcid IS NOT NULL)
IDX	lit_fulltext_cache		CREATE UNIQUE INDEX ON public.lit_fulltext_cache USING btree (pmid) WHERE (pmid IS NOT NULL)
IDX	llm_model_files		CREATE INDEX ON public.llm_model_files USING btree (model_id)
IDX	llm_model_files		CREATE INDEX ON public.llm_model_files USING btree (upload_status)
IDX	llm_models		CREATE INDEX ON public.llm_models USING btree (created_at DESC)
IDX	llm_models		CREATE INDEX ON public.llm_models USING btree (enabled)
IDX	llm_models		CREATE INDEX ON public.llm_models USING btree (engine_type)
IDX	llm_models		CREATE INDEX ON public.llm_models USING btree (provider_id)
IDX	llm_models		CREATE INDEX ON public.llm_models USING btree (required_runtime_version_id) WHERE (required_runtime_version_id IS NOT NULL)
IDX	llm_models		CREATE INDEX ON public.llm_models USING btree (validation_status)
IDX	llm_provider_files		CREATE INDEX ON public.llm_provider_files USING btree (file_id)
IDX	llm_provider_files		CREATE INDEX ON public.llm_provider_files USING btree (provider_id)
IDX	llm_provider_files		CREATE INDEX ON public.llm_provider_files USING btree (((provider_metadata ->> 'expires_at'::text))) WHERE ((provider_metadata ->> 'expires_at'::text) IS NOT NULL)
IDX	llm_provider_files		CREATE INDEX ON public.llm_provider_files USING btree (upload_status)
IDX	llm_provider_files		CREATE INDEX ON public.llm_provider_files USING gin (provider_metadata)
IDX	llm_providers		CREATE INDEX ON public.llm_providers USING btree (default_runtime_version_id) WHERE (default_runtime_version_id IS NOT NULL)
IDX	llm_providers		CREATE INDEX ON public.llm_providers USING btree (enabled)
IDX	llm_providers		CREATE INDEX ON public.llm_providers USING btree (provider_type)
IDX	llm_repositories		CREATE INDEX ON public.llm_repositories USING btree (last_health_check_status) WHERE (last_health_check_status = 'unhealthy'::text)
IDX	llm_runtime_instances		CREATE INDEX ON public.llm_runtime_instances USING btree (last_used_at) WHERE ((status)::text = 'running'::text)
IDX	llm_runtime_instances		CREATE INDEX ON public.llm_runtime_instances USING btree (provider_id)
IDX	llm_runtime_instances		CREATE INDEX ON public.llm_runtime_instances USING btree (runtime_version_id)
IDX	llm_runtime_instances		CREATE INDEX ON public.llm_runtime_instances USING btree (state)
IDX	llm_runtime_instances		CREATE INDEX ON public.llm_runtime_instances USING btree (status)
IDX	llm_runtime_versions		CREATE INDEX ON public.llm_runtime_versions USING btree (engine)
IDX	llm_runtime_versions		CREATE INDEX ON public.llm_runtime_versions USING btree (is_system_default) WHERE (is_system_default = true)
IDX	mcp_servers		CREATE INDEX ON public.mcp_servers USING btree (enabled)
IDX	mcp_servers		CREATE INDEX ON public.mcp_servers USING btree (is_built_in)
IDX	mcp_servers		CREATE INDEX ON public.mcp_servers USING btree (is_system)
IDX	mcp_servers		CREATE INDEX ON public.mcp_servers USING btree (last_health_check_status) WHERE (last_health_check_status = 'unhealthy'::text)
IDX	mcp_servers		CREATE INDEX ON public.mcp_servers USING btree (transport_type)
IDX	mcp_servers		CREATE INDEX ON public.mcp_servers USING btree (user_id)
IDX	mcp_settings		CREATE INDEX ON public.mcp_settings USING btree (user_id)
IDX	mcp_tool_calls		CREATE INDEX ON public.mcp_tool_calls USING btree (conversation_id) WHERE (conversation_id IS NOT NULL)
IDX	mcp_tool_calls		CREATE INDEX ON public.mcp_tool_calls USING btree (created_at)
IDX	mcp_tool_calls		CREATE INDEX ON public.mcp_tool_calls USING btree (server_id)
IDX	mcp_tool_calls		CREATE INDEX ON public.mcp_tool_calls USING btree (user_id, created_at DESC)
IDX	mcp_tool_calls		CREATE INDEX ON public.mcp_tool_calls USING btree (workflow_run_id) WHERE (workflow_run_id IS NOT NULL)
IDX	memory_audit_log		CREATE INDEX ON public.memory_audit_log USING btree (memory_id) WHERE (memory_id IS NOT NULL)
IDX	memory_audit_log		CREATE INDEX ON public.memory_audit_log USING btree (user_id, created_at DESC)
IDX	message_contents		CREATE INDEX ON public.message_contents USING btree (content_type)
IDX	message_contents		CREATE INDEX ON public.message_contents USING btree (message_id)
IDX	message_contents		CREATE INDEX ON public.message_contents USING gin (content)
IDX	message_contents		CREATE UNIQUE INDEX ON public.message_contents USING btree (message_id, sequence_order)
IDX	messages		CREATE INDEX ON public.messages USING btree (created_at DESC)
IDX	messages		CREATE INDEX ON public.messages USING btree (originated_from_id)
IDX	messages		CREATE INDEX ON public.messages USING btree (role)
IDX	notifications		CREATE INDEX ON public.notifications USING btree (user_id, created_at DESC)
IDX	notifications		CREATE INDEX ON public.notifications USING btree (user_id) WHERE (read_at IS NULL)
IDX	oauth_sessions		CREATE INDEX ON public.oauth_sessions USING btree (expires_at)
IDX	oauth_sessions		CREATE INDEX ON public.oauth_sessions USING btree (state)
IDX	pending_account_links		CREATE INDEX ON public.pending_account_links USING btree (expires_at)
IDX	pending_account_links		CREATE INDEX ON public.pending_account_links USING btree (target_user_id)
IDX	project_bibliography		CREATE INDEX ON public.project_bibliography USING btree (entry_id)
IDX	project_conversations		CREATE INDEX ON public.project_conversations USING btree (project_id)
IDX	project_files		CREATE INDEX ON public.project_files USING btree (file_id)
IDX	project_knowledge_bases		CREATE INDEX ON public.project_knowledge_bases USING btree (knowledge_base_id)
IDX	projects		CREATE INDEX ON public.projects USING btree (updated_at DESC)
IDX	projects		CREATE INDEX ON public.projects USING btree (user_id)
IDX	refresh_tokens		CREATE INDEX ON public.refresh_tokens USING btree (expires_at)
IDX	refresh_tokens		CREATE INDEX ON public.refresh_tokens USING btree (user_id) WHERE (revoked_at IS NULL)
IDX	sandbox_workspace_files		CREATE INDEX ON public.sandbox_workspace_files USING btree (file_id)
IDX	scheduled_task_runs		CREATE INDEX ON public.scheduled_task_runs USING btree (scheduled_task_id, fired_at DESC)
IDX	scheduled_task_runs		CREATE INDEX ON public.scheduled_task_runs USING btree (user_id, fired_at DESC)
IDX	scheduled_tasks		CREATE INDEX ON public.scheduled_tasks USING btree (next_run_at) WHERE (enabled AND (next_run_at IS NOT NULL))
IDX	scheduled_tasks		CREATE INDEX ON public.scheduled_tasks USING btree (user_id, created_at DESC)
IDX	skills		CREATE INDEX ON public.skills USING btree (enabled) WHERE (enabled = true)
IDX	skills		CREATE INDEX ON public.skills USING btree (name)
IDX	skills		CREATE INDEX ON public.skills USING btree (owner_user_id) WHERE ((scope)::text = 'user'::text)
IDX	skills		CREATE UNIQUE INDEX ON public.skills USING btree (name, version, owner_user_id) WHERE ((scope)::text = 'user'::text)
IDX	skills		CREATE UNIQUE INDEX ON public.skills USING btree (name, version) WHERE ((scope)::text = 'system'::text)
IDX	skills		CREATE UNIQUE INDEX ON public.skills USING btree (name) WHERE ((scope)::text = 'built_in'::text)
IDX	tool_use_approvals		CREATE INDEX ON public.tool_use_approvals USING btree (branch_id)
IDX	tool_use_approvals		CREATE INDEX ON public.tool_use_approvals USING btree (branch_id, status)
IDX	tool_use_approvals		CREATE INDEX ON public.tool_use_approvals USING btree (conversation_id)
IDX	tool_use_approvals		CREATE INDEX ON public.tool_use_approvals USING btree (message_id)
IDX	tool_use_approvals		CREATE INDEX ON public.tool_use_approvals USING btree (server_id)
IDX	tool_use_approvals		CREATE INDEX ON public.tool_use_approvals USING btree (status)
IDX	tool_use_approvals		CREATE INDEX ON public.tool_use_approvals USING btree (user_id)
IDX	user_auth_links		CREATE INDEX ON public.user_auth_links USING btree (provider_id)
IDX	user_auth_links		CREATE INDEX ON public.user_auth_links USING btree (provider_id, external_id)
IDX	user_auth_links		CREATE INDEX ON public.user_auth_links USING btree (user_id)
IDX	user_group_llm_providers		CREATE INDEX ON public.user_group_llm_providers USING btree (group_id)
IDX	user_group_llm_providers		CREATE INDEX ON public.user_group_llm_providers USING btree (provider_id)
IDX	user_group_mcp_servers		CREATE INDEX ON public.user_group_mcp_servers USING btree (group_id)
IDX	user_group_mcp_servers		CREATE INDEX ON public.user_group_mcp_servers USING btree (mcp_server_id)
IDX	user_groups		CREATE INDEX ON public.user_groups USING btree (group_id)
IDX	user_groups		CREATE INDEX ON public.user_groups USING btree (user_id)
IDX	user_memories		CREATE INDEX ON public.user_memories USING btree (user_id, conversation_id) WHERE ((scope = 'conversation'::text) AND (deleted_at IS NULL))
IDX	user_memories		CREATE INDEX ON public.user_memories USING btree (user_id, created_at) WHERE (source = 'extraction'::text)
IDX	user_memories		CREATE INDEX ON public.user_memories USING btree (user_id, created_at) WHERE (source = 'extraction'::text)
IDX	user_memories		CREATE INDEX ON public.user_memories USING btree (user_id, project_id) WHERE ((scope = 'project'::text) AND (deleted_at IS NULL))
IDX	user_memories		CREATE INDEX ON public.user_memories USING btree (user_id, updated_at DESC) WHERE (deleted_at IS NULL)
IDX	user_memories		CREATE INDEX ON public.user_memories USING btree (user_id) WHERE (deleted_at IS NULL)
IDX	user_memories		CREATE INDEX ON public.user_memories USING btree (user_id) WHERE ((scope = 'user'::text) AND (deleted_at IS NULL))
IDX	user_memories		CREATE INDEX ON public.user_memories USING gin (content_tsv)
IDX	user_memories		CREATE INDEX ON public.user_memories USING gin (metadata)
IDX	user_memories		CREATE INDEX ON public.user_memories USING hnsw (embedding halfvec_cosine_ops)
IDX	users		CREATE INDEX ON public.users USING btree (created_at)
IDX	users		CREATE INDEX ON public.users USING btree (email)
IDX	users		CREATE INDEX ON public.users USING btree (is_active)
IDX	users		CREATE INDEX ON public.users USING btree (last_login_at)
IDX	users		CREATE INDEX ON public.users USING btree (lower((email)::text))
IDX	users		CREATE INDEX ON public.users USING btree (username)
IDX	users		CREATE INDEX ON public.users USING gin (permissions)
IDX	users		CREATE UNIQUE INDEX ON public.users USING btree (is_admin) WHERE (is_admin = true)
IDX	voice_models		CREATE UNIQUE INDEX ON public.voice_models USING btree (name)
IDX	voice_runtime_versions		CREATE UNIQUE INDEX ON public.voice_runtime_versions USING btree (is_system_default) WHERE (is_system_default = true)
IDX	workflow_runs		CREATE INDEX ON public.workflow_runs USING btree (conversation_id) WHERE (conversation_id IS NOT NULL)
IDX	workflow_runs		CREATE INDEX ON public.workflow_runs USING btree (created_at)
IDX	workflow_runs		CREATE INDEX ON public.workflow_runs USING btree (run_kind)
IDX	workflow_runs		CREATE INDEX ON public.workflow_runs USING btree (status)
IDX	workflow_runs		CREATE INDEX ON public.workflow_runs USING btree (user_id)
IDX	workflow_runs		CREATE INDEX ON public.workflow_runs USING btree (user_id, created_at DESC)
IDX	workflow_runs		CREATE INDEX ON public.workflow_runs USING btree (workflow_id)
IDX	workflow_runs		CREATE INDEX ON public.workflow_runs USING btree (workflow_id, user_id, created_at DESC)
IDX	workflows		CREATE INDEX ON public.workflows USING btree (conversation_id) WHERE (ephemeral = true)
IDX	workflows		CREATE INDEX ON public.workflows USING btree (name)
IDX	workflows		CREATE INDEX ON public.workflows USING btree (owner_user_id) WHERE ((scope)::text = 'user'::text)
IDX	workflows		CREATE UNIQUE INDEX ON public.workflows USING btree (name, version, owner_user_id) WHERE ((scope)::text = 'user'::text)
IDX	workflows		CREATE UNIQUE INDEX ON public.workflows USING btree (name, version) WHERE ((scope)::text = 'system'::text)
SEQ	lit_fulltext_cache_id_seq	bigint	inc=1 min=1 max=9223372036854775807 start=1 cycle=false
SEQ	memory_audit_log_id_seq	bigint	inc=1 min=1 max=9223372036854775807 start=1 cycle=false
FUNC	enforce_system_scope_for_group_skills		CREATE OR REPLACE FUNCTION public.enforce_system_scope_for_group_skills() ⏎  RETURNS trigger ⏎  LANGUAGE plpgsql ⏎ AS $function$ ⏎ BEGIN ⏎     IF (SELECT scope FROM skills WHERE id = NEW.skill_id) <> 'system' THEN ⏎         RAISE EXCEPTION 'group_skills: only system-scope skills can be assigned to groups (skill_id=%)', NEW.skill_id; ⏎     END IF; ⏎     RETURN NEW; ⏎ END; ⏎ $function$ ⏎ 
FUNC	enforce_system_scope_for_group_workflows		CREATE OR REPLACE FUNCTION public.enforce_system_scope_for_group_workflows() ⏎  RETURNS trigger ⏎  LANGUAGE plpgsql ⏎ AS $function$ ⏎ BEGIN ⏎     IF (SELECT scope FROM workflows WHERE id = NEW.workflow_id) <> 'system' THEN ⏎         RAISE EXCEPTION 'group_workflows: only system-scope workflows can be assigned to groups (workflow_id=%)', NEW.workflow_id; ⏎     END IF; ⏎     RETURN NEW; ⏎ END; ⏎ $function$ ⏎ 
FUNC	update_updated_at_column		CREATE OR REPLACE FUNCTION public.update_updated_at_column() ⏎  RETURNS trigger ⏎  LANGUAGE plpgsql ⏎ AS $function$ ⏎ BEGIN ⏎     NEW.updated_at = NOW(); ⏎     RETURN NEW; ⏎ END; ⏎ $function$ ⏎ 
TRIG	assistants	update_assistants_updated_at	CREATE TRIGGER update_assistants_updated_at BEFORE UPDATE ON public.assistants FOR EACH ROW EXECUTE FUNCTION update_updated_at_column()
TRIG	auth_providers	update_auth_providers_updated_at	CREATE TRIGGER update_auth_providers_updated_at BEFORE UPDATE ON public.auth_providers FOR EACH ROW EXECUTE FUNCTION update_updated_at_column()
TRIG	group_skills	group_skills_scope_check	CREATE TRIGGER group_skills_scope_check BEFORE INSERT OR UPDATE ON public.group_skills FOR EACH ROW EXECUTE FUNCTION enforce_system_scope_for_group_skills()
TRIG	group_workflows	group_workflows_scope_check	CREATE TRIGGER group_workflows_scope_check BEFORE INSERT OR UPDATE ON public.group_workflows FOR EACH ROW EXECUTE FUNCTION enforce_system_scope_for_group_workflows()
TRIG	groups	update_groups_updated_at	CREATE TRIGGER update_groups_updated_at BEFORE UPDATE ON public.groups FOR EACH ROW EXECUTE FUNCTION update_updated_at_column()
TRIG	llm_provider_files	update_llm_provider_files_updated_at	CREATE TRIGGER update_llm_provider_files_updated_at BEFORE UPDATE ON public.llm_provider_files FOR EACH ROW EXECUTE FUNCTION update_updated_at_column()
TRIG	llm_providers	update_llm_providers_updated_at	CREATE TRIGGER update_llm_providers_updated_at BEFORE UPDATE ON public.llm_providers FOR EACH ROW EXECUTE FUNCTION update_updated_at_column()
TRIG	llm_repositories	update_llm_repositories_updated_at	CREATE TRIGGER update_llm_repositories_updated_at BEFORE UPDATE ON public.llm_repositories FOR EACH ROW EXECUTE FUNCTION update_updated_at_column()
TRIG	mcp_settings	update_mcp_settings_updated_at	CREATE TRIGGER update_mcp_settings_updated_at BEFORE UPDATE ON public.mcp_settings FOR EACH ROW EXECUTE FUNCTION update_updated_at_column()
TRIG	mcp_tool_calls	update_mcp_tool_calls_updated_at	CREATE TRIGGER update_mcp_tool_calls_updated_at BEFORE UPDATE ON public.mcp_tool_calls FOR EACH ROW EXECUTE FUNCTION update_updated_at_column()
TRIG	projects	update_projects_updated_at	CREATE TRIGGER update_projects_updated_at BEFORE UPDATE ON public.projects FOR EACH ROW EXECUTE FUNCTION update_updated_at_column()
TRIG	tool_use_approvals	update_tool_use_approvals_updated_at	CREATE TRIGGER update_tool_use_approvals_updated_at BEFORE UPDATE ON public.tool_use_approvals FOR EACH ROW EXECUTE FUNCTION update_updated_at_column()
TRIG	user_auth_links	update_user_auth_links_updated_at	CREATE TRIGGER update_user_auth_links_updated_at BEFORE UPDATE ON public.user_auth_links FOR EACH ROW EXECUTE FUNCTION update_updated_at_column()
TRIG	user_mcp_defaults	update_user_mcp_defaults_updated_at	CREATE TRIGGER update_user_mcp_defaults_updated_at BEFORE UPDATE ON public.user_mcp_defaults FOR EACH ROW EXECUTE FUNCTION update_updated_at_column()
TRIG	users	update_users_updated_at	CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON public.users FOR EACH ROW EXECUTE FUNCTION update_updated_at_column()
