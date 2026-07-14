-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- js_tool module tables + indexes + triggers.

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

ALTER TABLE ONLY public.js_tool_settings
    ADD CONSTRAINT js_tool_settings_pkey PRIMARY KEY (id);
