--
-- PostgreSQL database dump
--

\restrict z4RDTdKeyAR6vn3NocKseuwwDgmfsk8vWVTMZjaLN6j994Ihat9M5OgeOteptLx

-- Dumped from database version 18.4 (Debian 18.4-1.pgdg12+1)
-- Dumped by pg_dump version 18.4 (Debian 18.4-1.pgdg12+1)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET transaction_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: public; Type: SCHEMA; Schema: -; Owner: postgres
--

-- *not* creating schema, since initdb creates it


ALTER SCHEMA public OWNER TO postgres;

--
-- Name: SCHEMA public; Type: COMMENT; Schema: -; Owner: postgres
--

COMMENT ON SCHEMA public IS '';


--
-- Name: pgcrypto; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS pgcrypto WITH SCHEMA public;


--
-- Name: EXTENSION pgcrypto; Type: COMMENT; Schema: -; Owner: 
--

COMMENT ON EXTENSION pgcrypto IS 'cryptographic functions';


--
-- Name: vector; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS vector WITH SCHEMA public;


--
-- Name: EXTENSION vector; Type: COMMENT; Schema: -; Owner: 
--

COMMENT ON EXTENSION vector IS 'vector data type and ivfflat and hnsw access methods';


--
-- Name: enforce_system_scope_for_group_skills(); Type: FUNCTION; Schema: public; Owner: postgres
--

CREATE FUNCTION public.enforce_system_scope_for_group_skills() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF (SELECT scope FROM skills WHERE id = NEW.skill_id) <> 'system' THEN
        RAISE EXCEPTION 'group_skills: only system-scope skills can be assigned to groups (skill_id=%)', NEW.skill_id;
    END IF;
    RETURN NEW;
END;
$$;


ALTER FUNCTION public.enforce_system_scope_for_group_skills() OWNER TO postgres;

--
-- Name: enforce_system_scope_for_group_workflows(); Type: FUNCTION; Schema: public; Owner: postgres
--

CREATE FUNCTION public.enforce_system_scope_for_group_workflows() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF (SELECT scope FROM workflows WHERE id = NEW.workflow_id) <> 'system' THEN
        RAISE EXCEPTION 'group_workflows: only system-scope workflows can be assigned to groups (workflow_id=%)', NEW.workflow_id;
    END IF;
    RETURN NEW;
END;
$$;


ALTER FUNCTION public.enforce_system_scope_for_group_workflows() OWNER TO postgres;

--
-- Name: update_desktop_settings_updated_at(); Type: FUNCTION; Schema: public; Owner: postgres
--

CREATE FUNCTION public.update_desktop_settings_updated_at() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;


ALTER FUNCTION public.update_desktop_settings_updated_at() OWNER TO postgres;

--
-- Name: update_updated_at_column(); Type: FUNCTION; Schema: public; Owner: postgres
--

CREATE FUNCTION public.update_updated_at_column() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;


ALTER FUNCTION public.update_updated_at_column() OWNER TO postgres;

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: _sqlx_migrations; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public._sqlx_migrations (
    version bigint NOT NULL,
    description text NOT NULL,
    installed_on timestamp with time zone DEFAULT now() NOT NULL,
    success boolean NOT NULL,
    checksum bytea NOT NULL,
    execution_time bigint NOT NULL
);


ALTER TABLE public._sqlx_migrations OWNER TO postgres;

--
-- Name: assistant_core_memory; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.assistant_core_memory (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    assistant_id uuid NOT NULL,
    user_id uuid NOT NULL,
    block_label text NOT NULL,
    content text NOT NULL,
    char_limit integer DEFAULT 2000 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT assistant_core_memory_char_limit_check CHECK (((char_limit > 0) AND (char_limit <= 50000)))
);


ALTER TABLE public.assistant_core_memory OWNER TO postgres;

--
-- Name: assistants; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.assistants (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    instructions text,
    parameters jsonb DEFAULT '{}'::jsonb,
    created_by uuid,
    is_template boolean DEFAULT false NOT NULL,
    is_default boolean DEFAULT false NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT template_must_have_no_owner CHECK ((((is_template = true) AND (created_by IS NULL)) OR (is_template = false)))
);


ALTER TABLE public.assistants OWNER TO postgres;

--
-- Name: TABLE assistants; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.assistants IS 'Assistants with user-created and system template configurations';


--
-- Name: COLUMN assistants.name; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.assistants.name IS 'Unique name for the assistant within user scope';


--
-- Name: COLUMN assistants.description; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.assistants.description IS 'Brief description of the assistant purpose';


--
-- Name: COLUMN assistants.instructions; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.assistants.instructions IS 'System instructions for the AI assistant';


--
-- Name: COLUMN assistants.parameters; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.assistants.parameters IS 'Model parameters (temperature, max_tokens, etc.) as JSONB';


--
-- Name: COLUMN assistants.created_by; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.assistants.created_by IS 'User who created this assistant (NULL for templates)';


--
-- Name: COLUMN assistants.is_template; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.assistants.is_template IS 'Whether this is a system-wide template available to all users';


--
-- Name: COLUMN assistants.is_default; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.assistants.is_default IS 'Whether this is the default assistant for the user/template context';


--
-- Name: COLUMN assistants.enabled; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.assistants.enabled IS 'Whether this assistant is enabled (false means disabled/soft-deleted)';


--
-- Name: auth_providers; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.auth_providers (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(100) NOT NULL,
    provider_type character varying(50) NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    config jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    last_test_at timestamp with time zone,
    last_test_ok boolean,
    last_test_message text,
    client_secret_encrypted bytea
);


ALTER TABLE public.auth_providers OWNER TO postgres;

--
-- Name: bibliography_entries; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.bibliography_entries (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    csl_json jsonb NOT NULL,
    doi text,
    pmid text,
    pmcid text,
    arxiv_id text,
    title text,
    year integer,
    dedup_fingerprint text,
    citation_key text NOT NULL,
    verification_status text DEFAULT 'unverified'::text NOT NULL,
    verified_at timestamp with time zone,
    source text,
    content_tsv tsvector GENERATED ALWAYS AS (to_tsvector('english'::regconfig, COALESCE(title, ''::text))) STORED,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT bibliography_entries_verification_status_check CHECK ((verification_status = ANY (ARRAY['unverified'::text, 'verified'::text, 'mismatch'::text, 'not_found'::text])))
);


ALTER TABLE public.bibliography_entries OWNER TO postgres;

--
-- Name: branch_messages; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.branch_messages (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    branch_id uuid NOT NULL,
    message_id uuid NOT NULL,
    is_clone boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL
);


ALTER TABLE public.branch_messages OWNER TO postgres;

--
-- Name: TABLE branch_messages; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.branch_messages IS 'Junction table for messages and branches. Enables copy-on-write branching where messages can belong to multiple branches (via cloning). is_clone=true means message is referenced from another branch, is_clone=false means it was created in this branch.';


--
-- Name: branches; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.branches (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    conversation_id uuid NOT NULL,
    parent_branch_id uuid,
    created_from_message_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    fork_level text DEFAULT 'user'::text NOT NULL,
    CONSTRAINT branches_fork_level_check CHECK ((fork_level = ANY (ARRAY['user'::text, 'assistant'::text])))
);


ALTER TABLE public.branches OWNER TO postgres;

--
-- Name: code_sandbox_rootfs_artifacts; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.code_sandbox_rootfs_artifacts (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    version text NOT NULL,
    arch text NOT NULL,
    flavor text NOT NULL,
    package text NOT NULL,
    sha256 text NOT NULL,
    artifact_path text NOT NULL,
    cosign_bundle text,
    status text DEFAULT 'installed'::text NOT NULL,
    downloaded_at timestamp with time zone DEFAULT now() NOT NULL,
    last_used_at timestamp with time zone
);


ALTER TABLE public.code_sandbox_rootfs_artifacts OWNER TO postgres;

--
-- Name: code_sandbox_settings; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.code_sandbox_settings (
    id boolean DEFAULT true NOT NULL,
    memory_max_bytes bigint DEFAULT 536870912 NOT NULL,
    memory_swap_max_bytes bigint DEFAULT 0 NOT NULL,
    pids_max integer DEFAULT 256 NOT NULL,
    cpu_max text DEFAULT '100000 100000'::text NOT NULL,
    address_space_bytes bigint DEFAULT '4294967296'::bigint NOT NULL,
    fsize_bytes bigint DEFAULT 268435456 NOT NULL,
    nproc_max integer DEFAULT 256 NOT NULL,
    nofile_max integer DEFAULT 1024 NOT NULL,
    cpu_secs_max integer DEFAULT 1240 NOT NULL,
    timeout_secs integer DEFAULT 620 NOT NULL,
    vm_idle_evict_secs integer DEFAULT 900 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    mac_vm_vcpus integer DEFAULT 2 NOT NULL,
    mac_vm_ram_mib integer DEFAULT 2048 NOT NULL,
    vm_max_concurrent_execs integer DEFAULT 3 NOT NULL,
    current_rootfs_version text,
    CONSTRAINT address_space_bytes_positive CHECK ((address_space_bytes >= 16777216)),
    CONSTRAINT code_sandbox_settings_id_check CHECK ((id = true)),
    CONSTRAINT cpu_max_shape CHECK ((cpu_max ~ '^[0-9]+ [0-9]+$'::text)),
    CONSTRAINT cpu_secs_max_positive CHECK (((cpu_secs_max >= 10) AND (cpu_secs_max <= 86400))),
    CONSTRAINT fsize_bytes_positive CHECK ((fsize_bytes >= 1048576)),
    CONSTRAINT mac_vm_ram_mib_range CHECK (((mac_vm_ram_mib >= 256) AND (mac_vm_ram_mib <= 262144))),
    CONSTRAINT mac_vm_vcpus_range CHECK (((mac_vm_vcpus >= 1) AND (mac_vm_vcpus <= 128))),
    CONSTRAINT memory_max_bytes_positive CHECK ((memory_max_bytes >= 16777216)),
    CONSTRAINT memory_swap_max_bytes_nonneg CHECK ((memory_swap_max_bytes >= 0)),
    CONSTRAINT nofile_max_positive CHECK (((nofile_max >= 64) AND (nofile_max <= 1048576))),
    CONSTRAINT nproc_max_positive CHECK (((nproc_max >= 8) AND (nproc_max <= 100000))),
    CONSTRAINT pids_max_positive CHECK (((pids_max >= 8) AND (pids_max <= 100000))),
    CONSTRAINT timeout_secs_positive CHECK (((timeout_secs >= 5) AND (timeout_secs <= 86400))),
    CONSTRAINT vm_idle_evict_secs_nonneg CHECK ((vm_idle_evict_secs >= 0)),
    CONSTRAINT vm_max_concurrent_execs_range CHECK (((vm_max_concurrent_execs >= 1) AND (vm_max_concurrent_execs <= 1000)))
);


ALTER TABLE public.code_sandbox_settings OWNER TO postgres;

--
-- Name: TABLE code_sandbox_settings; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.code_sandbox_settings IS 'Singleton row of runtime-tunable code_sandbox resource limits. Plan 1 §6.';


--
-- Name: COLUMN code_sandbox_settings.mac_vm_vcpus; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.code_sandbox_settings.mac_vm_vcpus IS 'macOS libkrun microVM vCPU count (krun_set_vm_config). Replaces the VM_VCPUS const in mac_vm.rs.';


--
-- Name: COLUMN code_sandbox_settings.mac_vm_ram_mib; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.code_sandbox_settings.mac_vm_ram_mib IS 'macOS libkrun microVM RAM ceiling in MiB (krun_set_vm_config). Replaces VM_RAM_MIB.';


--
-- Name: COLUMN code_sandbox_settings.vm_max_concurrent_execs; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.code_sandbox_settings.vm_max_concurrent_execs IS 'Per-VM concurrent execute_command cap (macOS + WSL2). Replaces MAX_CONCURRENT_EXECS_PER_VM.';


