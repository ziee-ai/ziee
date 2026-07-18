-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- summarization module tables + indexes + triggers.

CREATE TABLE public.conversation_summaries (
    branch_id uuid NOT NULL,
    summary_text text NOT NULL,
    summarized_up_to_id uuid,
    message_count integer DEFAULT 0 NOT NULL,
    model_used text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.conversation_summarization_settings (
    conversation_id uuid NOT NULL,
    summarization_mode text DEFAULT 'inherit'::text NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT conversation_summarization_settings_summarization_mode_check CHECK ((summarization_mode = ANY (ARRAY['inherit'::text, 'on'::text, 'off'::text])))
);

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

ALTER TABLE ONLY public.conversation_summaries
    ADD CONSTRAINT conversation_summaries_pkey PRIMARY KEY (branch_id);

ALTER TABLE ONLY public.conversation_summarization_settings
    ADD CONSTRAINT conversation_summarization_settings_pkey PRIMARY KEY (conversation_id);

ALTER TABLE ONLY public.summarization_admin_settings
    ADD CONSTRAINT summarization_admin_settings_pkey PRIMARY KEY (id);