--
-- Name: conversation_deliverables; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.conversation_deliverables (
    conversation_id uuid NOT NULL,
    file_id uuid NOT NULL,
    pinned boolean DEFAULT true NOT NULL,
    title text,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.conversation_deliverables OWNER TO postgres;

--
-- Name: conversation_knowledge_bases; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.conversation_knowledge_bases (
    conversation_id uuid NOT NULL,
    knowledge_base_id uuid NOT NULL,
    added_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.conversation_knowledge_bases OWNER TO postgres;

--
-- Name: conversation_memory_settings; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.conversation_memory_settings (
    conversation_id uuid NOT NULL,
    memory_mode text NOT NULL,
    CONSTRAINT conversation_memory_settings_memory_mode_check CHECK ((memory_mode = ANY (ARRAY['inherit'::text, 'on'::text, 'off'::text])))
);


ALTER TABLE public.conversation_memory_settings OWNER TO postgres;

--
-- Name: conversation_skill_overrides; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.conversation_skill_overrides (
    conversation_id uuid NOT NULL,
    skill_id uuid NOT NULL,
    hidden boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.conversation_skill_overrides OWNER TO postgres;

--
-- Name: conversation_summaries; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.conversation_summaries (
    branch_id uuid NOT NULL,
    summary_text text NOT NULL,
    summarized_up_to_id uuid,
    message_count integer DEFAULT 0 NOT NULL,
    model_used text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.conversation_summaries OWNER TO postgres;

--
-- Name: conversation_summarization_settings; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.conversation_summarization_settings (
    conversation_id uuid NOT NULL,
    summarization_mode text DEFAULT 'inherit'::text NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT conversation_summarization_settings_summarization_mode_check CHECK ((summarization_mode = ANY (ARRAY['inherit'::text, 'on'::text, 'off'::text])))
);


ALTER TABLE public.conversation_summarization_settings OWNER TO postgres;

--
-- Name: conversations; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.conversations (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    model_id uuid,
    title character varying(500),
    active_branch_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.conversations OWNER TO postgres;

--
-- Name: desktop_settings; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.desktop_settings (
    key text NOT NULL,
    value text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.desktop_settings OWNER TO postgres;

--
-- Name: download_instances; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.download_instances (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    provider_id uuid NOT NULL,
    repository_id uuid NOT NULL,
    request_data jsonb NOT NULL,
    status character varying(50) NOT NULL,
    progress_data jsonb DEFAULT '{}'::jsonb,
    error_message text,
    started_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    completed_at timestamp with time zone,
    model_id uuid,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    CONSTRAINT download_instances_status_check CHECK (((status)::text = ANY ((ARRAY['pending'::character varying, 'downloading'::character varying, 'completed'::character varying, 'failed'::character varying, 'cancelled'::character varying])::text[])))
);


ALTER TABLE public.download_instances OWNER TO postgres;

--
-- Name: file_chunks; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.file_chunks (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    file_id uuid NOT NULL,
    user_id uuid NOT NULL,
    blob_version_id uuid NOT NULL,
    version integer NOT NULL,
    page_number integer NOT NULL,
    chunk_index integer NOT NULL,
    char_start integer NOT NULL,
    char_end integer NOT NULL,
    content text NOT NULL,
    embedding public.halfvec(768),
    embedding_model text,
    content_tsv tsvector GENERATED ALWAYS AS (to_tsvector('simple'::regconfig, content)) STORED,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.file_chunks OWNER TO postgres;

--
-- Name: file_index_state; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.file_index_state (
    file_id uuid NOT NULL,
    user_id uuid NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    error text,
    chunk_count integer DEFAULT 0 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT file_index_state_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'indexing'::text, 'indexed'::text, 'failed'::text, 'no_text'::text])))
);


ALTER TABLE public.file_index_state OWNER TO postgres;

--
-- Name: file_rag_admin_settings; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.file_rag_admin_settings (
    id smallint DEFAULT 1 NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    embedding_model_id uuid,
    embedding_dimensions integer DEFAULT 768 NOT NULL,
    chunk_chars integer DEFAULT 1200 NOT NULL,
    chunk_overlap_chars integer DEFAULT 200 NOT NULL,
    max_chunks_per_file integer DEFAULT 5000 NOT NULL,
    default_top_k smallint DEFAULT 8 NOT NULL,
    cosine_threshold real DEFAULT 0.6 NOT NULL,
    semantic_enabled boolean DEFAULT true NOT NULL,
    fts_enabled boolean DEFAULT true NOT NULL,
    fts_dictionary text DEFAULT 'simple'::text NOT NULL,
    fts_rrf_k integer DEFAULT 60 NOT NULL,
    fts_candidate_multiplier integer DEFAULT 4 NOT NULL,
    fts_min_rank real DEFAULT 0.0 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    reranker_model_id uuid,
    rerank_enabled boolean DEFAULT false NOT NULL,
    rerank_candidate_k integer DEFAULT 30 NOT NULL,
    kb_max_documents integer DEFAULT 2000 NOT NULL,
    search_max_hit_chars integer DEFAULT 2000 NOT NULL,
    search_snippet_chars integer DEFAULT 160 NOT NULL,
    search_max_top_k smallint DEFAULT 50 NOT NULL,
    CONSTRAINT file_rag_admin_settings_check CHECK (((chunk_overlap_chars >= 0) AND (chunk_overlap_chars < chunk_chars))),
    CONSTRAINT file_rag_admin_settings_chunk_chars_check CHECK (((chunk_chars >= 200) AND (chunk_chars <= 8000))),
    CONSTRAINT file_rag_admin_settings_cosine_threshold_check CHECK (((cosine_threshold >= (0.0)::double precision) AND (cosine_threshold <= (2.0)::double precision))),
    CONSTRAINT file_rag_admin_settings_default_top_k_check CHECK (((default_top_k > 0) AND (default_top_k <= 50))),
    CONSTRAINT file_rag_admin_settings_embedding_dimensions_check CHECK (((embedding_dimensions > 0) AND (embedding_dimensions <= 4000))),
    CONSTRAINT file_rag_admin_settings_fts_candidate_multiplier_check CHECK (((fts_candidate_multiplier >= 1) AND (fts_candidate_multiplier <= 20))),
    CONSTRAINT file_rag_admin_settings_fts_dictionary_check CHECK ((fts_dictionary = ANY (ARRAY['simple'::text, 'english'::text, 'french'::text, 'german'::text, 'spanish'::text, 'italian'::text, 'portuguese'::text, 'russian'::text, 'dutch'::text, 'norwegian'::text, 'swedish'::text, 'danish'::text, 'finnish'::text, 'hungarian'::text, 'turkish'::text]))),
    CONSTRAINT file_rag_admin_settings_fts_min_rank_check CHECK (((fts_min_rank >= (0.0)::double precision) AND (fts_min_rank <= (1.0)::double precision))),
    CONSTRAINT file_rag_admin_settings_fts_rrf_k_check CHECK (((fts_rrf_k >= 1) AND (fts_rrf_k <= 1000))),
    CONSTRAINT file_rag_admin_settings_id_check CHECK ((id = 1)),
    CONSTRAINT file_rag_admin_settings_kb_max_documents_check CHECK (((kb_max_documents >= 1) AND (kb_max_documents <= 100000))),
    CONSTRAINT file_rag_admin_settings_max_chunks_per_file_check CHECK ((max_chunks_per_file > 0)),
    CONSTRAINT file_rag_admin_settings_rerank_candidate_k_check CHECK (((rerank_candidate_k >= 1) AND (rerank_candidate_k <= 200))),
    CONSTRAINT file_rag_admin_settings_search_max_hit_chars_check CHECK (((search_max_hit_chars >= 100) AND (search_max_hit_chars <= 100000))),
    CONSTRAINT file_rag_admin_settings_search_max_top_k_check CHECK (((search_max_top_k >= 1) AND (search_max_top_k <= 500))),
    CONSTRAINT file_rag_admin_settings_search_snippet_chars_check CHECK (((search_snippet_chars >= 20) AND (search_snippet_chars <= 4000)))
);


ALTER TABLE public.file_rag_admin_settings OWNER TO postgres;

--
-- Name: file_versions; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.file_versions (
    id uuid NOT NULL,
    file_id uuid NOT NULL,
    version integer NOT NULL,
    is_head boolean DEFAULT false NOT NULL,
    blob_version_id uuid NOT NULL,
    file_size bigint NOT NULL,
    mime_type character varying(100),
    checksum character varying(64),
    has_thumbnail boolean DEFAULT false NOT NULL,
    preview_page_count integer DEFAULT 0 NOT NULL,
    text_page_count integer DEFAULT 0 NOT NULL,
    processing_metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    source_message_id uuid,
    created_by character varying(10) NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.file_versions OWNER TO postgres;

--
-- Name: files; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.files (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    filename character varying(255) NOT NULL,
    file_size bigint NOT NULL,
    mime_type character varying(100),
    checksum character varying(64),
    has_thumbnail boolean DEFAULT false NOT NULL,
    preview_page_count integer DEFAULT 0 NOT NULL,
    text_page_count integer DEFAULT 0 NOT NULL,
    processing_metadata jsonb DEFAULT '{}'::jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by character varying(10) DEFAULT 'user'::character varying NOT NULL,
    current_version_id uuid NOT NULL,
    workflow_run_id uuid
);


ALTER TABLE public.files OWNER TO postgres;

--
-- Name: group_skills; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.group_skills (
    group_id uuid NOT NULL,
    skill_id uuid NOT NULL,
    assigned_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.group_skills OWNER TO postgres;

--
-- Name: group_workflows; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.group_workflows (
    group_id uuid NOT NULL,
    workflow_id uuid NOT NULL,
    assigned_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.group_workflows OWNER TO postgres;

--
-- Name: groups; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.groups (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(100) NOT NULL,
    description text,
    permissions text[] DEFAULT '{}'::text[] NOT NULL,
    is_system boolean DEFAULT false NOT NULL,
    is_active boolean DEFAULT true NOT NULL,
    is_default boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.groups OWNER TO postgres;

--
-- Name: host_mount_policy; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.host_mount_policy (
    id smallint DEFAULT 1 NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    allowed_prefixes text[] DEFAULT '{}'::text[] NOT NULL,
    allow_readwrite boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT host_mount_policy_id_check CHECK ((id = 1))
);


ALTER TABLE public.host_mount_policy OWNER TO postgres;

--
-- Name: TABLE host_mount_policy; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.host_mount_policy IS 'Singleton (id=1) deployment policy for the desktop host-folder mount feature: enabled toggle, allowed host path prefixes, read-write opt-in.';


--
-- Name: host_mounts; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.host_mounts (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    conversation_id uuid,
    project_id uuid,
    user_id uuid NOT NULL,
    mounts jsonb DEFAULT '[]'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT host_mounts_one_scope CHECK (((conversation_id IS NULL) <> (project_id IS NULL)))
);


ALTER TABLE public.host_mounts OWNER TO postgres;

--
-- Name: TABLE host_mounts; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.host_mounts IS 'Per-scope (conversation XOR project) list of host folders mounted into the code sandbox. Resolved at execute_command time with read-through fallback from conversation to its project.';


--
-- Name: hub_entities; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.hub_entities (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id uuid NOT NULL,
    hub_id character varying(255) NOT NULL,
    hub_category character varying(50) NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by uuid,
    hub_version character varying(32),
    CONSTRAINT valid_entity_type CHECK (((entity_type)::text = ANY ((ARRAY['assistant'::character varying, 'mcp_server'::character varying, 'llm_model'::character varying, 'skill'::character varying, 'workflow'::character varying])::text[]))),
    CONSTRAINT valid_hub_category CHECK (((hub_category)::text = ANY ((ARRAY['assistant'::character varying, 'mcp_server'::character varying, 'model'::character varying, 'skill'::character varying, 'workflow'::character varying])::text[])))
);


ALTER TABLE public.hub_entities OWNER TO postgres;

--
-- Name: TABLE hub_entities; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.hub_entities IS 'Tracks which entities were created from hub catalog';


--
-- Name: COLUMN hub_entities.entity_type; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.hub_entities.entity_type IS 'Type of entity: assistant, mcp_server, or llm_model';


--
-- Name: COLUMN hub_entities.entity_id; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.hub_entities.entity_id IS 'UUID of the created entity';


--
-- Name: COLUMN hub_entities.hub_id; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.hub_entities.hub_id IS 'ID from hub catalog (e.g., code-assistant-v1)';


--
-- Name: COLUMN hub_entities.hub_category; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.hub_entities.hub_category IS 'Category in hub: assistant, mcp_server, or model';


--
-- Name: COLUMN hub_entities.created_by; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.hub_entities.created_by IS 'User who created this entity from hub (NULL for system-wide like models)';


--
-- Name: COLUMN hub_entities.hub_version; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.hub_entities.hub_version IS 'Hub catalog version (semver) the entity was installed from. NULL for legacy rows.';


--
-- Name: hub_settings; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.hub_settings (
    id boolean DEFAULT true NOT NULL,
    pinned_version character varying(32),
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT hub_settings_id_check CHECK ((id = true))
);


ALTER TABLE public.hub_settings OWNER TO postgres;

--
-- Name: TABLE hub_settings; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.hub_settings IS 'Singleton deployment-wide hub catalog settings (admin-pinned version).';


--
-- Name: COLUMN hub_settings.pinned_version; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.hub_settings.pinned_version IS 'Admin-pinned catalog version (semver, no leading v). NULL = track latest GitHub release.';


--
-- Name: js_tool_settings; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.js_tool_settings (
    id boolean DEFAULT true NOT NULL,
    memory_bytes bigint DEFAULT 134217728 NOT NULL,
    max_stack_bytes bigint DEFAULT 524288 NOT NULL,
    wall_secs integer DEFAULT 300 NOT NULL,
    approval_timeout_secs integer DEFAULT 300 NOT NULL,
    max_concurrent_runs integer DEFAULT 8 NOT NULL,
    max_concurrent_dispatch integer DEFAULT 6 NOT NULL,
    max_trace_entries integer DEFAULT 256 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT approval_timeout_secs_range CHECK (((approval_timeout_secs >= 5) AND (approval_timeout_secs <= 3600))),
    CONSTRAINT js_tool_settings_id_check CHECK ((id = true)),
    CONSTRAINT max_concurrent_dispatch_range CHECK (((max_concurrent_dispatch >= 1) AND (max_concurrent_dispatch <= 64))),
    CONSTRAINT max_concurrent_runs_range CHECK (((max_concurrent_runs >= 1) AND (max_concurrent_runs <= 256))),
    CONSTRAINT max_stack_bytes_range CHECK (((max_stack_bytes >= 65536) AND (max_stack_bytes <= 67108864))),
    CONSTRAINT max_trace_entries_range CHECK (((max_trace_entries >= 1) AND (max_trace_entries <= 10000))),
    CONSTRAINT memory_bytes_range CHECK (((memory_bytes >= 16777216) AND (memory_bytes <= '4294967296'::bigint))),
    CONSTRAINT wall_secs_range CHECK (((wall_secs >= 1) AND (wall_secs <= 3600)))
);


ALTER TABLE public.js_tool_settings OWNER TO postgres;

--
-- Name: TABLE js_tool_settings; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.js_tool_settings IS 'Singleton row of runtime-tunable run_js (js_tool) limits. Mirrors code_sandbox_settings.';


--
-- Name: knowledge_base_documents; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.knowledge_base_documents (
    knowledge_base_id uuid NOT NULL,
    file_id uuid NOT NULL,
    added_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.knowledge_base_documents OWNER TO postgres;

--
-- Name: knowledge_bases; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.knowledge_bases (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    name text NOT NULL,
    description text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.knowledge_bases OWNER TO postgres;

--
-- Name: lit_fulltext_cache; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.lit_fulltext_cache (
    id bigint NOT NULL,
    doi text,
    pmid text,
    pmcid text,
    arxiv_id text,
    content_hash text,
    status text NOT NULL,
    source text,
    license text,
    version text,
    byte_size bigint DEFAULT 0 NOT NULL,
    fetched_at timestamp with time zone DEFAULT now() NOT NULL,
    last_accessed_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT lit_cache_has_id CHECK (((doi IS NOT NULL) OR (pmid IS NOT NULL) OR (pmcid IS NOT NULL) OR (arxiv_id IS NOT NULL)))
);


ALTER TABLE public.lit_fulltext_cache OWNER TO postgres;

--
-- Name: TABLE lit_fulltext_cache; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.lit_fulltext_cache IS 'Index for the shared on-disk full-text cache: any id -> content_hash (blob) + provenance + LRU. Deployment-wide; public OA content only.';


--
-- Name: lit_fulltext_cache_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.lit_fulltext_cache_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER SEQUENCE public.lit_fulltext_cache_id_seq OWNER TO postgres;

--
-- Name: lit_fulltext_cache_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.lit_fulltext_cache_id_seq OWNED BY public.lit_fulltext_cache.id;


--
-- Name: lit_search_connectors; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.lit_search_connectors (
    connector text NOT NULL,
    api_key text,
    api_key_encrypted bytea,
    config jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT lit_connector_nonempty CHECK ((connector <> ''::text))
);


ALTER TABLE public.lit_search_connectors OWNER TO postgres;

--
-- Name: TABLE lit_search_connectors; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.lit_search_connectors IS 'Per-connector {api_key, config} keyed by the Rust LitConnector registry name. New connectors = code-only, no migration.';


--
-- Name: lit_search_settings; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.lit_search_settings (
    id boolean DEFAULT true NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    enabled_connectors text[] DEFAULT ARRAY['europepmc'::text, 'crossref'::text, 'semanticscholar'::text, 'pubmed'::text, 'arxiv'::text] NOT NULL,
    max_results integer DEFAULT 25 NOT NULL,
    per_source_limit integer DEFAULT 50 NOT NULL,
    request_timeout_secs integer DEFAULT 30 NOT NULL,
    completeness_estimate_enabled boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT lit_max_results_range CHECK (((max_results >= 1) AND (max_results <= 200))),
    CONSTRAINT lit_per_source_limit_range CHECK (((per_source_limit >= 1) AND (per_source_limit <= 100))),
    CONSTRAINT lit_search_settings_id_check CHECK ((id = true)),
    CONSTRAINT lit_timeout_range CHECK (((request_timeout_secs >= 1) AND (request_timeout_secs <= 120)))
);


ALTER TABLE public.lit_search_settings OWNER TO postgres;

--
-- Name: TABLE lit_search_settings; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.lit_search_settings IS 'Singleton deployment-wide lit_search config (enable + active connectors + caps + completeness toggle).';


--
-- Name: llm_model_files; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.llm_model_files (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    model_id uuid NOT NULL,
    filename character varying(500) NOT NULL,
    file_path character varying(1000) NOT NULL,
    file_size_bytes bigint NOT NULL,
    file_type character varying(50) NOT NULL,
    upload_status character varying(50) DEFAULT 'pending'::character varying NOT NULL,
    uploaded_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT llm_model_files_upload_status_check CHECK (((upload_status)::text = ANY ((ARRAY['pending'::character varying, 'uploading'::character varying, 'completed'::character varying, 'failed'::character varying])::text[])))
);


ALTER TABLE public.llm_model_files OWNER TO postgres;

--
-- Name: llm_models; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.llm_models (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    provider_id uuid NOT NULL,
    name character varying(255) NOT NULL,
    display_name character varying(255) NOT NULL,
    description text,
    enabled boolean DEFAULT true NOT NULL,
    is_deprecated boolean DEFAULT false NOT NULL,
    is_active boolean DEFAULT false NOT NULL,
    capabilities jsonb DEFAULT '{}'::jsonb,
    parameters jsonb DEFAULT '{}'::jsonb,
    file_size_bytes bigint,
    validation_status character varying(50),
    validation_issues jsonb,
    engine_type character varying(50) DEFAULT 'mistralrs'::character varying NOT NULL,
    engine_settings jsonb,
    file_format character varying(20) DEFAULT 'safetensors'::character varying NOT NULL,
    port integer,
    pid integer,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    required_runtime_version_id uuid,
    CONSTRAINT check_engine_type CHECK (((engine_type)::text = ANY ((ARRAY['mistralrs'::character varying, 'llamacpp'::character varying, 'none'::character varying])::text[]))),
    CONSTRAINT check_file_format CHECK (((file_format)::text = ANY ((ARRAY['safetensors'::character varying, 'pytorch'::character varying, 'gguf'::character varying])::text[]))),
    CONSTRAINT llm_models_display_name_not_empty CHECK (((display_name)::text <> ''::text)),
    CONSTRAINT llm_models_validation_status_check CHECK (((validation_status)::text = ANY ((ARRAY['pending'::character varying, 'await_upload'::character varying, 'downloading'::character varying, 'processing'::character varying, 'completed'::character varying, 'failed'::character varying, 'valid'::character varying, 'invalid'::character varying, 'error'::character varying, 'validation_warning'::character varying])::text[])))
);


ALTER TABLE public.llm_models OWNER TO postgres;

--
-- Name: llm_provider_files; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.llm_provider_files (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    file_id uuid NOT NULL,
    provider_id uuid NOT NULL,
    provider_file_id character varying(512),
    provider_metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    upload_status character varying(50) DEFAULT 'pending'::character varying NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT llm_provider_files_upload_status_check CHECK (((upload_status)::text = ANY ((ARRAY['pending'::character varying, 'uploading'::character varying, 'completed'::character varying, 'failed'::character varying, 'expired'::character varying])::text[])))
);


ALTER TABLE public.llm_provider_files OWNER TO postgres;

--
-- Name: TABLE llm_provider_files; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.llm_provider_files IS 'Maps system files to provider-specific file IDs for caching and reuse';


--
-- Name: COLUMN llm_provider_files.provider_file_id; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.llm_provider_files.provider_file_id IS 'File ID/URI returned by providers Files API (Anthropic, Gemini)';


--
-- Name: COLUMN llm_provider_files.provider_metadata; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.llm_provider_files.provider_metadata IS 'Provider-specific metadata: upload time, expiration, workspace, errors';


--
-- Name: COLUMN llm_provider_files.upload_status; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.llm_provider_files.upload_status IS 'Current upload status: pending, uploading, completed, failed, expired';


--
-- Name: llm_providers; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.llm_providers (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    provider_type character varying(50) NOT NULL,
    enabled boolean DEFAULT false NOT NULL,
    api_key text,
    base_url character varying(512),
    built_in boolean DEFAULT false NOT NULL,
    proxy_settings jsonb DEFAULT '{}'::jsonb,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deployment_config jsonb DEFAULT '{"type": "local", "binary_path": null}'::jsonb,
    default_runtime_version_id uuid,
    api_key_encrypted bytea,
    CONSTRAINT llm_providers_provider_type_check CHECK (((provider_type)::text = ANY ((ARRAY['local'::character varying, 'openai'::character varying, 'anthropic'::character varying, 'groq'::character varying, 'gemini'::character varying, 'mistral'::character varying, 'deepseek'::character varying, 'huggingface'::character varying, 'custom'::character varying, 'openrouter'::character varying])::text[])))
);


ALTER TABLE public.llm_providers OWNER TO postgres;

--
-- Name: llm_repositories; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.llm_repositories (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    url character varying(512) NOT NULL,
    auth_type character varying(50) NOT NULL,
    auth_config jsonb DEFAULT '{}'::jsonb,
    enabled boolean DEFAULT true NOT NULL,
    built_in boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    auth_config_encrypted bytea,
    last_health_check_at timestamp with time zone,
    last_health_check_status text DEFAULT 'untested'::text NOT NULL,
    last_health_check_reason text,
    CONSTRAINT llm_repositories_auth_type_check CHECK (((auth_type)::text = ANY ((ARRAY['none'::character varying, 'api_key'::character varying, 'basic_auth'::character varying, 'bearer_token'::character varying])::text[]))),
    CONSTRAINT llm_repositories_last_health_check_status_check CHECK ((last_health_check_status = ANY (ARRAY['untested'::text, 'healthy'::text, 'unhealthy'::text])))
);


ALTER TABLE public.llm_repositories OWNER TO postgres;

--
-- Name: TABLE llm_repositories; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.llm_repositories IS 'LLM model repositories (Hugging Face, GitHub, custom sources)';


--
-- Name: COLUMN llm_repositories.auth_type; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.llm_repositories.auth_type IS 'Authentication type: none, api_key, basic_auth, bearer_token';


--
-- Name: COLUMN llm_repositories.auth_config; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.llm_repositories.auth_config IS 'JSON object containing auth credentials and optional test endpoint';


--
-- Name: COLUMN llm_repositories.enabled; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.llm_repositories.enabled IS 'Whether this repository is currently enabled for use';


--
-- Name: COLUMN llm_repositories.built_in; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.llm_repositories.built_in IS 'true for default repositories (Hugging Face, GitHub) - cannot be deleted';


--
-- Name: llm_runtime_instances; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.llm_runtime_instances (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    model_id uuid NOT NULL,
    provider_id uuid NOT NULL,
    local_port integer NOT NULL,
    base_url character varying(512) NOT NULL,
    status character varying(50) NOT NULL,
    error_message text,
    started_at timestamp with time zone DEFAULT now() NOT NULL,
    last_health_check timestamp with time zone,
    stopped_at timestamp with time zone,
    runtime_version_id uuid,
    state character varying(50) DEFAULT 'starting'::character varying NOT NULL,
    state_changed_at timestamp with time zone DEFAULT now() NOT NULL,
    restart_attempts integer DEFAULT 0 NOT NULL,
    last_failure_reason text,
    last_used_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT llm_runtime_instances_state_check CHECK (((state)::text = ANY ((ARRAY['starting'::character varying, 'healthy'::character varying, 'unhealthy'::character varying, 'crashed'::character varying, 'restarting'::character varying, 'failed'::character varying, 'stopped'::character varying])::text[]))),
    CONSTRAINT llm_runtime_instances_status_check CHECK (((status)::text = ANY ((ARRAY['starting'::character varying, 'running'::character varying, 'stopping'::character varying, 'stopped'::character varying, 'failed'::character varying])::text[])))
);


ALTER TABLE public.llm_runtime_instances OWNER TO postgres;

--
-- Name: llm_runtime_settings; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.llm_runtime_settings (
    id boolean DEFAULT true NOT NULL,
    idle_unload_secs integer DEFAULT 1800 NOT NULL,
    auto_start_timeout_secs integer DEFAULT 30 NOT NULL,
    drain_timeout_secs integer DEFAULT 30 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT llm_runtime_settings_auto_start_timeout_secs_check CHECK (((auto_start_timeout_secs >= 1) AND (auto_start_timeout_secs <= 600))),
    CONSTRAINT llm_runtime_settings_drain_timeout_secs_check CHECK (((drain_timeout_secs >= 1) AND (drain_timeout_secs <= 600))),
    CONSTRAINT llm_runtime_settings_id_check CHECK ((id = true)),
    CONSTRAINT llm_runtime_settings_idle_unload_secs_check CHECK (((idle_unload_secs >= 0) AND (idle_unload_secs <= 86400)))
);


ALTER TABLE public.llm_runtime_settings OWNER TO postgres;

--
-- Name: llm_runtime_versions; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.llm_runtime_versions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    engine character varying(50) NOT NULL,
    version character varying(100) NOT NULL,
    platform character varying(50) NOT NULL,
    arch character varying(50) NOT NULL,
    backend character varying(50) NOT NULL,
    binary_path text NOT NULL,
    is_system_default boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT llm_runtime_versions_engine_check CHECK (((engine)::text = ANY ((ARRAY['llamacpp'::character varying, 'mistralrs'::character varying])::text[])))
);


ALTER TABLE public.llm_runtime_versions OWNER TO postgres;

--
-- Name: magic_link_tokens; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.magic_link_tokens (
    token_hash text NOT NULL,
    user_id uuid NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    used_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.magic_link_tokens OWNER TO postgres;

--
-- Name: TABLE magic_link_tokens; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.magic_link_tokens IS 'One-time login tokens issued by the desktop admin for phone/browser logins via the Remote Access tunnel. Plaintext token returned ONCE on issue and never stored (SHA-256 hash is the primary key). Single-use, 5-min TTL.';


--
-- Name: mcp_server_oauth_configs; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.mcp_server_oauth_configs (
    server_id uuid NOT NULL,
    client_id text NOT NULL,
    client_secret text,
    scopes text,
    resource text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    client_secret_encrypted bytea
);


ALTER TABLE public.mcp_server_oauth_configs OWNER TO postgres;

--
-- Name: TABLE mcp_server_oauth_configs; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.mcp_server_oauth_configs IS 'OAuth 2.1 client_credentials config for external HTTP MCP servers (one row per server).';


--
-- Name: mcp_servers; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.mcp_servers (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid,
    name character varying(255) NOT NULL,
    display_name character varying(255) NOT NULL,
    description text,
    enabled boolean DEFAULT true NOT NULL,
    is_system boolean DEFAULT false NOT NULL,
    transport_type character varying(50) DEFAULT 'stdio'::character varying NOT NULL,
    command text,
    args jsonb DEFAULT '[]'::jsonb,
    environment_variables jsonb DEFAULT '{}'::jsonb,
    url text,
    headers jsonb DEFAULT '{}'::jsonb,
    timeout_seconds integer DEFAULT 30 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    supports_sampling boolean DEFAULT false NOT NULL,
    usage_mode character varying(50) DEFAULT 'auto'::character varying NOT NULL,
    max_concurrent_sessions integer,
    is_built_in boolean DEFAULT false NOT NULL,
    run_in_sandbox boolean DEFAULT false NOT NULL,
    environment_variables_encrypted jsonb DEFAULT '{}'::jsonb NOT NULL,
    environment_variables_secret_keys text[] DEFAULT '{}'::text[] NOT NULL,
    headers_encrypted jsonb DEFAULT '{}'::jsonb NOT NULL,
    headers_secret_keys text[] DEFAULT '{}'::text[] NOT NULL,
    last_health_check_at timestamp with time zone,
    last_health_check_status text DEFAULT 'untested'::text NOT NULL,
    last_health_check_reason text,
    sandbox_flavor character varying(32) DEFAULT 'full'::character varying NOT NULL,
    CONSTRAINT mcp_servers_last_health_check_status_check CHECK ((last_health_check_status = ANY (ARRAY['untested'::text, 'healthy'::text, 'unhealthy'::text]))),
    CONSTRAINT mcp_servers_usage_mode_check CHECK (((usage_mode)::text = ANY ((ARRAY['auto'::character varying, 'always'::character varying])::text[]))),
    CONSTRAINT system_server_must_have_no_owner CHECK ((((is_system = true) AND (user_id IS NULL)) OR ((is_system = false) AND (user_id IS NOT NULL)))),
    CONSTRAINT valid_transport_config CHECK (((((transport_type)::text = 'stdio'::text) AND (command IS NOT NULL)) OR (((transport_type)::text = ANY ((ARRAY['http'::character varying, 'sse'::character varying])::text[])) AND (url IS NOT NULL))))
);


ALTER TABLE public.mcp_servers OWNER TO postgres;

--
-- Name: COLUMN mcp_servers.run_in_sandbox; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.mcp_servers.run_in_sandbox IS 'When true AND is_system AND transport_type=''stdio'', launch the MCP subprocess inside the code_sandbox bwrap isolation. See server/src/modules/mcp/client/stdio.rs.';


--
-- Name: COLUMN mcp_servers.sandbox_flavor; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.mcp_servers.sandbox_flavor IS 'Rootfs flavor (KNOWN_FLAVORS, e.g. minimal/full) used when run_in_sandbox launches this stdio server inside the code_sandbox. Defaults to full. See server/src/modules/code_sandbox/mcp_spawn.rs.';


--
-- Name: mcp_settings; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.mcp_settings (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    conversation_id uuid,
    project_id uuid,
    user_id uuid NOT NULL,
    approval_mode character varying(50) DEFAULT 'manual_approve'::character varying NOT NULL,
    auto_approved_tools jsonb DEFAULT '[]'::jsonb NOT NULL,
    disabled_servers jsonb DEFAULT '[]'::jsonb NOT NULL,
    loop_settings jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT mcp_settings_one_scope CHECK (((conversation_id IS NULL) <> (project_id IS NULL)))
);


ALTER TABLE public.mcp_settings OWNER TO postgres;

--
-- Name: mcp_tool_calls; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.mcp_tool_calls (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    server_id uuid,
    server_name character varying(255) NOT NULL,
    is_built_in boolean DEFAULT false NOT NULL,
    user_id uuid NOT NULL,
    conversation_id uuid,
    branch_id uuid,
    message_id uuid,
    tool_use_id character varying(255),
    tool_name character varying(255) NOT NULL,
    arguments_json jsonb DEFAULT '{}'::jsonb NOT NULL,
    source character varying(20) DEFAULT 'chat'::character varying NOT NULL,
    status character varying(20) DEFAULT 'completed'::character varying NOT NULL,
    is_error boolean DEFAULT false NOT NULL,
    result_json jsonb,
    content_kinds text[] DEFAULT '{}'::text[] NOT NULL,
    result_bytes bigint DEFAULT 0 NOT NULL,
    error_message text,
    started_at timestamp with time zone DEFAULT now() NOT NULL,
    finished_at timestamp with time zone,
    duration_ms bigint,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    workflow_run_id uuid,
    CONSTRAINT mcp_tool_calls_source_check CHECK (((source)::text = ANY ((ARRAY['chat'::character varying, 'rest'::character varying, 'always'::character varying, 'sampling'::character varying, 'approval'::character varying, 'workflow'::character varying, 'script'::character varying])::text[]))),
    CONSTRAINT mcp_tool_calls_status_check CHECK (((status)::text = ANY ((ARRAY['completed'::character varying, 'failed'::character varying, 'timeout'::character varying, 'cancelled'::character varying])::text[])))
);


ALTER TABLE public.mcp_tool_calls OWNER TO postgres;

--
-- Name: mcp_user_policy; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.mcp_user_policy (
    id integer DEFAULT 1 NOT NULL,
    allowed_transports text[] DEFAULT ARRAY['http'::text, 'stdio'::text] NOT NULL,
    user_stdio_sandbox_flavor text DEFAULT 'full'::text,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_by uuid,
    tool_call_retention_days integer DEFAULT 90 NOT NULL,
    CONSTRAINT mcp_user_policy_id_check CHECK ((id = 1))
);


ALTER TABLE public.mcp_user_policy OWNER TO postgres;

--
-- Name: memory_admin_settings; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.memory_admin_settings (
    id smallint DEFAULT 1 NOT NULL,
    embedding_model_id uuid,
    embedding_dimensions integer DEFAULT 768 NOT NULL,
    default_extraction_model_id uuid,
    default_top_k smallint DEFAULT 8 NOT NULL,
    cosine_threshold real DEFAULT 0.6 NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    soft_delete_grace_days integer DEFAULT 30 NOT NULL,
    daily_extraction_quota integer DEFAULT 200 NOT NULL,
    fts_dictionary text DEFAULT 'simple'::text NOT NULL,
    fts_enabled boolean DEFAULT true NOT NULL,
    fts_rrf_k integer DEFAULT 60 NOT NULL,
    fts_candidate_multiplier integer DEFAULT 4 NOT NULL,
    fts_min_rank real DEFAULT 0.0 NOT NULL,
    fts_rebuild_started_at timestamp with time zone,
    fts_rebuild_completed_at timestamp with time zone,
    semantic_enabled boolean DEFAULT true NOT NULL,
    CONSTRAINT memory_admin_settings_cosine_threshold_check CHECK (((cosine_threshold >= (0.0)::double precision) AND (cosine_threshold <= (2.0)::double precision))),
    CONSTRAINT memory_admin_settings_daily_extraction_quota_check CHECK (((daily_extraction_quota >= 1) AND (daily_extraction_quota <= 10000))),
    CONSTRAINT memory_admin_settings_default_top_k_check CHECK (((default_top_k > 0) AND (default_top_k <= 100))),
    CONSTRAINT memory_admin_settings_embedding_dimensions_check CHECK (((embedding_dimensions > 0) AND (embedding_dimensions <= 16000))),
    CONSTRAINT memory_admin_settings_fts_candidate_multiplier_check CHECK (((fts_candidate_multiplier >= 1) AND (fts_candidate_multiplier <= 20))),
    CONSTRAINT memory_admin_settings_fts_dictionary_check CHECK ((fts_dictionary = ANY (ARRAY['simple'::text, 'english'::text, 'french'::text, 'german'::text, 'spanish'::text, 'italian'::text, 'portuguese'::text, 'russian'::text, 'dutch'::text, 'norwegian'::text, 'swedish'::text, 'danish'::text, 'finnish'::text, 'hungarian'::text, 'turkish'::text]))),
    CONSTRAINT memory_admin_settings_fts_min_rank_check CHECK (((fts_min_rank >= (0.0)::double precision) AND (fts_min_rank <= (1.0)::double precision))),
    CONSTRAINT memory_admin_settings_fts_rrf_k_check CHECK (((fts_rrf_k >= 1) AND (fts_rrf_k <= 1000))),
    CONSTRAINT memory_admin_settings_id_check CHECK ((id = 1)),
    CONSTRAINT memory_admin_settings_soft_delete_grace_days_check CHECK (((soft_delete_grace_days >= 1) AND (soft_delete_grace_days <= 365)))
);


ALTER TABLE public.memory_admin_settings OWNER TO postgres;

--
-- Name: memory_audit_log; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.memory_audit_log (
    id bigint NOT NULL,
    user_id uuid NOT NULL,
    memory_id uuid,
    op text NOT NULL,
    source text NOT NULL,
    content_snapshot text,
    actor_kind text DEFAULT 'user'::text NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT memory_audit_log_actor_kind_check CHECK ((actor_kind = ANY (ARRAY['user'::text, 'assistant'::text, 'admin'::text, 'system'::text]))),
    CONSTRAINT memory_audit_log_op_check CHECK ((op = ANY (ARRAY['ADD'::text, 'UPDATE'::text, 'DELETE'::text, 'BULK_DELETE'::text]))),
    CONSTRAINT memory_audit_log_source_check CHECK ((source = ANY (ARRAY['extraction'::text, 'mcp_tool'::text, 'manual'::text, 'admin'::text])))
);


ALTER TABLE public.memory_audit_log OWNER TO postgres;

--
-- Name: memory_audit_log_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.memory_audit_log_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER SEQUENCE public.memory_audit_log_id_seq OWNER TO postgres;

--
-- Name: memory_audit_log_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.memory_audit_log_id_seq OWNED BY public.memory_audit_log.id;


--
-- Name: message_assistant; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.message_assistant (
    message_id uuid NOT NULL,
    assistant_id uuid NOT NULL
);


ALTER TABLE public.message_assistant OWNER TO postgres;

--
-- Name: message_contents; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.message_contents (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    message_id uuid NOT NULL,
    content_type character varying(50) NOT NULL,
    content jsonb NOT NULL,
    sequence_order integer DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.message_contents OWNER TO postgres;

--
-- Name: message_mcp_servers; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.message_mcp_servers (
    message_id uuid NOT NULL,
    server_id uuid NOT NULL
);


ALTER TABLE public.message_mcp_servers OWNER TO postgres;

--
-- Name: messages; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.messages (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    role character varying(20) NOT NULL,
    originated_from_id uuid NOT NULL,
    edit_count integer DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    model_id uuid
);


ALTER TABLE public.messages OWNER TO postgres;

--
-- Name: COLUMN messages.originated_from_id; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.messages.originated_from_id IS 'UUID of the original message in an edit lineage. All edits of the same message share this ID.';


--
-- Name: COLUMN messages.edit_count; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.messages.edit_count IS 'Number of times any message in this edit lineage has been edited. Increments for all messages with the same originated_from_id.';


--
-- Name: notifications; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.notifications (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    kind text NOT NULL,
    title text NOT NULL,
    body text DEFAULT ''::text NOT NULL,
    interrupt boolean DEFAULT true NOT NULL,
    scheduled_task_id uuid,
    workflow_run_id uuid,
    conversation_id uuid,
    read_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.notifications OWNER TO postgres;

--
-- Name: TABLE notifications; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.notifications IS 'Durable owner-scoped notification inbox for background results (scheduler + future producers).';


--
-- Name: oauth_sessions; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.oauth_sessions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    state character varying(255) NOT NULL,
    provider_id uuid NOT NULL,
    pkce_verifier character varying(255),
    nonce character varying(255),
    redirect_uri text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    return_to text
);


ALTER TABLE public.oauth_sessions OWNER TO postgres;

--
-- Name: pending_account_links; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.pending_account_links (
    link_token character varying(255) NOT NULL,
    provider_id uuid NOT NULL,
    target_user_id uuid NOT NULL,
    external_id character varying(255) NOT NULL,
    external_email character varying(255),
    external_data jsonb,
    attempts integer DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    expires_at timestamp with time zone NOT NULL
);


ALTER TABLE public.pending_account_links OWNER TO postgres;

--
-- Name: project_bibliography; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.project_bibliography (
    project_id uuid NOT NULL,
    entry_id uuid NOT NULL,
    added_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.project_bibliography OWNER TO postgres;

--
-- Name: project_conversations; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.project_conversations (
    conversation_id uuid NOT NULL,
    project_id uuid NOT NULL,
    attached_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.project_conversations OWNER TO postgres;

--
-- Name: project_files; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.project_files (
    project_id uuid NOT NULL,
    file_id uuid NOT NULL,
    added_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.project_files OWNER TO postgres;

--
-- Name: project_knowledge_bases; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.project_knowledge_bases (
    project_id uuid NOT NULL,
    knowledge_base_id uuid NOT NULL,
    added_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.project_knowledge_bases OWNER TO postgres;

--
-- Name: projects; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.projects (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    instructions text,
    default_assistant_id uuid,
    default_model_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.projects OWNER TO postgres;

--
-- Name: refresh_tokens; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.refresh_tokens (
    jti uuid NOT NULL,
    user_id uuid NOT NULL,
    issued_at timestamp with time zone DEFAULT now() NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    revoked_at timestamp with time zone,
    rotated_to uuid
);


ALTER TABLE public.refresh_tokens OWNER TO postgres;

--
-- Name: COLUMN refresh_tokens.rotated_to; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.refresh_tokens.rotated_to IS 'Successor jti when revoked by rotation (30s grace for racing clients); NULL when revoked by logout.';


--
-- Name: remote_access_settings; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.remote_access_settings (
    id smallint DEFAULT 1 NOT NULL,
    ngrok_auth_token_enc bytea,
    ngrok_domain text,
    auto_start_tunnel boolean DEFAULT false NOT NULL,
    password_auth_enabled boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT remote_access_auto_start_requires_domain CHECK (((auto_start_tunnel = false) OR (ngrok_domain IS NOT NULL))),
    CONSTRAINT remote_access_settings_id_check CHECK ((id = 1))
);


ALTER TABLE public.remote_access_settings OWNER TO postgres;

--
-- Name: TABLE remote_access_settings; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.remote_access_settings IS 'Singleton config row (id=1) for the Remote Access feature: ngrok auth token + optional reserved domain + auto-start gate + password-auth opt-in.';


--
-- Name: sandbox_workspace_files; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.sandbox_workspace_files (
    conversation_id uuid NOT NULL,
    workspace_relpath text NOT NULL,
    file_id uuid NOT NULL,
    base_version_id uuid NOT NULL
);


ALTER TABLE public.sandbox_workspace_files OWNER TO postgres;

--
-- Name: scheduled_task_runs; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.scheduled_task_runs (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    scheduled_task_id uuid NOT NULL,
    user_id uuid NOT NULL,
    trigger text DEFAULT 'schedule'::text NOT NULL,
    status text NOT NULL,
    error_class text,
    error_message text,
    notification_id uuid,
    workflow_run_id uuid,
    conversation_id uuid,
    fired_at timestamp with time zone DEFAULT now() NOT NULL,
    finished_at timestamp with time zone,
    skipped_tools jsonb DEFAULT '[]'::jsonb NOT NULL,
    result_preview text,
    change_summary_json jsonb,
    CONSTRAINT scheduled_task_runs_status_check CHECK ((status = ANY (ARRAY['completed'::text, 'no_change'::text, 'failed'::text]))),
    CONSTRAINT scheduled_task_runs_trigger_check CHECK ((trigger = ANY (ARRAY['schedule'::text, 'run_now'::text, 'catchup'::text])))
);


ALTER TABLE public.scheduled_task_runs OWNER TO postgres;

--
-- Name: TABLE scheduled_task_runs; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.scheduled_task_runs IS 'Per-firing audit history for a scheduled task (Runs tab); excludes side-effect-free dry-runs.';


--
-- Name: COLUMN scheduled_task_runs.skipped_tools; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.scheduled_task_runs.skipped_tools IS 'Tools skipped during this firing because they were not permitted unattended (DEC-17.5); [] when none.';


--
-- Name: scheduled_tasks; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.scheduled_tasks (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    name character varying(255) NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    paused_reason text,
    target_kind text NOT NULL,
    workflow_id uuid,
    inputs_json jsonb DEFAULT '{}'::jsonb NOT NULL,
    assistant_id uuid,
    prompt text,
    model_id uuid,
    schedule_kind text NOT NULL,
    run_at timestamp with time zone,
    cron_expr text,
    timezone text DEFAULT 'UTC'::text NOT NULL,
    next_run_at timestamp with time zone,
    last_run_at timestamp with time zone,
    last_status text,
    consecutive_failures integer DEFAULT 0 NOT NULL,
    notify_mode text DEFAULT 'always'::text NOT NULL,
    notify_on text DEFAULT 'always'::text NOT NULL,
    last_result_fingerprint text,
    last_result_signature_json jsonb,
    bound_conversation_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    allowed_unattended_tools jsonb DEFAULT '[]'::jsonb NOT NULL,
    CONSTRAINT scheduled_tasks_notify_mode_check CHECK ((notify_mode = ANY (ARRAY['always'::text, 'silent'::text]))),
    CONSTRAINT scheduled_tasks_notify_on_check CHECK ((notify_on = ANY (ARRAY['always'::text, 'on_change'::text]))),
    CONSTRAINT scheduled_tasks_schedule_coherent CHECK ((((schedule_kind = 'once'::text) AND (run_at IS NOT NULL)) OR ((schedule_kind = 'recurring'::text) AND (cron_expr IS NOT NULL)))),
    CONSTRAINT scheduled_tasks_schedule_kind_check CHECK ((schedule_kind = ANY (ARRAY['once'::text, 'recurring'::text]))),
    CONSTRAINT scheduled_tasks_target_coherent CHECK ((((target_kind = 'workflow'::text) AND (workflow_id IS NOT NULL)) OR ((target_kind = 'prompt'::text) AND (prompt IS NOT NULL)))),
    CONSTRAINT scheduled_tasks_target_kind_check CHECK ((target_kind = ANY (ARRAY['workflow'::text, 'prompt'::text])))
);


ALTER TABLE public.scheduled_tasks OWNER TO postgres;

--
-- Name: TABLE scheduled_tasks; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.scheduled_tasks IS 'User-owned scheduled/recurring background tasks (workflow or prompt target); fired by the scheduler tick loop.';


--
-- Name: COLUMN scheduled_tasks.allowed_unattended_tools; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.scheduled_tasks.allowed_unattended_tools IS 'Per-task allow-list of MCP servers/tools that may run unattended without per-call approval (DEC-17); subset of the owner''s accessible servers. Empty = built-in read-only tools only.';


--
-- Name: scheduler_admin_settings; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.scheduler_admin_settings (
    id boolean DEFAULT true NOT NULL,
    max_active_tasks_per_user integer DEFAULT 20 NOT NULL,
    min_interval_seconds integer DEFAULT 300 NOT NULL,
    max_consecutive_failures integer DEFAULT 5 NOT NULL,
    notification_retention_days integer DEFAULT 30 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT scheduler_admin_settings_id_check CHECK ((id = true)),
    CONSTRAINT scheduler_max_active_range CHECK (((max_active_tasks_per_user >= 1) AND (max_active_tasks_per_user <= 1000))),
    CONSTRAINT scheduler_max_failures_range CHECK (((max_consecutive_failures >= 1) AND (max_consecutive_failures <= 100))),
    CONSTRAINT scheduler_min_interval_range CHECK (((min_interval_seconds >= 60) AND (min_interval_seconds <= 86400))),
    CONSTRAINT scheduler_retention_range CHECK (((notification_retention_days >= 0) AND (notification_retention_days <= 3650)))
);


ALTER TABLE public.scheduler_admin_settings OWNER TO postgres;

--
-- Name: TABLE scheduler_admin_settings; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.scheduler_admin_settings IS 'Singleton deployment-wide scheduler config (quota, cadence floor, failure cap, notification retention).';


--
-- Name: session_settings; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.session_settings (
    id boolean DEFAULT true NOT NULL,
    access_token_expiry_hours integer DEFAULT 24 NOT NULL,
    refresh_token_expiry_days integer DEFAULT 30 NOT NULL,
    seeded_from_config boolean DEFAULT false NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT access_token_expiry_hours_range CHECK (((access_token_expiry_hours >= 1) AND (access_token_expiry_hours <= 8760))),
    CONSTRAINT refresh_token_expiry_days_range CHECK (((refresh_token_expiry_days >= 1) AND (refresh_token_expiry_days <= 3650))),
    CONSTRAINT session_settings_id_check CHECK ((id = true))
);


ALTER TABLE public.session_settings OWNER TO postgres;

--
-- Name: TABLE session_settings; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.session_settings IS 'Singleton deployment-wide JWT session config (access-token TTL + max session length).';


--
-- Name: skills; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.skills (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name text NOT NULL,
    version text,
    display_name text,
    description text,
    when_to_use text,
    extracted_path text NOT NULL,
    bundle_sha256 text NOT NULL,
    bundle_size_bytes bigint NOT NULL,
    file_count integer NOT NULL,
    entry_point text NOT NULL,
    frontmatter_json jsonb DEFAULT '{}'::jsonb NOT NULL,
    tags jsonb DEFAULT '[]'::jsonb NOT NULL,
    scope character varying(10) DEFAULT 'user'::character varying NOT NULL,
    owner_user_id uuid,
    created_by uuid,
    enabled boolean DEFAULT true NOT NULL,
    is_dev boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT skills_scope_check CHECK (((scope)::text = ANY ((ARRAY['user'::character varying, 'system'::character varying, 'built_in'::character varying])::text[]))),
    CONSTRAINT skills_scope_owner_check CHECK (((((scope)::text = 'user'::text) AND (owner_user_id IS NOT NULL)) OR (((scope)::text = ANY ((ARRAY['system'::character varying, 'built_in'::character varying])::text[])) AND (owner_user_id IS NULL))))
);


ALTER TABLE public.skills OWNER TO postgres;

--
-- Name: summarization_admin_settings; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.summarization_admin_settings (
    id smallint DEFAULT 1 NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    default_summarization_model_id uuid,
    summarize_after_tokens integer DEFAULT 12000 NOT NULL,
    summarizer_keep_recent_tokens integer DEFAULT 3000 CONSTRAINT summarization_admin_setting_summarizer_keep_recent_tok_not_null NOT NULL,
    full_summary_prompt text,
    incremental_summary_prompt text,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT summarization_admin_settings_id_check CHECK ((id = 1)),
    CONSTRAINT summarization_admin_settings_summarize_after_tokens_check CHECK (((summarize_after_tokens >= 500) AND (summarize_after_tokens <= 1000000))),
    CONSTRAINT summarization_admin_settings_summarizer_keep_recent_token_check CHECK ((summarizer_keep_recent_tokens >= 100)),
    CONSTRAINT summarizer_keep_lt_trigger CHECK ((summarizer_keep_recent_tokens < summarize_after_tokens))
);


ALTER TABLE public.summarization_admin_settings OWNER TO postgres;

--
-- Name: tool_use_approvals; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.tool_use_approvals (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    conversation_id uuid NOT NULL,
    branch_id uuid NOT NULL,
    message_id uuid NOT NULL,
    user_id uuid NOT NULL,
    tool_use_id character varying(255) NOT NULL,
    tool_name character varying(255) NOT NULL,
    tool_input jsonb NOT NULL,
    server_id uuid,
    server_name character varying(255) NOT NULL,
    status character varying(50) DEFAULT 'pending'::character varying NOT NULL,
    approved_at timestamp with time zone,
    approved_by uuid,
    approval_note text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.tool_use_approvals OWNER TO postgres;

--
-- Name: user_auth_links; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.user_auth_links (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    provider_id uuid NOT NULL,
    external_id character varying(255) NOT NULL,
    external_email character varying(255),
    external_data jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    last_login_at timestamp with time zone
);


ALTER TABLE public.user_auth_links OWNER TO postgres;

--
-- Name: user_group_llm_providers; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.user_group_llm_providers (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    group_id uuid NOT NULL,
    provider_id uuid NOT NULL,
    assigned_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.user_group_llm_providers OWNER TO postgres;

--
-- Name: user_group_mcp_servers; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.user_group_mcp_servers (
    group_id uuid NOT NULL,
    mcp_server_id uuid NOT NULL,
    assigned_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.user_group_mcp_servers OWNER TO postgres;

--
-- Name: user_groups; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.user_groups (
    user_id uuid NOT NULL,
    group_id uuid NOT NULL,
    assigned_at timestamp with time zone DEFAULT now() NOT NULL,
    assigned_by uuid
);


ALTER TABLE public.user_groups OWNER TO postgres;

--
-- Name: user_lit_search_connector_keys; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.user_lit_search_connector_keys (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    connector text NOT NULL,
    api_key text,
    api_key_encrypted bytea,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.user_lit_search_connector_keys OWNER TO postgres;

--
-- Name: user_llm_provider_api_keys; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.user_llm_provider_api_keys (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    provider_id uuid NOT NULL,
    api_key text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    api_key_encrypted bytea
);


ALTER TABLE public.user_llm_provider_api_keys OWNER TO postgres;

--
-- Name: user_mcp_defaults; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.user_mcp_defaults (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    approval_mode character varying(50) DEFAULT 'manual_approve'::character varying NOT NULL,
    auto_approved_tools jsonb DEFAULT '[]'::jsonb NOT NULL,
    disabled_servers jsonb DEFAULT '[]'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    loop_settings jsonb
);


ALTER TABLE public.user_mcp_defaults OWNER TO postgres;

--
-- Name: user_memories; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.user_memories (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    content text NOT NULL,
    embedding public.halfvec(768),
    embedding_model text,
    source text NOT NULL,
    source_message_id uuid,
    importance smallint DEFAULT 50 NOT NULL,
    confidence smallint DEFAULT 80 NOT NULL,
    kind text DEFAULT 'fact'::text NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    last_recalled_at timestamp with time zone,
    recall_count integer DEFAULT 0 NOT NULL,
    deleted_at timestamp with time zone,
    scope text DEFAULT 'user'::text NOT NULL,
    project_id uuid,
    conversation_id uuid,
    content_tsv tsvector GENERATED ALWAYS AS (to_tsvector('simple'::regconfig, content)) STORED,
    CONSTRAINT user_memories_confidence_check CHECK (((confidence >= 0) AND (confidence <= 100))),
    CONSTRAINT user_memories_importance_check CHECK (((importance >= 0) AND (importance <= 100))),
    CONSTRAINT user_memories_kind_check CHECK ((kind = ANY (ARRAY['preference'::text, 'fact'::text, 'goal'::text, 'relationship'::text, 'other'::text]))),
    CONSTRAINT user_memories_scope_check CHECK ((scope = ANY (ARRAY['user'::text, 'project'::text, 'conversation'::text]))),
    CONSTRAINT user_memories_scope_ids_chk CHECK ((((scope = 'user'::text) AND (project_id IS NULL) AND (conversation_id IS NULL)) OR ((scope = 'project'::text) AND (project_id IS NOT NULL) AND (conversation_id IS NULL)) OR ((scope = 'conversation'::text) AND (project_id IS NULL) AND (conversation_id IS NOT NULL)))),
    CONSTRAINT user_memories_source_check CHECK ((source = ANY (ARRAY['extraction'::text, 'mcp_tool'::text, 'manual'::text])))
);


ALTER TABLE public.user_memories OWNER TO postgres;

--
-- Name: user_memory_settings; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.user_memory_settings (
    user_id uuid NOT NULL,
    extraction_enabled boolean DEFAULT false NOT NULL,
    retrieval_enabled boolean DEFAULT false NOT NULL,
    max_memories integer DEFAULT 1000 NOT NULL,
    retention_days integer,
    extraction_model_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT user_memory_settings_max_memories_check CHECK (((max_memories > 0) AND (max_memories <= 100000)))
);


ALTER TABLE public.user_memory_settings OWNER TO postgres;

--
-- Name: user_onboarding; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.user_onboarding (
    user_id uuid NOT NULL,
    completed_guide_ids text[] DEFAULT '{}'::text[] NOT NULL,
    completed_step_ids text[] DEFAULT '{}'::text[] NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.user_onboarding OWNER TO postgres;

--
-- Name: user_web_search_provider_keys; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.user_web_search_provider_keys (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    provider text NOT NULL,
    api_key text,
    api_key_encrypted bytea,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.user_web_search_provider_keys OWNER TO postgres;

--
-- Name: users; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.users (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    username character varying(100) NOT NULL,
    email character varying(255) NOT NULL,
    email_verified boolean DEFAULT false NOT NULL,
    password_hash character varying(255),
    display_name character varying(255),
    avatar_url text,
    is_active boolean DEFAULT true NOT NULL,
    is_admin boolean DEFAULT false NOT NULL,
    permissions text[] DEFAULT '{}'::text[] NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    last_login_at timestamp with time zone,
    password_changed_at timestamp with time zone
);


ALTER TABLE public.users OWNER TO postgres;

--
-- Name: COLUMN users.password_changed_at; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON COLUMN public.users.password_changed_at IS 'Timestamp of the most recent password change. NULL means the user is still using their bootstrap-issued password. Remote Access password-auth toggle requires this to be non-NULL for the admin user.';


--
-- Name: voice_models; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.voice_models (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(50) NOT NULL,
    filename character varying(200) NOT NULL,
    source character varying(20) DEFAULT 'catalog'::character varying NOT NULL,
    source_url text,
    size_bytes bigint DEFAULT 0 NOT NULL,
    sha256 character(64),
    verified boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT voice_models_source_check CHECK (((source)::text = ANY ((ARRAY['catalog'::character varying, 'url'::character varying, 'upload'::character varying])::text[])))
);


ALTER TABLE public.voice_models OWNER TO postgres;

--
-- Name: voice_runtime_instance; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.voice_runtime_instance (
    id boolean DEFAULT true NOT NULL,
    runtime_version_id uuid,
    active_model character varying(100),
    local_port integer,
    base_url text,
    status character varying(20) DEFAULT 'stopped'::character varying NOT NULL,
    state character varying(30) DEFAULT 'stopped'::character varying NOT NULL,
    state_changed_at timestamp with time zone DEFAULT now() NOT NULL,
    restart_attempts integer DEFAULT 0 NOT NULL,
    last_failure_reason text,
    last_used_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT voice_runtime_instance_id_check CHECK ((id = true)),
    CONSTRAINT voice_runtime_instance_state_check CHECK (((state)::text = ANY ((ARRAY['starting'::character varying, 'healthy'::character varying, 'unhealthy'::character varying, 'crashed'::character varying, 'restarting'::character varying, 'failed'::character varying, 'stopped'::character varying])::text[]))),
    CONSTRAINT voice_runtime_instance_status_check CHECK (((status)::text = ANY ((ARRAY['stopped'::character varying, 'running'::character varying])::text[])))
);


ALTER TABLE public.voice_runtime_instance OWNER TO postgres;

--
-- Name: voice_runtime_settings; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.voice_runtime_settings (
    id boolean DEFAULT true NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    model character varying(50) DEFAULT 'base'::character varying NOT NULL,
    language character varying(20) DEFAULT 'auto'::character varying NOT NULL,
    idle_unload_secs integer DEFAULT 1800 NOT NULL,
    auto_start_timeout_secs integer DEFAULT 60 NOT NULL,
    drain_timeout_secs integer DEFAULT 30 NOT NULL,
    max_clip_seconds integer DEFAULT 120 NOT NULL,
    max_upload_bytes bigint DEFAULT 33554432 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    streaming_enabled boolean DEFAULT true NOT NULL,
    stream_interval_ms integer DEFAULT 1000 NOT NULL,
    stream_max_decode_secs integer DEFAULT 30 NOT NULL,
    model_source_repo character varying(200) DEFAULT 'ggerganov/whisper.cpp'::character varying NOT NULL,
    CONSTRAINT voice_runtime_settings_auto_start_timeout_secs_check CHECK (((auto_start_timeout_secs >= 1) AND (auto_start_timeout_secs <= 600))),
    CONSTRAINT voice_runtime_settings_drain_timeout_secs_check CHECK (((drain_timeout_secs >= 1) AND (drain_timeout_secs <= 600))),
    CONSTRAINT voice_runtime_settings_id_check CHECK ((id = true)),
    CONSTRAINT voice_runtime_settings_idle_unload_secs_check CHECK (((idle_unload_secs >= 0) AND (idle_unload_secs <= 86400))),
    CONSTRAINT voice_runtime_settings_max_clip_seconds_check CHECK (((max_clip_seconds >= 1) AND (max_clip_seconds <= 3600))),
    CONSTRAINT voice_runtime_settings_max_upload_bytes_check CHECK (((max_upload_bytes >= 1024) AND (max_upload_bytes <= 67108864))),
    CONSTRAINT voice_runtime_settings_stream_interval_ms_check CHECK (((stream_interval_ms >= 300) AND (stream_interval_ms <= 10000))),
    CONSTRAINT voice_runtime_settings_stream_max_decode_secs_check CHECK (((stream_max_decode_secs >= 5) AND (stream_max_decode_secs <= 600)))
);


ALTER TABLE public.voice_runtime_settings OWNER TO postgres;

--
-- Name: voice_runtime_versions; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.voice_runtime_versions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    version character varying(100) NOT NULL,
    platform character varying(50) NOT NULL,
    arch character varying(50) NOT NULL,
    backend character varying(50) NOT NULL,
    binary_path text NOT NULL,
    is_system_default boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.voice_runtime_versions OWNER TO postgres;

--
-- Name: web_search_providers; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.web_search_providers (
    provider text NOT NULL,
    api_key text,
    api_key_encrypted bytea,
    config jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT provider_nonempty CHECK ((provider <> ''::text))
);


ALTER TABLE public.web_search_providers OWNER TO postgres;

--
-- Name: TABLE web_search_providers; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.web_search_providers IS 'Per-engine {api_key, config} keyed by the Rust SearchProvider registry name. New engines = code-only, no migration.';


--
-- Name: web_search_settings; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.web_search_settings (
    id boolean DEFAULT true NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    provider_chain text[] DEFAULT ARRAY['searxng'::text, 'brave'::text] NOT NULL,
    max_results integer DEFAULT 5 NOT NULL,
    fetch_max_bytes bigint DEFAULT 5242880 NOT NULL,
    fetch_max_chars integer DEFAULT 40000 NOT NULL,
    request_timeout_secs integer DEFAULT 20 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT fetch_max_bytes_range CHECK (((fetch_max_bytes >= 65536) AND (fetch_max_bytes <= 104857600))),
    CONSTRAINT fetch_max_chars_range CHECK (((fetch_max_chars >= 1000) AND (fetch_max_chars <= 500000))),
    CONSTRAINT max_results_range CHECK (((max_results >= 1) AND (max_results <= 20))),
    CONSTRAINT request_timeout_secs_range CHECK (((request_timeout_secs >= 1) AND (request_timeout_secs <= 120))),
    CONSTRAINT web_search_settings_id_check CHECK ((id = true))
);


ALTER TABLE public.web_search_settings OWNER TO postgres;

--
-- Name: TABLE web_search_settings; Type: COMMENT; Schema: public; Owner: postgres
--

COMMENT ON TABLE public.web_search_settings IS 'Singleton deployment-wide web_search config (enable + active provider + caps).';


--
-- Name: workflow_runs; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.workflow_runs (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    workflow_id uuid NOT NULL,
    conversation_id uuid,
    user_id uuid NOT NULL,
    model_id uuid,
    sandbox_flavor text,
    run_kind character varying(10) DEFAULT 'normal'::character varying NOT NULL,
    inputs_json jsonb DEFAULT '{}'::jsonb NOT NULL,
    step_outputs_json jsonb DEFAULT '{}'::jsonb NOT NULL,
    step_item_progress_json jsonb DEFAULT '{}'::jsonb NOT NULL,
    step_logs_json jsonb DEFAULT '{}'::jsonb NOT NULL,
    step_artifacts_json jsonb DEFAULT '{}'::jsonb NOT NULL,
    pending_elicitation_json jsonb,
    final_output_json jsonb,
    status character varying(50) DEFAULT 'pending'::character varying NOT NULL,
    current_step text,
    error_message text,
    total_tokens bigint DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    invocation_source character varying(20) DEFAULT 'manual'::character varying NOT NULL,
    step_progress_json jsonb,
    elicit_response_json jsonb,
    CONSTRAINT workflow_runs_invocation_source_check CHECK (((invocation_source)::text = ANY ((ARRAY['manual'::character varying, 'conversation'::character varying, 'agent'::character varying, 'mcp_tool'::character varying, 'scheduled'::character varying])::text[]))),
    CONSTRAINT workflow_runs_run_kind_check CHECK (((run_kind)::text = ANY ((ARRAY['normal'::character varying, 'test'::character varying, 'dry_run'::character varying])::text[]))),
    CONSTRAINT workflow_runs_status_check CHECK (((status)::text = ANY ((ARRAY['pending'::character varying, 'running'::character varying, 'waiting'::character varying, 'completed'::character varying, 'failed'::character varying, 'cancelled'::character varying])::text[])))
);


ALTER TABLE public.workflow_runs OWNER TO postgres;

--
-- Name: workflows; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.workflows (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name text NOT NULL,
    version text,
    display_name text,
    description text,
    extracted_path text NOT NULL,
    bundle_sha256 text NOT NULL,
    bundle_size_bytes bigint NOT NULL,
    file_count integer NOT NULL,
    entry_point text NOT NULL,
    tags jsonb DEFAULT '[]'::jsonb NOT NULL,
    scope character varying(10) DEFAULT 'user'::character varying NOT NULL,
    owner_user_id uuid,
    created_by uuid,
    enabled boolean DEFAULT true NOT NULL,
    is_dev boolean DEFAULT false NOT NULL,
    compiled_ir_json jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    ephemeral boolean DEFAULT false NOT NULL,
    conversation_id uuid,
    CONSTRAINT workflows_scope_check CHECK (((scope)::text = ANY ((ARRAY['user'::character varying, 'system'::character varying])::text[]))),
    CONSTRAINT workflows_scope_owner_check CHECK (((((scope)::text = 'user'::text) AND (owner_user_id IS NOT NULL)) OR (((scope)::text = 'system'::text) AND (owner_user_id IS NULL))))
);


ALTER TABLE public.workflows OWNER TO postgres;

--
-- Name: lit_fulltext_cache id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.lit_fulltext_cache ALTER COLUMN id SET DEFAULT nextval('public.lit_fulltext_cache_id_seq'::regclass);


--
-- Name: memory_audit_log id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.memory_audit_log ALTER COLUMN id SET DEFAULT nextval('public.memory_audit_log_id_seq'::regclass);


--
-- Name: _sqlx_migrations _sqlx_migrations_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public._sqlx_migrations
    ADD CONSTRAINT _sqlx_migrations_pkey PRIMARY KEY (version);


--
-- Name: assistant_core_memory assistant_core_memory_assistant_id_user_id_block_label_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.assistant_core_memory
    ADD CONSTRAINT assistant_core_memory_assistant_id_user_id_block_label_key UNIQUE (assistant_id, user_id, block_label);


--
-- Name: assistant_core_memory assistant_core_memory_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.assistant_core_memory
    ADD CONSTRAINT assistant_core_memory_pkey PRIMARY KEY (id);


--
-- Name: assistants assistants_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.assistants
    ADD CONSTRAINT assistants_pkey PRIMARY KEY (id);


--
-- Name: auth_providers auth_providers_name_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.auth_providers
    ADD CONSTRAINT auth_providers_name_key UNIQUE (name);


--
-- Name: auth_providers auth_providers_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.auth_providers
    ADD CONSTRAINT auth_providers_pkey PRIMARY KEY (id);


--
-- Name: bibliography_entries bibliography_entries_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.bibliography_entries
    ADD CONSTRAINT bibliography_entries_pkey PRIMARY KEY (id);


--
-- Name: branch_messages branch_messages_branch_id_message_id_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.branch_messages
    ADD CONSTRAINT branch_messages_branch_id_message_id_key UNIQUE (branch_id, message_id);


--
-- Name: branch_messages branch_messages_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.branch_messages
    ADD CONSTRAINT branch_messages_pkey PRIMARY KEY (id);


--
-- Name: branches branches_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.branches
    ADD CONSTRAINT branches_pkey PRIMARY KEY (id);


--
-- Name: code_sandbox_rootfs_artifacts code_sandbox_rootfs_artifacts_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.code_sandbox_rootfs_artifacts
    ADD CONSTRAINT code_sandbox_rootfs_artifacts_pkey PRIMARY KEY (id);


--
-- Name: code_sandbox_rootfs_artifacts code_sandbox_rootfs_artifacts_version_arch_flavor_package_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.code_sandbox_rootfs_artifacts
    ADD CONSTRAINT code_sandbox_rootfs_artifacts_version_arch_flavor_package_key UNIQUE (version, arch, flavor, package);


--
-- Name: code_sandbox_settings code_sandbox_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.code_sandbox_settings
    ADD CONSTRAINT code_sandbox_settings_pkey PRIMARY KEY (id);


--
-- Name: conversation_deliverables conversation_deliverables_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversation_deliverables
    ADD CONSTRAINT conversation_deliverables_pkey PRIMARY KEY (conversation_id, file_id);


--
-- Name: conversation_knowledge_bases conversation_knowledge_bases_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversation_knowledge_bases
    ADD CONSTRAINT conversation_knowledge_bases_pkey PRIMARY KEY (conversation_id, knowledge_base_id);


--
-- Name: conversation_memory_settings conversation_memory_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversation_memory_settings
    ADD CONSTRAINT conversation_memory_settings_pkey PRIMARY KEY (conversation_id);


--
-- Name: conversation_skill_overrides conversation_skill_overrides_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversation_skill_overrides
    ADD CONSTRAINT conversation_skill_overrides_pkey PRIMARY KEY (conversation_id, skill_id);


--
-- Name: conversation_summaries conversation_summaries_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversation_summaries
    ADD CONSTRAINT conversation_summaries_pkey PRIMARY KEY (branch_id);


--
-- Name: conversation_summarization_settings conversation_summarization_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversation_summarization_settings
    ADD CONSTRAINT conversation_summarization_settings_pkey PRIMARY KEY (conversation_id);


--
-- Name: conversations conversations_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversations
    ADD CONSTRAINT conversations_pkey PRIMARY KEY (id);


--
-- Name: desktop_settings desktop_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.desktop_settings
    ADD CONSTRAINT desktop_settings_pkey PRIMARY KEY (key);


--
-- Name: download_instances download_instances_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.download_instances
    ADD CONSTRAINT download_instances_pkey PRIMARY KEY (id);


--
-- Name: file_chunks file_chunks_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.file_chunks
    ADD CONSTRAINT file_chunks_pkey PRIMARY KEY (id);


--
-- Name: file_index_state file_index_state_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.file_index_state
    ADD CONSTRAINT file_index_state_pkey PRIMARY KEY (file_id);


--
-- Name: file_rag_admin_settings file_rag_admin_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.file_rag_admin_settings
    ADD CONSTRAINT file_rag_admin_settings_pkey PRIMARY KEY (id);


--
-- Name: file_versions file_versions_file_id_version_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.file_versions
    ADD CONSTRAINT file_versions_file_id_version_key UNIQUE (file_id, version);


--
-- Name: file_versions file_versions_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.file_versions
    ADD CONSTRAINT file_versions_pkey PRIMARY KEY (id);


--
-- Name: files files_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.files
    ADD CONSTRAINT files_pkey PRIMARY KEY (id);


--
-- Name: group_skills group_skills_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.group_skills
    ADD CONSTRAINT group_skills_pkey PRIMARY KEY (group_id, skill_id);


--
-- Name: group_workflows group_workflows_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.group_workflows
    ADD CONSTRAINT group_workflows_pkey PRIMARY KEY (group_id, workflow_id);


--
-- Name: groups groups_name_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.groups
    ADD CONSTRAINT groups_name_key UNIQUE (name);


--
-- Name: groups groups_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.groups
    ADD CONSTRAINT groups_pkey PRIMARY KEY (id);


--
-- Name: host_mount_policy host_mount_policy_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.host_mount_policy
    ADD CONSTRAINT host_mount_policy_pkey PRIMARY KEY (id);


--
-- Name: host_mounts host_mounts_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.host_mounts
    ADD CONSTRAINT host_mounts_pkey PRIMARY KEY (id);


--
-- Name: hub_entities hub_entities_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.hub_entities
    ADD CONSTRAINT hub_entities_pkey PRIMARY KEY (id);


--
-- Name: hub_settings hub_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.hub_settings
    ADD CONSTRAINT hub_settings_pkey PRIMARY KEY (id);


--
-- Name: js_tool_settings js_tool_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.js_tool_settings
    ADD CONSTRAINT js_tool_settings_pkey PRIMARY KEY (id);


--
-- Name: knowledge_base_documents knowledge_base_documents_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.knowledge_base_documents
    ADD CONSTRAINT knowledge_base_documents_pkey PRIMARY KEY (knowledge_base_id, file_id);


--
-- Name: knowledge_bases knowledge_bases_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.knowledge_bases
    ADD CONSTRAINT knowledge_bases_pkey PRIMARY KEY (id);


--
-- Name: lit_fulltext_cache lit_fulltext_cache_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.lit_fulltext_cache
    ADD CONSTRAINT lit_fulltext_cache_pkey PRIMARY KEY (id);


--
-- Name: lit_search_connectors lit_search_connectors_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.lit_search_connectors
    ADD CONSTRAINT lit_search_connectors_pkey PRIMARY KEY (connector);


--
-- Name: lit_search_settings lit_search_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.lit_search_settings
    ADD CONSTRAINT lit_search_settings_pkey PRIMARY KEY (id);


--
-- Name: llm_model_files llm_model_files_model_id_filename_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_model_files
    ADD CONSTRAINT llm_model_files_model_id_filename_key UNIQUE (model_id, filename);


--
-- Name: llm_model_files llm_model_files_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_model_files
    ADD CONSTRAINT llm_model_files_pkey PRIMARY KEY (id);


--
-- Name: llm_models llm_models_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_models
    ADD CONSTRAINT llm_models_pkey PRIMARY KEY (id);


--
-- Name: llm_models llm_models_provider_id_name_unique; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_models
    ADD CONSTRAINT llm_models_provider_id_name_unique UNIQUE (provider_id, name);


--
-- Name: llm_provider_files llm_provider_files_file_id_provider_id_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_provider_files
    ADD CONSTRAINT llm_provider_files_file_id_provider_id_key UNIQUE (file_id, provider_id);


--
-- Name: llm_provider_files llm_provider_files_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_provider_files
    ADD CONSTRAINT llm_provider_files_pkey PRIMARY KEY (id);


--
-- Name: llm_providers llm_providers_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_providers
    ADD CONSTRAINT llm_providers_pkey PRIMARY KEY (id);


--
-- Name: llm_repositories llm_repositories_name_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_repositories
    ADD CONSTRAINT llm_repositories_name_key UNIQUE (name);


--
-- Name: llm_repositories llm_repositories_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_repositories
    ADD CONSTRAINT llm_repositories_pkey PRIMARY KEY (id);


--
-- Name: llm_repositories llm_repositories_url_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_repositories
    ADD CONSTRAINT llm_repositories_url_key UNIQUE (url);


--
-- Name: llm_runtime_instances llm_runtime_instances_model_id_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_runtime_instances
    ADD CONSTRAINT llm_runtime_instances_model_id_key UNIQUE (model_id);


--
-- Name: llm_runtime_instances llm_runtime_instances_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_runtime_instances
    ADD CONSTRAINT llm_runtime_instances_pkey PRIMARY KEY (id);


--
-- Name: llm_runtime_settings llm_runtime_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_runtime_settings
    ADD CONSTRAINT llm_runtime_settings_pkey PRIMARY KEY (id);


--
-- Name: llm_runtime_versions llm_runtime_versions_engine_version_platform_arch_backend_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_runtime_versions
    ADD CONSTRAINT llm_runtime_versions_engine_version_platform_arch_backend_key UNIQUE (engine, version, platform, arch, backend);


--
-- Name: llm_runtime_versions llm_runtime_versions_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_runtime_versions
    ADD CONSTRAINT llm_runtime_versions_pkey PRIMARY KEY (id);


--
-- Name: magic_link_tokens magic_link_tokens_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.magic_link_tokens
    ADD CONSTRAINT magic_link_tokens_pkey PRIMARY KEY (token_hash);


--
-- Name: mcp_server_oauth_configs mcp_server_oauth_configs_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.mcp_server_oauth_configs
    ADD CONSTRAINT mcp_server_oauth_configs_pkey PRIMARY KEY (server_id);


--
-- Name: mcp_servers mcp_servers_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.mcp_servers
    ADD CONSTRAINT mcp_servers_pkey PRIMARY KEY (id);


--
-- Name: mcp_settings mcp_settings_one_per_conversation; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.mcp_settings
    ADD CONSTRAINT mcp_settings_one_per_conversation UNIQUE (conversation_id);


--
-- Name: mcp_settings mcp_settings_one_per_project; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.mcp_settings
    ADD CONSTRAINT mcp_settings_one_per_project UNIQUE (project_id);


--
-- Name: mcp_settings mcp_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.mcp_settings
    ADD CONSTRAINT mcp_settings_pkey PRIMARY KEY (id);


--
-- Name: mcp_tool_calls mcp_tool_calls_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.mcp_tool_calls
    ADD CONSTRAINT mcp_tool_calls_pkey PRIMARY KEY (id);


--
-- Name: mcp_user_policy mcp_user_policy_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.mcp_user_policy
    ADD CONSTRAINT mcp_user_policy_pkey PRIMARY KEY (id);


--
-- Name: memory_admin_settings memory_admin_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.memory_admin_settings
    ADD CONSTRAINT memory_admin_settings_pkey PRIMARY KEY (id);


--
-- Name: memory_audit_log memory_audit_log_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.memory_audit_log
    ADD CONSTRAINT memory_audit_log_pkey PRIMARY KEY (id);


--
-- Name: message_assistant message_assistant_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.message_assistant
    ADD CONSTRAINT message_assistant_pkey PRIMARY KEY (message_id);


--
-- Name: message_contents message_contents_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.message_contents
    ADD CONSTRAINT message_contents_pkey PRIMARY KEY (id);


--
-- Name: message_mcp_servers message_mcp_servers_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.message_mcp_servers
    ADD CONSTRAINT message_mcp_servers_pkey PRIMARY KEY (message_id, server_id);


--
-- Name: messages messages_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.messages
    ADD CONSTRAINT messages_pkey PRIMARY KEY (id);


--
-- Name: notifications notifications_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.notifications
    ADD CONSTRAINT notifications_pkey PRIMARY KEY (id);


--
-- Name: oauth_sessions oauth_sessions_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.oauth_sessions
    ADD CONSTRAINT oauth_sessions_pkey PRIMARY KEY (id);


--
-- Name: oauth_sessions oauth_sessions_state_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.oauth_sessions
    ADD CONSTRAINT oauth_sessions_state_key UNIQUE (state);


--
-- Name: pending_account_links pending_account_links_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.pending_account_links
    ADD CONSTRAINT pending_account_links_pkey PRIMARY KEY (link_token);


--
-- Name: project_bibliography project_bibliography_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.project_bibliography
    ADD CONSTRAINT project_bibliography_pkey PRIMARY KEY (project_id, entry_id);


--
-- Name: project_conversations project_conversations_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.project_conversations
    ADD CONSTRAINT project_conversations_pkey PRIMARY KEY (conversation_id);


--
-- Name: project_files project_files_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.project_files
    ADD CONSTRAINT project_files_pkey PRIMARY KEY (project_id, file_id);


--
-- Name: project_knowledge_bases project_knowledge_bases_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.project_knowledge_bases
    ADD CONSTRAINT project_knowledge_bases_pkey PRIMARY KEY (project_id, knowledge_base_id);


--
-- Name: projects projects_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.projects
    ADD CONSTRAINT projects_pkey PRIMARY KEY (id);


--
-- Name: projects projects_user_name_unique; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.projects
    ADD CONSTRAINT projects_user_name_unique UNIQUE (user_id, name);


--
-- Name: refresh_tokens refresh_tokens_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.refresh_tokens
    ADD CONSTRAINT refresh_tokens_pkey PRIMARY KEY (jti);


--
-- Name: remote_access_settings remote_access_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.remote_access_settings
    ADD CONSTRAINT remote_access_settings_pkey PRIMARY KEY (id);


--
-- Name: sandbox_workspace_files sandbox_workspace_files_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.sandbox_workspace_files
    ADD CONSTRAINT sandbox_workspace_files_pkey PRIMARY KEY (conversation_id, workspace_relpath);


--
-- Name: scheduled_task_runs scheduled_task_runs_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.scheduled_task_runs
    ADD CONSTRAINT scheduled_task_runs_pkey PRIMARY KEY (id);


--
-- Name: scheduled_tasks scheduled_tasks_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.scheduled_tasks
    ADD CONSTRAINT scheduled_tasks_pkey PRIMARY KEY (id);


--
-- Name: scheduler_admin_settings scheduler_admin_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.scheduler_admin_settings
    ADD CONSTRAINT scheduler_admin_settings_pkey PRIMARY KEY (id);


--
-- Name: session_settings session_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.session_settings
    ADD CONSTRAINT session_settings_pkey PRIMARY KEY (id);


--
-- Name: skills skills_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.skills
    ADD CONSTRAINT skills_pkey PRIMARY KEY (id);


--
-- Name: summarization_admin_settings summarization_admin_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.summarization_admin_settings
    ADD CONSTRAINT summarization_admin_settings_pkey PRIMARY KEY (id);


--
-- Name: tool_use_approvals tool_use_approvals_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.tool_use_approvals
    ADD CONSTRAINT tool_use_approvals_pkey PRIMARY KEY (id);


--
-- Name: hub_entities unique_entity_hub_tracking; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.hub_entities
    ADD CONSTRAINT unique_entity_hub_tracking UNIQUE (entity_type, entity_id);


--
-- Name: tool_use_approvals unique_tool_use; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.tool_use_approvals
    ADD CONSTRAINT unique_tool_use UNIQUE (message_id, tool_use_id);


--
-- Name: user_mcp_defaults unique_user_mcp_defaults; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_mcp_defaults
    ADD CONSTRAINT unique_user_mcp_defaults UNIQUE (user_id);


--
-- Name: message_contents uq_message_contents_message_sequence; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.message_contents
    ADD CONSTRAINT uq_message_contents_message_sequence UNIQUE (message_id, sequence_order);


--
-- Name: user_auth_links user_auth_links_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_auth_links
    ADD CONSTRAINT user_auth_links_pkey PRIMARY KEY (id);


--
-- Name: user_auth_links user_auth_links_provider_id_external_id_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_auth_links
    ADD CONSTRAINT user_auth_links_provider_id_external_id_key UNIQUE (provider_id, external_id);


--
-- Name: user_group_llm_providers user_group_llm_providers_group_id_provider_id_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_group_llm_providers
    ADD CONSTRAINT user_group_llm_providers_group_id_provider_id_key UNIQUE (group_id, provider_id);


--
-- Name: user_group_llm_providers user_group_llm_providers_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_group_llm_providers
    ADD CONSTRAINT user_group_llm_providers_pkey PRIMARY KEY (id);


--
-- Name: user_group_mcp_servers user_group_mcp_servers_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_group_mcp_servers
    ADD CONSTRAINT user_group_mcp_servers_pkey PRIMARY KEY (group_id, mcp_server_id);


--
-- Name: user_groups user_groups_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_groups
    ADD CONSTRAINT user_groups_pkey PRIMARY KEY (user_id, group_id);


--
-- Name: user_lit_search_connector_keys user_lit_search_connector_keys_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_lit_search_connector_keys
    ADD CONSTRAINT user_lit_search_connector_keys_pkey PRIMARY KEY (id);


--
-- Name: user_lit_search_connector_keys user_lit_search_connector_keys_user_id_connector_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_lit_search_connector_keys
    ADD CONSTRAINT user_lit_search_connector_keys_user_id_connector_key UNIQUE (user_id, connector);


--
-- Name: user_llm_provider_api_keys user_llm_provider_api_keys_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_llm_provider_api_keys
    ADD CONSTRAINT user_llm_provider_api_keys_pkey PRIMARY KEY (id);


--
-- Name: user_llm_provider_api_keys user_llm_provider_api_keys_user_id_provider_id_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_llm_provider_api_keys
    ADD CONSTRAINT user_llm_provider_api_keys_user_id_provider_id_key UNIQUE (user_id, provider_id);


--
-- Name: user_mcp_defaults user_mcp_defaults_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_mcp_defaults
    ADD CONSTRAINT user_mcp_defaults_pkey PRIMARY KEY (id);


--
-- Name: user_memories user_memories_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_memories
    ADD CONSTRAINT user_memories_pkey PRIMARY KEY (id);


--
-- Name: user_memory_settings user_memory_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_memory_settings
    ADD CONSTRAINT user_memory_settings_pkey PRIMARY KEY (user_id);


--
-- Name: user_onboarding user_onboarding_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_onboarding
    ADD CONSTRAINT user_onboarding_pkey PRIMARY KEY (user_id);


--
-- Name: user_web_search_provider_keys user_web_search_provider_keys_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_web_search_provider_keys
    ADD CONSTRAINT user_web_search_provider_keys_pkey PRIMARY KEY (id);


--
-- Name: user_web_search_provider_keys user_web_search_provider_keys_user_id_provider_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_web_search_provider_keys
    ADD CONSTRAINT user_web_search_provider_keys_user_id_provider_key UNIQUE (user_id, provider);


--
-- Name: users users_email_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_email_key UNIQUE (email);


--
-- Name: users users_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);


--
-- Name: users users_username_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_username_key UNIQUE (username);


--
-- Name: voice_models voice_models_filename_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.voice_models
    ADD CONSTRAINT voice_models_filename_key UNIQUE (filename);


--
-- Name: voice_models voice_models_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.voice_models
    ADD CONSTRAINT voice_models_pkey PRIMARY KEY (id);


--
-- Name: voice_runtime_instance voice_runtime_instance_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.voice_runtime_instance
    ADD CONSTRAINT voice_runtime_instance_pkey PRIMARY KEY (id);


--
-- Name: voice_runtime_settings voice_runtime_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.voice_runtime_settings
    ADD CONSTRAINT voice_runtime_settings_pkey PRIMARY KEY (id);


--
-- Name: voice_runtime_versions voice_runtime_versions_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.voice_runtime_versions
    ADD CONSTRAINT voice_runtime_versions_pkey PRIMARY KEY (id);


--
-- Name: voice_runtime_versions voice_runtime_versions_version_platform_arch_backend_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.voice_runtime_versions
    ADD CONSTRAINT voice_runtime_versions_version_platform_arch_backend_key UNIQUE (version, platform, arch, backend);


--
-- Name: web_search_providers web_search_providers_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.web_search_providers
    ADD CONSTRAINT web_search_providers_pkey PRIMARY KEY (provider);


--
-- Name: web_search_settings web_search_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.web_search_settings
    ADD CONSTRAINT web_search_settings_pkey PRIMARY KEY (id);


--
-- Name: workflow_runs workflow_runs_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.workflow_runs
    ADD CONSTRAINT workflow_runs_pkey PRIMARY KEY (id);


--
-- Name: workflows workflows_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.workflows
    ADD CONSTRAINT workflows_pkey PRIMARY KEY (id);


--
-- Name: host_mounts_conversation_uq; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX host_mounts_conversation_uq ON public.host_mounts USING btree (conversation_id) WHERE (conversation_id IS NOT NULL);


--
-- Name: host_mounts_project_uq; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX host_mounts_project_uq ON public.host_mounts USING btree (project_id) WHERE (project_id IS NOT NULL);


--
-- Name: idx_assistants_created_by; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_assistants_created_by ON public.assistants USING btree (created_by);


--
-- Name: idx_assistants_default_lookup; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_assistants_default_lookup ON public.assistants USING btree (created_by) WHERE ((is_default = true) AND (enabled = true));


--
-- Name: idx_assistants_enabled; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_assistants_enabled ON public.assistants USING btree (enabled);


--
-- Name: idx_assistants_is_default; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_assistants_is_default ON public.assistants USING btree (is_default);


--
-- Name: idx_assistants_is_template; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_assistants_is_template ON public.assistants USING btree (is_template);


--
-- Name: idx_assistants_name; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_assistants_name ON public.assistants USING btree (name);


--
-- Name: idx_auth_providers_enabled; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_auth_providers_enabled ON public.auth_providers USING btree (enabled);


--
-- Name: idx_bibliography_tsv; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_bibliography_tsv ON public.bibliography_entries USING gin (content_tsv);


--
-- Name: idx_bibliography_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_bibliography_user ON public.bibliography_entries USING btree (user_id);


--
-- Name: idx_branch_messages_branch_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_branch_messages_branch_id ON public.branch_messages USING btree (branch_id, created_at);


--
-- Name: idx_branch_messages_message_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_branch_messages_message_id ON public.branch_messages USING btree (message_id);


--
-- Name: idx_branches_conversation_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_branches_conversation_id ON public.branches USING btree (conversation_id);


--
-- Name: idx_branches_created_from_message_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_branches_created_from_message_id ON public.branches USING btree (created_from_message_id);


--
-- Name: idx_branches_parent_branch_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_branches_parent_branch_id ON public.branches USING btree (parent_branch_id);


--
-- Name: idx_code_sandbox_rootfs_artifacts_arch_flavor; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_code_sandbox_rootfs_artifacts_arch_flavor ON public.code_sandbox_rootfs_artifacts USING btree (arch, flavor);


--
-- Name: idx_code_sandbox_rootfs_artifacts_version; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_code_sandbox_rootfs_artifacts_version ON public.code_sandbox_rootfs_artifacts USING btree (version);


--
-- Name: idx_conversation_deliverables_file_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_conversation_deliverables_file_id ON public.conversation_deliverables USING btree (file_id);


--
-- Name: idx_conversation_knowledge_bases_kb; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_conversation_knowledge_bases_kb ON public.conversation_knowledge_bases USING btree (knowledge_base_id);


--
-- Name: idx_conversation_skill_overrides_conv; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_conversation_skill_overrides_conv ON public.conversation_skill_overrides USING btree (conversation_id);


--
-- Name: idx_conversations_created_at; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_conversations_created_at ON public.conversations USING btree (created_at DESC);


--
-- Name: idx_conversations_model_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_conversations_model_id ON public.conversations USING btree (model_id);


--
-- Name: idx_conversations_user_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_conversations_user_id ON public.conversations USING btree (user_id);


--
-- Name: idx_core_memory_lookup; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_core_memory_lookup ON public.assistant_core_memory USING btree (user_id, assistant_id);


--
-- Name: idx_desktop_settings_key; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_desktop_settings_key ON public.desktop_settings USING btree (key);


--
-- Name: idx_download_instances_created_at; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_download_instances_created_at ON public.download_instances USING btree (created_at DESC);


--
-- Name: idx_download_instances_provider_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_download_instances_provider_id ON public.download_instances USING btree (provider_id);


--
-- Name: idx_download_instances_repository_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_download_instances_repository_id ON public.download_instances USING btree (repository_id);


--
-- Name: idx_download_instances_status; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_download_instances_status ON public.download_instances USING btree (status);


--
-- Name: idx_file_chunks_embedding; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_file_chunks_embedding ON public.file_chunks USING hnsw (embedding public.halfvec_cosine_ops);


--
-- Name: idx_file_chunks_file; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_file_chunks_file ON public.file_chunks USING btree (file_id);


--
-- Name: idx_file_chunks_tsv; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_file_chunks_tsv ON public.file_chunks USING gin (content_tsv);


--
-- Name: idx_file_index_state_status; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_file_index_state_status ON public.file_index_state USING btree (status);


--
-- Name: idx_file_index_state_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_file_index_state_user ON public.file_index_state USING btree (user_id);


--
-- Name: idx_file_versions_blob; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_file_versions_blob ON public.file_versions USING btree (blob_version_id);


--
-- Name: idx_file_versions_file; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_file_versions_file ON public.file_versions USING btree (file_id, version DESC);


--
-- Name: idx_files_checksum; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_files_checksum ON public.files USING btree (checksum);


--
-- Name: idx_files_created_at; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_files_created_at ON public.files USING btree (created_at DESC);


--
-- Name: idx_files_file_size; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_files_file_size ON public.files USING btree (file_size);


--
-- Name: idx_files_mime_type; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_files_mime_type ON public.files USING btree (mime_type);


--
-- Name: idx_files_processing_metadata; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_files_processing_metadata ON public.files USING gin (processing_metadata);


--
-- Name: idx_files_user_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_files_user_id ON public.files USING btree (user_id);


--
-- Name: idx_files_workflow_run_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_files_workflow_run_id ON public.files USING btree (workflow_run_id) WHERE (workflow_run_id IS NOT NULL);


--
-- Name: idx_group_mcp_servers_group_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_group_mcp_servers_group_id ON public.user_group_mcp_servers USING btree (group_id);


--
-- Name: idx_group_mcp_servers_server_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_group_mcp_servers_server_id ON public.user_group_mcp_servers USING btree (mcp_server_id);


--
-- Name: idx_group_skills_skill; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_group_skills_skill ON public.group_skills USING btree (skill_id);


--
-- Name: idx_group_workflows_workflow; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_group_workflows_workflow ON public.group_workflows USING btree (workflow_id);


--
-- Name: idx_groups_name; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_groups_name ON public.groups USING btree (name);


--
-- Name: idx_groups_permissions; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_groups_permissions ON public.groups USING gin (permissions);


--
-- Name: idx_hub_entities_hub_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_hub_entities_hub_id ON public.hub_entities USING btree (hub_id, entity_type);


--
-- Name: idx_hub_entities_lookup; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_hub_entities_lookup ON public.hub_entities USING btree (entity_type, entity_id);


--
-- Name: idx_hub_entities_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_hub_entities_user ON public.hub_entities USING btree (created_by) WHERE (created_by IS NOT NULL);


--
-- Name: idx_knowledge_base_documents_file; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_knowledge_base_documents_file ON public.knowledge_base_documents USING btree (file_id);


--
-- Name: idx_knowledge_bases_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_knowledge_bases_user ON public.knowledge_bases USING btree (user_id);


--
-- Name: idx_knowledge_bases_user_name; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX idx_knowledge_bases_user_name ON public.knowledge_bases USING btree (user_id, lower(name));


--
-- Name: idx_llm_model_files_model_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_llm_model_files_model_id ON public.llm_model_files USING btree (model_id);


--
-- Name: idx_llm_model_files_upload_status; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_llm_model_files_upload_status ON public.llm_model_files USING btree (upload_status);


--
-- Name: idx_llm_models_created_at; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_llm_models_created_at ON public.llm_models USING btree (created_at DESC);


--
-- Name: idx_llm_models_enabled; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_llm_models_enabled ON public.llm_models USING btree (enabled);


--
-- Name: idx_llm_models_engine_type; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_llm_models_engine_type ON public.llm_models USING btree (engine_type);


--
-- Name: idx_llm_models_provider_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_llm_models_provider_id ON public.llm_models USING btree (provider_id);


--
-- Name: idx_llm_models_validation_status; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_llm_models_validation_status ON public.llm_models USING btree (validation_status);


--
-- Name: idx_llm_provider_files_expires_at; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_llm_provider_files_expires_at ON public.llm_provider_files USING btree (((provider_metadata ->> 'expires_at'::text))) WHERE ((provider_metadata ->> 'expires_at'::text) IS NOT NULL);


--
-- Name: idx_llm_provider_files_file_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_llm_provider_files_file_id ON public.llm_provider_files USING btree (file_id);


--
-- Name: idx_llm_provider_files_metadata; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_llm_provider_files_metadata ON public.llm_provider_files USING gin (provider_metadata);


--
-- Name: idx_llm_provider_files_provider_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_llm_provider_files_provider_id ON public.llm_provider_files USING btree (provider_id);


--
-- Name: idx_llm_provider_files_status; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_llm_provider_files_status ON public.llm_provider_files USING btree (upload_status);


--
-- Name: idx_llm_providers_enabled; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_llm_providers_enabled ON public.llm_providers USING btree (enabled);


--
-- Name: idx_llm_providers_type; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_llm_providers_type ON public.llm_providers USING btree (provider_type);


--
-- Name: idx_llm_repositories_health_status_unhealthy; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_llm_repositories_health_status_unhealthy ON public.llm_repositories USING btree (last_health_check_status) WHERE (last_health_check_status = 'unhealthy'::text);


--
-- Name: idx_llm_runtime_instances_last_used; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_llm_runtime_instances_last_used ON public.llm_runtime_instances USING btree (last_used_at) WHERE ((status)::text = 'running'::text);


--
-- Name: idx_llm_runtime_instances_state; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_llm_runtime_instances_state ON public.llm_runtime_instances USING btree (state);


--
-- Name: idx_magic_link_tokens_expires; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_magic_link_tokens_expires ON public.magic_link_tokens USING btree (expires_at);


--
-- Name: idx_magic_link_tokens_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_magic_link_tokens_user ON public.magic_link_tokens USING btree (user_id, created_at DESC);


--
-- Name: idx_mcp_servers_enabled; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_mcp_servers_enabled ON public.mcp_servers USING btree (enabled);


--
-- Name: idx_mcp_servers_health_status_unhealthy; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_mcp_servers_health_status_unhealthy ON public.mcp_servers USING btree (last_health_check_status) WHERE (last_health_check_status = 'unhealthy'::text);


--
-- Name: idx_mcp_servers_is_built_in; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_mcp_servers_is_built_in ON public.mcp_servers USING btree (is_built_in);


--
-- Name: idx_mcp_servers_is_system; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_mcp_servers_is_system ON public.mcp_servers USING btree (is_system);


--
-- Name: idx_mcp_servers_transport_type; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_mcp_servers_transport_type ON public.mcp_servers USING btree (transport_type);


--
-- Name: idx_mcp_servers_user_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_mcp_servers_user_id ON public.mcp_servers USING btree (user_id);


--
-- Name: idx_mcp_settings_user_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_mcp_settings_user_id ON public.mcp_settings USING btree (user_id);


--
-- Name: idx_mcp_tool_calls_conv; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_mcp_tool_calls_conv ON public.mcp_tool_calls USING btree (conversation_id) WHERE (conversation_id IS NOT NULL);


--
-- Name: idx_mcp_tool_calls_created; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_mcp_tool_calls_created ON public.mcp_tool_calls USING btree (created_at);


--
-- Name: idx_mcp_tool_calls_server; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_mcp_tool_calls_server ON public.mcp_tool_calls USING btree (server_id);


--
-- Name: idx_mcp_tool_calls_user_created; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_mcp_tool_calls_user_created ON public.mcp_tool_calls USING btree (user_id, created_at DESC);


--
-- Name: idx_mcp_tool_calls_workflow_run; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_mcp_tool_calls_workflow_run ON public.mcp_tool_calls USING btree (workflow_run_id) WHERE (workflow_run_id IS NOT NULL);


--
-- Name: idx_memory_audit_log_memory; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_memory_audit_log_memory ON public.memory_audit_log USING btree (memory_id) WHERE (memory_id IS NOT NULL);


--
-- Name: idx_memory_audit_log_user_created; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_memory_audit_log_user_created ON public.memory_audit_log USING btree (user_id, created_at DESC);


--
-- Name: idx_message_contents_content; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_message_contents_content ON public.message_contents USING gin (content);


--
-- Name: idx_message_contents_message_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_message_contents_message_id ON public.message_contents USING btree (message_id);


--
-- Name: idx_message_contents_message_seq_unique; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX idx_message_contents_message_seq_unique ON public.message_contents USING btree (message_id, sequence_order);


--
-- Name: idx_message_contents_type; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_message_contents_type ON public.message_contents USING btree (content_type);


--
-- Name: idx_messages_created_at; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_messages_created_at ON public.messages USING btree (created_at DESC);


--
-- Name: idx_messages_originated_from_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_messages_originated_from_id ON public.messages USING btree (originated_from_id);


--
-- Name: idx_messages_role; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_messages_role ON public.messages USING btree (role);


--
-- Name: idx_models_required_runtime_version; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_models_required_runtime_version ON public.llm_models USING btree (required_runtime_version_id) WHERE (required_runtime_version_id IS NOT NULL);


--
-- Name: idx_notifications_user_created; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_notifications_user_created ON public.notifications USING btree (user_id, created_at DESC);


--
-- Name: idx_notifications_user_unread; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_notifications_user_unread ON public.notifications USING btree (user_id) WHERE (read_at IS NULL);


--
-- Name: idx_oauth_sessions_expires_at; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_oauth_sessions_expires_at ON public.oauth_sessions USING btree (expires_at);


--
-- Name: idx_oauth_sessions_state; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_oauth_sessions_state ON public.oauth_sessions USING btree (state);


--
-- Name: idx_pending_links_expires_at; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_pending_links_expires_at ON public.pending_account_links USING btree (expires_at);


--
-- Name: idx_pending_links_target_user_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_pending_links_target_user_id ON public.pending_account_links USING btree (target_user_id);


--
-- Name: idx_project_bibliography_entry_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_project_bibliography_entry_id ON public.project_bibliography USING btree (entry_id);


--
-- Name: idx_project_conversations_project_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_project_conversations_project_id ON public.project_conversations USING btree (project_id);


--
-- Name: idx_project_files_file_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_project_files_file_id ON public.project_files USING btree (file_id);


--
-- Name: idx_project_knowledge_bases_kb; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_project_knowledge_bases_kb ON public.project_knowledge_bases USING btree (knowledge_base_id);


--
-- Name: idx_projects_updated_at; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_projects_updated_at ON public.projects USING btree (updated_at DESC);


--
-- Name: idx_projects_user_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_projects_user_id ON public.projects USING btree (user_id);


--
-- Name: idx_providers_default_runtime_version; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_providers_default_runtime_version ON public.llm_providers USING btree (default_runtime_version_id) WHERE (default_runtime_version_id IS NOT NULL);


--
-- Name: idx_refresh_tokens_expires_at; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_refresh_tokens_expires_at ON public.refresh_tokens USING btree (expires_at);


--
-- Name: idx_refresh_tokens_user_active; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_refresh_tokens_user_active ON public.refresh_tokens USING btree (user_id) WHERE (revoked_at IS NULL);


--
-- Name: idx_runtime_instances_provider; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_runtime_instances_provider ON public.llm_runtime_instances USING btree (provider_id);


--
-- Name: idx_runtime_instances_status; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_runtime_instances_status ON public.llm_runtime_instances USING btree (status);


--
-- Name: idx_runtime_instances_version; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_runtime_instances_version ON public.llm_runtime_instances USING btree (runtime_version_id);


--
-- Name: idx_runtime_versions_default; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_runtime_versions_default ON public.llm_runtime_versions USING btree (is_system_default) WHERE (is_system_default = true);


--
-- Name: idx_runtime_versions_engine; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_runtime_versions_engine ON public.llm_runtime_versions USING btree (engine);


--
-- Name: idx_sandbox_workspace_files_file; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_sandbox_workspace_files_file ON public.sandbox_workspace_files USING btree (file_id);


--
-- Name: idx_scheduled_task_runs_task; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_scheduled_task_runs_task ON public.scheduled_task_runs USING btree (scheduled_task_id, fired_at DESC);


--
-- Name: idx_scheduled_task_runs_user_fired; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_scheduled_task_runs_user_fired ON public.scheduled_task_runs USING btree (user_id, fired_at DESC);


--
-- Name: idx_scheduled_tasks_due; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_scheduled_tasks_due ON public.scheduled_tasks USING btree (next_run_at) WHERE (enabled AND (next_run_at IS NOT NULL));


--
-- Name: idx_scheduled_tasks_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_scheduled_tasks_user ON public.scheduled_tasks USING btree (user_id, created_at DESC);


--
-- Name: idx_skills_enabled; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_skills_enabled ON public.skills USING btree (enabled) WHERE (enabled = true);


--
-- Name: idx_skills_name; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_skills_name ON public.skills USING btree (name);


--
-- Name: idx_skills_owner; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_skills_owner ON public.skills USING btree (owner_user_id) WHERE ((scope)::text = 'user'::text);


--
-- Name: idx_tool_use_approvals_branch_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_tool_use_approvals_branch_id ON public.tool_use_approvals USING btree (branch_id);


--
-- Name: idx_tool_use_approvals_branch_status; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_tool_use_approvals_branch_status ON public.tool_use_approvals USING btree (branch_id, status);


--
-- Name: idx_tool_use_approvals_conversation_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_tool_use_approvals_conversation_id ON public.tool_use_approvals USING btree (conversation_id);


--
-- Name: idx_tool_use_approvals_message_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_tool_use_approvals_message_id ON public.tool_use_approvals USING btree (message_id);


--
-- Name: idx_tool_use_approvals_server_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_tool_use_approvals_server_id ON public.tool_use_approvals USING btree (server_id);


--
-- Name: idx_tool_use_approvals_status; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_tool_use_approvals_status ON public.tool_use_approvals USING btree (status);


--
-- Name: idx_tool_use_approvals_user_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_tool_use_approvals_user_id ON public.tool_use_approvals USING btree (user_id);


--
-- Name: idx_ugp_group; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_ugp_group ON public.user_group_llm_providers USING btree (group_id);


--
-- Name: idx_ugp_provider; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_ugp_provider ON public.user_group_llm_providers USING btree (provider_id);


--
-- Name: idx_user_auth_links_external_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_user_auth_links_external_id ON public.user_auth_links USING btree (provider_id, external_id);


--
-- Name: idx_user_auth_links_provider_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_user_auth_links_provider_id ON public.user_auth_links USING btree (provider_id);


--
-- Name: idx_user_auth_links_user_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_user_auth_links_user_id ON public.user_auth_links USING btree (user_id);


--
-- Name: idx_user_groups_group_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_user_groups_group_id ON public.user_groups USING btree (group_id);


--
-- Name: idx_user_groups_user_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_user_groups_user_id ON public.user_groups USING btree (user_id);


--
-- Name: idx_user_memories_embedding; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_user_memories_embedding ON public.user_memories USING hnsw (embedding public.halfvec_cosine_ops);


--
-- Name: idx_user_memories_extraction_quota; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_user_memories_extraction_quota ON public.user_memories USING btree (user_id, created_at) WHERE (source = 'extraction'::text);


--
-- Name: idx_user_memories_extraction_recent; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_user_memories_extraction_recent ON public.user_memories USING btree (user_id, created_at) WHERE (source = 'extraction'::text);


--
-- Name: idx_user_memories_metadata; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_user_memories_metadata ON public.user_memories USING gin (metadata);


--
-- Name: idx_user_memories_scope_conversation; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_user_memories_scope_conversation ON public.user_memories USING btree (user_id, conversation_id) WHERE ((scope = 'conversation'::text) AND (deleted_at IS NULL));


--
-- Name: idx_user_memories_scope_project; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_user_memories_scope_project ON public.user_memories USING btree (user_id, project_id) WHERE ((scope = 'project'::text) AND (deleted_at IS NULL));


--
-- Name: idx_user_memories_scope_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_user_memories_scope_user ON public.user_memories USING btree (user_id) WHERE ((scope = 'user'::text) AND (deleted_at IS NULL));


--
-- Name: idx_user_memories_tsv; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_user_memories_tsv ON public.user_memories USING gin (content_tsv);


--
-- Name: idx_user_memories_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_user_memories_user ON public.user_memories USING btree (user_id) WHERE (deleted_at IS NULL);


--
-- Name: idx_user_memories_user_updated; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_user_memories_user_updated ON public.user_memories USING btree (user_id, updated_at DESC) WHERE (deleted_at IS NULL);


--
-- Name: idx_users_created_at; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_users_created_at ON public.users USING btree (created_at);


--
-- Name: idx_users_email; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_users_email ON public.users USING btree (email);


--
-- Name: idx_users_is_active; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_users_is_active ON public.users USING btree (is_active);


--
-- Name: idx_users_last_login_at; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_users_last_login_at ON public.users USING btree (last_login_at);


--
-- Name: idx_users_lower_email; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_users_lower_email ON public.users USING btree (lower((email)::text));


--
-- Name: idx_users_permissions; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_users_permissions ON public.users USING gin (permissions);


--
-- Name: idx_users_username; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_users_username ON public.users USING btree (username);


--
-- Name: idx_workflow_runs_conv; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_workflow_runs_conv ON public.workflow_runs USING btree (conversation_id) WHERE (conversation_id IS NOT NULL);


--
-- Name: idx_workflow_runs_created_at; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_workflow_runs_created_at ON public.workflow_runs USING btree (created_at);


--
-- Name: idx_workflow_runs_history; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_workflow_runs_history ON public.workflow_runs USING btree (workflow_id, user_id, created_at DESC);


--
-- Name: idx_workflow_runs_run_kind; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_workflow_runs_run_kind ON public.workflow_runs USING btree (run_kind);


--
-- Name: idx_workflow_runs_status; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_workflow_runs_status ON public.workflow_runs USING btree (status);


--
-- Name: idx_workflow_runs_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_workflow_runs_user ON public.workflow_runs USING btree (user_id);


--
-- Name: idx_workflow_runs_user_created; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_workflow_runs_user_created ON public.workflow_runs USING btree (user_id, created_at DESC);


--
-- Name: idx_workflow_runs_workflow; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_workflow_runs_workflow ON public.workflow_runs USING btree (workflow_id);


--
-- Name: idx_workflows_conversation; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_workflows_conversation ON public.workflows USING btree (conversation_id) WHERE (ephemeral = true);


--
-- Name: idx_workflows_name; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_workflows_name ON public.workflows USING btree (name);


--
-- Name: idx_workflows_owner; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_workflows_owner ON public.workflows USING btree (owner_user_id) WHERE ((scope)::text = 'user'::text);


--
-- Name: lit_fulltext_cache_arxiv; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX lit_fulltext_cache_arxiv ON public.lit_fulltext_cache USING btree (arxiv_id) WHERE (arxiv_id IS NOT NULL);


--
-- Name: lit_fulltext_cache_doi; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX lit_fulltext_cache_doi ON public.lit_fulltext_cache USING btree (doi) WHERE (doi IS NOT NULL);


--
-- Name: lit_fulltext_cache_lru; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX lit_fulltext_cache_lru ON public.lit_fulltext_cache USING btree (last_accessed_at);


--
-- Name: lit_fulltext_cache_pmcid; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX lit_fulltext_cache_pmcid ON public.lit_fulltext_cache USING btree (pmcid) WHERE (pmcid IS NOT NULL);


--
-- Name: lit_fulltext_cache_pmid; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX lit_fulltext_cache_pmid ON public.lit_fulltext_cache USING btree (pmid) WHERE (pmid IS NOT NULL);


--
-- Name: uniq_hub_system_mcp_install; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX uniq_hub_system_mcp_install ON public.hub_entities USING btree (hub_id) WHERE (((entity_type)::text = 'mcp_server'::text) AND (created_by IS NULL));


--
-- Name: uniq_hub_template_install; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX uniq_hub_template_install ON public.hub_entities USING btree (hub_id) WHERE (((entity_type)::text = 'assistant'::text) AND (created_by IS NULL));


--
-- Name: uniq_skills_builtin_name; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX uniq_skills_builtin_name ON public.skills USING btree (name) WHERE ((scope)::text = 'built_in'::text);


--
-- Name: uniq_skills_system_name_version; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX uniq_skills_system_name_version ON public.skills USING btree (name, version) WHERE ((scope)::text = 'system'::text);


--
-- Name: uniq_skills_user_name_version_owner; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX uniq_skills_user_name_version_owner ON public.skills USING btree (name, version, owner_user_id) WHERE ((scope)::text = 'user'::text);


--
-- Name: uniq_workflows_system_name_version; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX uniq_workflows_system_name_version ON public.workflows USING btree (name, version) WHERE ((scope)::text = 'system'::text);


--
-- Name: uniq_workflows_user_name_version_owner; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX uniq_workflows_user_name_version_owner ON public.workflows USING btree (name, version, owner_user_id) WHERE ((scope)::text = 'user'::text);


--
-- Name: unique_root_admin; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX unique_root_admin ON public.users USING btree (is_admin) WHERE (is_admin = true);


--
-- Name: uq_bibliography_user_citation_key; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX uq_bibliography_user_citation_key ON public.bibliography_entries USING btree (user_id, citation_key);


--
-- Name: uq_bibliography_user_doi; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX uq_bibliography_user_doi ON public.bibliography_entries USING btree (user_id, lower(doi)) WHERE (doi IS NOT NULL);


--
-- Name: uq_bibliography_user_fingerprint; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX uq_bibliography_user_fingerprint ON public.bibliography_entries USING btree (user_id, dedup_fingerprint) WHERE ((doi IS NULL) AND (pmid IS NULL) AND (dedup_fingerprint IS NOT NULL));


--
-- Name: uq_bibliography_user_pmid; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX uq_bibliography_user_pmid ON public.bibliography_entries USING btree (user_id, pmid) WHERE (pmid IS NOT NULL);


--
-- Name: uq_download_instances_in_progress; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX uq_download_instances_in_progress ON public.download_instances USING btree (repository_id, provider_id, ((request_data ->> 'repository_path'::text)), ((request_data ->> 'main_filename'::text))) WHERE ((status)::text = ANY ((ARRAY['pending'::character varying, 'downloading'::character varying])::text[]));


--
-- Name: uq_file_versions_head; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX uq_file_versions_head ON public.file_versions USING btree (file_id) WHERE is_head;


--
-- Name: voice_models_one_name; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX voice_models_one_name ON public.voice_models USING btree (name);


--
-- Name: voice_runtime_versions_one_default; Type: INDEX; Schema: public; Owner: postgres
--

CREATE UNIQUE INDEX voice_runtime_versions_one_default ON public.voice_runtime_versions USING btree (is_system_default) WHERE (is_system_default = true);


--
-- Name: group_skills group_skills_scope_check; Type: TRIGGER; Schema: public; Owner: postgres
--

CREATE TRIGGER group_skills_scope_check BEFORE INSERT OR UPDATE ON public.group_skills FOR EACH ROW EXECUTE FUNCTION public.enforce_system_scope_for_group_skills();


--
-- Name: group_workflows group_workflows_scope_check; Type: TRIGGER; Schema: public; Owner: postgres
--

CREATE TRIGGER group_workflows_scope_check BEFORE INSERT OR UPDATE ON public.group_workflows FOR EACH ROW EXECUTE FUNCTION public.enforce_system_scope_for_group_workflows();


--
-- Name: desktop_settings trigger_desktop_settings_updated_at; Type: TRIGGER; Schema: public; Owner: postgres
--

CREATE TRIGGER trigger_desktop_settings_updated_at BEFORE UPDATE ON public.desktop_settings FOR EACH ROW EXECUTE FUNCTION public.update_desktop_settings_updated_at();


--
-- Name: assistants update_assistants_updated_at; Type: TRIGGER; Schema: public; Owner: postgres
--

CREATE TRIGGER update_assistants_updated_at BEFORE UPDATE ON public.assistants FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: auth_providers update_auth_providers_updated_at; Type: TRIGGER; Schema: public; Owner: postgres
--

CREATE TRIGGER update_auth_providers_updated_at BEFORE UPDATE ON public.auth_providers FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: groups update_groups_updated_at; Type: TRIGGER; Schema: public; Owner: postgres
--

CREATE TRIGGER update_groups_updated_at BEFORE UPDATE ON public.groups FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: llm_provider_files update_llm_provider_files_updated_at; Type: TRIGGER; Schema: public; Owner: postgres
--

CREATE TRIGGER update_llm_provider_files_updated_at BEFORE UPDATE ON public.llm_provider_files FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: llm_providers update_llm_providers_updated_at; Type: TRIGGER; Schema: public; Owner: postgres
--

CREATE TRIGGER update_llm_providers_updated_at BEFORE UPDATE ON public.llm_providers FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: llm_repositories update_llm_repositories_updated_at; Type: TRIGGER; Schema: public; Owner: postgres
--

CREATE TRIGGER update_llm_repositories_updated_at BEFORE UPDATE ON public.llm_repositories FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: mcp_settings update_mcp_settings_updated_at; Type: TRIGGER; Schema: public; Owner: postgres
--

CREATE TRIGGER update_mcp_settings_updated_at BEFORE UPDATE ON public.mcp_settings FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: mcp_tool_calls update_mcp_tool_calls_updated_at; Type: TRIGGER; Schema: public; Owner: postgres
--

CREATE TRIGGER update_mcp_tool_calls_updated_at BEFORE UPDATE ON public.mcp_tool_calls FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: projects update_projects_updated_at; Type: TRIGGER; Schema: public; Owner: postgres
--

CREATE TRIGGER update_projects_updated_at BEFORE UPDATE ON public.projects FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: tool_use_approvals update_tool_use_approvals_updated_at; Type: TRIGGER; Schema: public; Owner: postgres
--

CREATE TRIGGER update_tool_use_approvals_updated_at BEFORE UPDATE ON public.tool_use_approvals FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: user_auth_links update_user_auth_links_updated_at; Type: TRIGGER; Schema: public; Owner: postgres
--

CREATE TRIGGER update_user_auth_links_updated_at BEFORE UPDATE ON public.user_auth_links FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: user_mcp_defaults update_user_mcp_defaults_updated_at; Type: TRIGGER; Schema: public; Owner: postgres
--

CREATE TRIGGER update_user_mcp_defaults_updated_at BEFORE UPDATE ON public.user_mcp_defaults FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: users update_users_updated_at; Type: TRIGGER; Schema: public; Owner: postgres
--

CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON public.users FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: assistant_core_memory assistant_core_memory_assistant_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.assistant_core_memory
    ADD CONSTRAINT assistant_core_memory_assistant_id_fkey FOREIGN KEY (assistant_id) REFERENCES public.assistants(id) ON DELETE CASCADE;


--
-- Name: assistant_core_memory assistant_core_memory_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.assistant_core_memory
    ADD CONSTRAINT assistant_core_memory_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: assistants assistants_created_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.assistants
    ADD CONSTRAINT assistants_created_by_fkey FOREIGN KEY (created_by) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: bibliography_entries bibliography_entries_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.bibliography_entries
    ADD CONSTRAINT bibliography_entries_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: branch_messages branch_messages_branch_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.branch_messages
    ADD CONSTRAINT branch_messages_branch_id_fkey FOREIGN KEY (branch_id) REFERENCES public.branches(id) ON DELETE CASCADE;


--
-- Name: branch_messages branch_messages_message_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.branch_messages
    ADD CONSTRAINT branch_messages_message_id_fkey FOREIGN KEY (message_id) REFERENCES public.messages(id) ON DELETE CASCADE;


--
-- Name: branches branches_conversation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.branches
    ADD CONSTRAINT branches_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE CASCADE;


--
-- Name: branches branches_parent_branch_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.branches
    ADD CONSTRAINT branches_parent_branch_id_fkey FOREIGN KEY (parent_branch_id) REFERENCES public.branches(id) ON DELETE SET NULL;


--
-- Name: conversation_deliverables conversation_deliverables_conversation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversation_deliverables
    ADD CONSTRAINT conversation_deliverables_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE CASCADE;


--
-- Name: conversation_deliverables conversation_deliverables_file_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversation_deliverables
    ADD CONSTRAINT conversation_deliverables_file_id_fkey FOREIGN KEY (file_id) REFERENCES public.files(id) ON DELETE CASCADE;


--
-- Name: conversation_knowledge_bases conversation_knowledge_bases_conversation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversation_knowledge_bases
    ADD CONSTRAINT conversation_knowledge_bases_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE CASCADE;


--
-- Name: conversation_knowledge_bases conversation_knowledge_bases_knowledge_base_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversation_knowledge_bases
    ADD CONSTRAINT conversation_knowledge_bases_knowledge_base_id_fkey FOREIGN KEY (knowledge_base_id) REFERENCES public.knowledge_bases(id) ON DELETE CASCADE;


--
-- Name: conversation_memory_settings conversation_memory_settings_conversation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversation_memory_settings
    ADD CONSTRAINT conversation_memory_settings_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE CASCADE;


--
-- Name: conversation_skill_overrides conversation_skill_overrides_conversation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversation_skill_overrides
    ADD CONSTRAINT conversation_skill_overrides_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE CASCADE;


--
-- Name: conversation_skill_overrides conversation_skill_overrides_skill_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversation_skill_overrides
    ADD CONSTRAINT conversation_skill_overrides_skill_id_fkey FOREIGN KEY (skill_id) REFERENCES public.skills(id) ON DELETE CASCADE;


--
-- Name: conversation_summaries conversation_summaries_branch_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversation_summaries
    ADD CONSTRAINT conversation_summaries_branch_id_fkey FOREIGN KEY (branch_id) REFERENCES public.branches(id) ON DELETE CASCADE;


--
-- Name: conversation_summaries conversation_summaries_summarized_up_to_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversation_summaries
    ADD CONSTRAINT conversation_summaries_summarized_up_to_id_fkey FOREIGN KEY (summarized_up_to_id) REFERENCES public.messages(id) ON DELETE SET NULL;


--
-- Name: conversation_summarization_settings conversation_summarization_settings_conversation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversation_summarization_settings
    ADD CONSTRAINT conversation_summarization_settings_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE CASCADE;


--
-- Name: conversations conversations_model_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversations
    ADD CONSTRAINT conversations_model_id_fkey FOREIGN KEY (model_id) REFERENCES public.llm_models(id) ON DELETE SET NULL;


--
-- Name: conversations conversations_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversations
    ADD CONSTRAINT conversations_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: download_instances download_instances_model_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.download_instances
    ADD CONSTRAINT download_instances_model_id_fkey FOREIGN KEY (model_id) REFERENCES public.llm_models(id) ON DELETE SET NULL;


--
-- Name: download_instances download_instances_provider_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.download_instances
    ADD CONSTRAINT download_instances_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.llm_providers(id) ON DELETE CASCADE;


--
-- Name: download_instances download_instances_repository_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.download_instances
    ADD CONSTRAINT download_instances_repository_id_fkey FOREIGN KEY (repository_id) REFERENCES public.llm_repositories(id) ON DELETE CASCADE;


--
-- Name: file_chunks file_chunks_file_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.file_chunks
    ADD CONSTRAINT file_chunks_file_id_fkey FOREIGN KEY (file_id) REFERENCES public.files(id) ON DELETE CASCADE;


--
-- Name: file_chunks file_chunks_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.file_chunks
    ADD CONSTRAINT file_chunks_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: file_index_state file_index_state_file_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.file_index_state
    ADD CONSTRAINT file_index_state_file_id_fkey FOREIGN KEY (file_id) REFERENCES public.files(id) ON DELETE CASCADE;


--
-- Name: file_index_state file_index_state_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.file_index_state
    ADD CONSTRAINT file_index_state_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: file_rag_admin_settings file_rag_admin_settings_embedding_model_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.file_rag_admin_settings
    ADD CONSTRAINT file_rag_admin_settings_embedding_model_id_fkey FOREIGN KEY (embedding_model_id) REFERENCES public.llm_models(id) ON DELETE SET NULL;


--
-- Name: file_rag_admin_settings file_rag_admin_settings_reranker_model_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.file_rag_admin_settings
    ADD CONSTRAINT file_rag_admin_settings_reranker_model_id_fkey FOREIGN KEY (reranker_model_id) REFERENCES public.llm_models(id) ON DELETE SET NULL;


--
-- Name: file_versions file_versions_file_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.file_versions
    ADD CONSTRAINT file_versions_file_id_fkey FOREIGN KEY (file_id) REFERENCES public.files(id) ON DELETE CASCADE;


--
-- Name: files files_current_version_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.files
    ADD CONSTRAINT files_current_version_id_fkey FOREIGN KEY (current_version_id) REFERENCES public.file_versions(id) DEFERRABLE INITIALLY DEFERRED;


--
-- Name: files files_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.files
    ADD CONSTRAINT files_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: files files_workflow_run_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.files
    ADD CONSTRAINT files_workflow_run_id_fkey FOREIGN KEY (workflow_run_id) REFERENCES public.workflow_runs(id) ON DELETE SET NULL;


--
-- Name: conversations fk_conversations_active_branch; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.conversations
    ADD CONSTRAINT fk_conversations_active_branch FOREIGN KEY (active_branch_id) REFERENCES public.branches(id) ON DELETE SET NULL;


--
-- Name: group_skills group_skills_group_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.group_skills
    ADD CONSTRAINT group_skills_group_id_fkey FOREIGN KEY (group_id) REFERENCES public.groups(id) ON DELETE CASCADE;


--
-- Name: group_skills group_skills_skill_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.group_skills
    ADD CONSTRAINT group_skills_skill_id_fkey FOREIGN KEY (skill_id) REFERENCES public.skills(id) ON DELETE CASCADE;


--
-- Name: group_workflows group_workflows_group_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.group_workflows
    ADD CONSTRAINT group_workflows_group_id_fkey FOREIGN KEY (group_id) REFERENCES public.groups(id) ON DELETE CASCADE;


--
-- Name: group_workflows group_workflows_workflow_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.group_workflows
    ADD CONSTRAINT group_workflows_workflow_id_fkey FOREIGN KEY (workflow_id) REFERENCES public.workflows(id) ON DELETE CASCADE;


--
-- Name: host_mounts host_mounts_conversation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.host_mounts
    ADD CONSTRAINT host_mounts_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE CASCADE;


--
-- Name: host_mounts host_mounts_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.host_mounts
    ADD CONSTRAINT host_mounts_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;


--
-- Name: host_mounts host_mounts_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.host_mounts
    ADD CONSTRAINT host_mounts_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: hub_entities hub_entities_created_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.hub_entities
    ADD CONSTRAINT hub_entities_created_by_fkey FOREIGN KEY (created_by) REFERENCES public.users(id) ON DELETE SET NULL;


--
-- Name: knowledge_base_documents knowledge_base_documents_file_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.knowledge_base_documents
    ADD CONSTRAINT knowledge_base_documents_file_id_fkey FOREIGN KEY (file_id) REFERENCES public.files(id) ON DELETE CASCADE;


--
-- Name: knowledge_base_documents knowledge_base_documents_knowledge_base_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.knowledge_base_documents
    ADD CONSTRAINT knowledge_base_documents_knowledge_base_id_fkey FOREIGN KEY (knowledge_base_id) REFERENCES public.knowledge_bases(id) ON DELETE CASCADE;


--
-- Name: knowledge_bases knowledge_bases_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.knowledge_bases
    ADD CONSTRAINT knowledge_bases_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: llm_model_files llm_model_files_model_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_model_files
    ADD CONSTRAINT llm_model_files_model_id_fkey FOREIGN KEY (model_id) REFERENCES public.llm_models(id) ON DELETE CASCADE;


--
-- Name: llm_models llm_models_provider_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_models
    ADD CONSTRAINT llm_models_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.llm_providers(id) ON DELETE CASCADE;


--
-- Name: llm_models llm_models_required_runtime_version_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_models
    ADD CONSTRAINT llm_models_required_runtime_version_id_fkey FOREIGN KEY (required_runtime_version_id) REFERENCES public.llm_runtime_versions(id) ON DELETE SET NULL;


--
-- Name: llm_provider_files llm_provider_files_file_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_provider_files
    ADD CONSTRAINT llm_provider_files_file_id_fkey FOREIGN KEY (file_id) REFERENCES public.files(id) ON DELETE CASCADE;


--
-- Name: llm_provider_files llm_provider_files_provider_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_provider_files
    ADD CONSTRAINT llm_provider_files_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.llm_providers(id) ON DELETE CASCADE;


--
-- Name: llm_providers llm_providers_default_runtime_version_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_providers
    ADD CONSTRAINT llm_providers_default_runtime_version_id_fkey FOREIGN KEY (default_runtime_version_id) REFERENCES public.llm_runtime_versions(id) ON DELETE SET NULL;


--
-- Name: llm_runtime_instances llm_runtime_instances_model_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_runtime_instances
    ADD CONSTRAINT llm_runtime_instances_model_id_fkey FOREIGN KEY (model_id) REFERENCES public.llm_models(id) ON DELETE CASCADE;


--
-- Name: llm_runtime_instances llm_runtime_instances_provider_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_runtime_instances
    ADD CONSTRAINT llm_runtime_instances_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.llm_providers(id) ON DELETE CASCADE;


--
-- Name: llm_runtime_instances llm_runtime_instances_runtime_version_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.llm_runtime_instances
    ADD CONSTRAINT llm_runtime_instances_runtime_version_id_fkey FOREIGN KEY (runtime_version_id) REFERENCES public.llm_runtime_versions(id) ON DELETE SET NULL;


--
-- Name: magic_link_tokens magic_link_tokens_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.magic_link_tokens
    ADD CONSTRAINT magic_link_tokens_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: mcp_server_oauth_configs mcp_server_oauth_configs_server_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.mcp_server_oauth_configs
    ADD CONSTRAINT mcp_server_oauth_configs_server_id_fkey FOREIGN KEY (server_id) REFERENCES public.mcp_servers(id) ON DELETE CASCADE;


--
-- Name: mcp_servers mcp_servers_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.mcp_servers
    ADD CONSTRAINT mcp_servers_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: mcp_settings mcp_settings_conversation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.mcp_settings
    ADD CONSTRAINT mcp_settings_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE CASCADE;


--
-- Name: mcp_settings mcp_settings_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.mcp_settings
    ADD CONSTRAINT mcp_settings_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;


--
-- Name: mcp_settings mcp_settings_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.mcp_settings
    ADD CONSTRAINT mcp_settings_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: mcp_tool_calls mcp_tool_calls_branch_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.mcp_tool_calls
    ADD CONSTRAINT mcp_tool_calls_branch_id_fkey FOREIGN KEY (branch_id) REFERENCES public.branches(id) ON DELETE SET NULL;


--
-- Name: mcp_tool_calls mcp_tool_calls_conversation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.mcp_tool_calls
    ADD CONSTRAINT mcp_tool_calls_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE SET NULL;


--
-- Name: mcp_tool_calls mcp_tool_calls_message_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.mcp_tool_calls
    ADD CONSTRAINT mcp_tool_calls_message_id_fkey FOREIGN KEY (message_id) REFERENCES public.messages(id) ON DELETE SET NULL;


--
-- Name: mcp_tool_calls mcp_tool_calls_server_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.mcp_tool_calls
    ADD CONSTRAINT mcp_tool_calls_server_id_fkey FOREIGN KEY (server_id) REFERENCES public.mcp_servers(id) ON DELETE SET NULL;


--
-- Name: mcp_tool_calls mcp_tool_calls_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.mcp_tool_calls
    ADD CONSTRAINT mcp_tool_calls_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: mcp_tool_calls mcp_tool_calls_workflow_run_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.mcp_tool_calls
    ADD CONSTRAINT mcp_tool_calls_workflow_run_id_fkey FOREIGN KEY (workflow_run_id) REFERENCES public.workflow_runs(id) ON DELETE SET NULL;


--
-- Name: mcp_user_policy mcp_user_policy_updated_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.mcp_user_policy
    ADD CONSTRAINT mcp_user_policy_updated_by_fkey FOREIGN KEY (updated_by) REFERENCES public.users(id) ON DELETE SET NULL;


--
-- Name: memory_admin_settings memory_admin_settings_default_extraction_model_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.memory_admin_settings
    ADD CONSTRAINT memory_admin_settings_default_extraction_model_id_fkey FOREIGN KEY (default_extraction_model_id) REFERENCES public.llm_models(id) ON DELETE SET NULL;


--
-- Name: memory_admin_settings memory_admin_settings_embedding_model_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.memory_admin_settings
    ADD CONSTRAINT memory_admin_settings_embedding_model_id_fkey FOREIGN KEY (embedding_model_id) REFERENCES public.llm_models(id) ON DELETE SET NULL;


--
-- Name: memory_audit_log memory_audit_log_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.memory_audit_log
    ADD CONSTRAINT memory_audit_log_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: message_assistant message_assistant_message_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.message_assistant
    ADD CONSTRAINT message_assistant_message_id_fkey FOREIGN KEY (message_id) REFERENCES public.messages(id) ON DELETE CASCADE;


--
-- Name: message_contents message_contents_message_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.message_contents
    ADD CONSTRAINT message_contents_message_id_fkey FOREIGN KEY (message_id) REFERENCES public.messages(id) ON DELETE CASCADE;


--
-- Name: message_mcp_servers message_mcp_servers_message_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.message_mcp_servers
    ADD CONSTRAINT message_mcp_servers_message_id_fkey FOREIGN KEY (message_id) REFERENCES public.messages(id) ON DELETE CASCADE;


--
-- Name: notifications notifications_conversation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.notifications
    ADD CONSTRAINT notifications_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE SET NULL;


--
-- Name: notifications notifications_scheduled_task_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.notifications
    ADD CONSTRAINT notifications_scheduled_task_id_fkey FOREIGN KEY (scheduled_task_id) REFERENCES public.scheduled_tasks(id) ON DELETE SET NULL;


--
-- Name: notifications notifications_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.notifications
    ADD CONSTRAINT notifications_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: notifications notifications_workflow_run_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.notifications
    ADD CONSTRAINT notifications_workflow_run_id_fkey FOREIGN KEY (workflow_run_id) REFERENCES public.workflow_runs(id) ON DELETE SET NULL;


--
-- Name: oauth_sessions oauth_sessions_provider_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.oauth_sessions
    ADD CONSTRAINT oauth_sessions_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.auth_providers(id) ON DELETE CASCADE;


--
-- Name: pending_account_links pending_account_links_provider_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.pending_account_links
    ADD CONSTRAINT pending_account_links_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.auth_providers(id) ON DELETE CASCADE;


--
-- Name: pending_account_links pending_account_links_target_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.pending_account_links
    ADD CONSTRAINT pending_account_links_target_user_id_fkey FOREIGN KEY (target_user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: project_bibliography project_bibliography_entry_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.project_bibliography
    ADD CONSTRAINT project_bibliography_entry_id_fkey FOREIGN KEY (entry_id) REFERENCES public.bibliography_entries(id) ON DELETE CASCADE;


--
-- Name: project_bibliography project_bibliography_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.project_bibliography
    ADD CONSTRAINT project_bibliography_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;


--
-- Name: project_conversations project_conversations_conversation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.project_conversations
    ADD CONSTRAINT project_conversations_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE CASCADE;


--
-- Name: project_conversations project_conversations_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.project_conversations
    ADD CONSTRAINT project_conversations_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;


--
-- Name: project_files project_files_file_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.project_files
    ADD CONSTRAINT project_files_file_id_fkey FOREIGN KEY (file_id) REFERENCES public.files(id) ON DELETE CASCADE;


--
-- Name: project_files project_files_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.project_files
    ADD CONSTRAINT project_files_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;


--
-- Name: project_knowledge_bases project_knowledge_bases_knowledge_base_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.project_knowledge_bases
    ADD CONSTRAINT project_knowledge_bases_knowledge_base_id_fkey FOREIGN KEY (knowledge_base_id) REFERENCES public.knowledge_bases(id) ON DELETE CASCADE;


--
-- Name: project_knowledge_bases project_knowledge_bases_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.project_knowledge_bases
    ADD CONSTRAINT project_knowledge_bases_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;


--
-- Name: projects projects_default_assistant_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.projects
    ADD CONSTRAINT projects_default_assistant_id_fkey FOREIGN KEY (default_assistant_id) REFERENCES public.assistants(id) ON DELETE SET NULL;


--
-- Name: projects projects_default_model_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.projects
    ADD CONSTRAINT projects_default_model_id_fkey FOREIGN KEY (default_model_id) REFERENCES public.llm_models(id) ON DELETE SET NULL;


--
-- Name: projects projects_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.projects
    ADD CONSTRAINT projects_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: refresh_tokens refresh_tokens_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.refresh_tokens
    ADD CONSTRAINT refresh_tokens_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: sandbox_workspace_files sandbox_workspace_files_base_version_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.sandbox_workspace_files
    ADD CONSTRAINT sandbox_workspace_files_base_version_id_fkey FOREIGN KEY (base_version_id) REFERENCES public.file_versions(id) ON DELETE CASCADE;


--
-- Name: sandbox_workspace_files sandbox_workspace_files_conversation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.sandbox_workspace_files
    ADD CONSTRAINT sandbox_workspace_files_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE CASCADE;


--
-- Name: sandbox_workspace_files sandbox_workspace_files_file_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.sandbox_workspace_files
    ADD CONSTRAINT sandbox_workspace_files_file_id_fkey FOREIGN KEY (file_id) REFERENCES public.files(id) ON DELETE CASCADE;


--
-- Name: scheduled_task_runs scheduled_task_runs_conversation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.scheduled_task_runs
    ADD CONSTRAINT scheduled_task_runs_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE SET NULL;


--
-- Name: scheduled_task_runs scheduled_task_runs_notification_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.scheduled_task_runs
    ADD CONSTRAINT scheduled_task_runs_notification_id_fkey FOREIGN KEY (notification_id) REFERENCES public.notifications(id) ON DELETE SET NULL;


--
-- Name: scheduled_task_runs scheduled_task_runs_scheduled_task_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.scheduled_task_runs
    ADD CONSTRAINT scheduled_task_runs_scheduled_task_id_fkey FOREIGN KEY (scheduled_task_id) REFERENCES public.scheduled_tasks(id) ON DELETE CASCADE;


--
-- Name: scheduled_task_runs scheduled_task_runs_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.scheduled_task_runs
    ADD CONSTRAINT scheduled_task_runs_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: scheduled_task_runs scheduled_task_runs_workflow_run_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.scheduled_task_runs
    ADD CONSTRAINT scheduled_task_runs_workflow_run_id_fkey FOREIGN KEY (workflow_run_id) REFERENCES public.workflow_runs(id) ON DELETE SET NULL;


--
-- Name: scheduled_tasks scheduled_tasks_assistant_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.scheduled_tasks
    ADD CONSTRAINT scheduled_tasks_assistant_id_fkey FOREIGN KEY (assistant_id) REFERENCES public.assistants(id) ON DELETE SET NULL;


--
-- Name: scheduled_tasks scheduled_tasks_bound_conversation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.scheduled_tasks
    ADD CONSTRAINT scheduled_tasks_bound_conversation_id_fkey FOREIGN KEY (bound_conversation_id) REFERENCES public.conversations(id) ON DELETE SET NULL;


--
-- Name: scheduled_tasks scheduled_tasks_model_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.scheduled_tasks
    ADD CONSTRAINT scheduled_tasks_model_id_fkey FOREIGN KEY (model_id) REFERENCES public.llm_models(id) ON DELETE SET NULL;


--
-- Name: scheduled_tasks scheduled_tasks_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.scheduled_tasks
    ADD CONSTRAINT scheduled_tasks_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: scheduled_tasks scheduled_tasks_workflow_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.scheduled_tasks
    ADD CONSTRAINT scheduled_tasks_workflow_id_fkey FOREIGN KEY (workflow_id) REFERENCES public.workflows(id) ON DELETE SET NULL;


--
-- Name: skills skills_created_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.skills
    ADD CONSTRAINT skills_created_by_fkey FOREIGN KEY (created_by) REFERENCES public.users(id) ON DELETE SET NULL;


--
-- Name: skills skills_owner_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.skills
    ADD CONSTRAINT skills_owner_user_id_fkey FOREIGN KEY (owner_user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: summarization_admin_settings summarization_admin_settings_default_summarization_model_i_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.summarization_admin_settings
    ADD CONSTRAINT summarization_admin_settings_default_summarization_model_i_fkey FOREIGN KEY (default_summarization_model_id) REFERENCES public.llm_models(id) ON DELETE SET NULL;


--
-- Name: tool_use_approvals tool_use_approvals_approved_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.tool_use_approvals
    ADD CONSTRAINT tool_use_approvals_approved_by_fkey FOREIGN KEY (approved_by) REFERENCES public.users(id) ON DELETE SET NULL;


--
-- Name: tool_use_approvals tool_use_approvals_branch_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.tool_use_approvals
    ADD CONSTRAINT tool_use_approvals_branch_id_fkey FOREIGN KEY (branch_id) REFERENCES public.branches(id) ON DELETE CASCADE;


--
-- Name: tool_use_approvals tool_use_approvals_conversation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.tool_use_approvals
    ADD CONSTRAINT tool_use_approvals_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE CASCADE;


--
-- Name: tool_use_approvals tool_use_approvals_message_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.tool_use_approvals
    ADD CONSTRAINT tool_use_approvals_message_id_fkey FOREIGN KEY (message_id) REFERENCES public.messages(id) ON DELETE CASCADE;


--
-- Name: tool_use_approvals tool_use_approvals_server_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.tool_use_approvals
    ADD CONSTRAINT tool_use_approvals_server_id_fkey FOREIGN KEY (server_id) REFERENCES public.mcp_servers(id) ON DELETE CASCADE;


--
-- Name: tool_use_approvals tool_use_approvals_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.tool_use_approvals
    ADD CONSTRAINT tool_use_approvals_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: user_auth_links user_auth_links_provider_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_auth_links
    ADD CONSTRAINT user_auth_links_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.auth_providers(id) ON DELETE CASCADE;


--
-- Name: user_auth_links user_auth_links_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_auth_links
    ADD CONSTRAINT user_auth_links_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: user_group_llm_providers user_group_llm_providers_group_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_group_llm_providers
    ADD CONSTRAINT user_group_llm_providers_group_id_fkey FOREIGN KEY (group_id) REFERENCES public.groups(id) ON DELETE CASCADE;


--
-- Name: user_group_llm_providers user_group_llm_providers_provider_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_group_llm_providers
    ADD CONSTRAINT user_group_llm_providers_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.llm_providers(id) ON DELETE CASCADE;


--
-- Name: user_group_mcp_servers user_group_mcp_servers_group_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_group_mcp_servers
    ADD CONSTRAINT user_group_mcp_servers_group_id_fkey FOREIGN KEY (group_id) REFERENCES public.groups(id) ON DELETE CASCADE;


--
-- Name: user_group_mcp_servers user_group_mcp_servers_mcp_server_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_group_mcp_servers
    ADD CONSTRAINT user_group_mcp_servers_mcp_server_id_fkey FOREIGN KEY (mcp_server_id) REFERENCES public.mcp_servers(id) ON DELETE CASCADE;


--
-- Name: user_groups user_groups_assigned_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_groups
    ADD CONSTRAINT user_groups_assigned_by_fkey FOREIGN KEY (assigned_by) REFERENCES public.users(id) ON DELETE SET NULL;


--
-- Name: user_groups user_groups_group_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_groups
    ADD CONSTRAINT user_groups_group_id_fkey FOREIGN KEY (group_id) REFERENCES public.groups(id) ON DELETE CASCADE;


--
-- Name: user_groups user_groups_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_groups
    ADD CONSTRAINT user_groups_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: user_lit_search_connector_keys user_lit_search_connector_keys_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_lit_search_connector_keys
    ADD CONSTRAINT user_lit_search_connector_keys_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: user_llm_provider_api_keys user_llm_provider_api_keys_provider_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_llm_provider_api_keys
    ADD CONSTRAINT user_llm_provider_api_keys_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.llm_providers(id) ON DELETE CASCADE;


--
-- Name: user_llm_provider_api_keys user_llm_provider_api_keys_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_llm_provider_api_keys
    ADD CONSTRAINT user_llm_provider_api_keys_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: user_mcp_defaults user_mcp_defaults_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_mcp_defaults
    ADD CONSTRAINT user_mcp_defaults_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: user_memories user_memories_conversation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_memories
    ADD CONSTRAINT user_memories_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE CASCADE;


--
-- Name: user_memories user_memories_project_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_memories
    ADD CONSTRAINT user_memories_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;


--
-- Name: user_memories user_memories_source_message_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_memories
    ADD CONSTRAINT user_memories_source_message_id_fkey FOREIGN KEY (source_message_id) REFERENCES public.messages(id) ON DELETE SET NULL;


--
-- Name: user_memories user_memories_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_memories
    ADD CONSTRAINT user_memories_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: user_memory_settings user_memory_settings_extraction_model_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_memory_settings
    ADD CONSTRAINT user_memory_settings_extraction_model_id_fkey FOREIGN KEY (extraction_model_id) REFERENCES public.llm_models(id) ON DELETE SET NULL;


--
-- Name: user_memory_settings user_memory_settings_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_memory_settings
    ADD CONSTRAINT user_memory_settings_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: user_onboarding user_onboarding_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_onboarding
    ADD CONSTRAINT user_onboarding_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: user_web_search_provider_keys user_web_search_provider_keys_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_web_search_provider_keys
    ADD CONSTRAINT user_web_search_provider_keys_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: voice_runtime_instance voice_runtime_instance_runtime_version_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.voice_runtime_instance
    ADD CONSTRAINT voice_runtime_instance_runtime_version_id_fkey FOREIGN KEY (runtime_version_id) REFERENCES public.voice_runtime_versions(id) ON DELETE SET NULL;


--
-- Name: workflow_runs workflow_runs_conversation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.workflow_runs
    ADD CONSTRAINT workflow_runs_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE SET NULL;


--
-- Name: workflow_runs workflow_runs_model_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.workflow_runs
    ADD CONSTRAINT workflow_runs_model_id_fkey FOREIGN KEY (model_id) REFERENCES public.llm_models(id) ON DELETE SET NULL;


--
-- Name: workflow_runs workflow_runs_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.workflow_runs
    ADD CONSTRAINT workflow_runs_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: workflow_runs workflow_runs_workflow_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.workflow_runs
    ADD CONSTRAINT workflow_runs_workflow_id_fkey FOREIGN KEY (workflow_id) REFERENCES public.workflows(id) ON DELETE CASCADE;


--
-- Name: workflows workflows_conversation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.workflows
    ADD CONSTRAINT workflows_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE CASCADE;


--
-- Name: workflows workflows_created_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.workflows
    ADD CONSTRAINT workflows_created_by_fkey FOREIGN KEY (created_by) REFERENCES public.users(id) ON DELETE SET NULL;


--
-- Name: workflows workflows_owner_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.workflows
    ADD CONSTRAINT workflows_owner_user_id_fkey FOREIGN KEY (owner_user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: SCHEMA public; Type: ACL; Schema: -; Owner: postgres
--

REVOKE USAGE ON SCHEMA public FROM PUBLIC;
GRANT ALL ON SCHEMA public TO PUBLIC;


--
-- PostgreSQL database dump complete
--

\unrestrict z4RDTdKeyAR6vn3NocKseuwwDgmfsk8vWVTMZjaLN6j994Ihat9M5OgeOteptLx

