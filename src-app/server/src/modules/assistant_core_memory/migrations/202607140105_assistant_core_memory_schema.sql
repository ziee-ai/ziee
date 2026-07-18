-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- assistant_core_memory module tables + indexes + triggers.

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

ALTER TABLE ONLY public.assistant_core_memory
    ADD CONSTRAINT assistant_core_memory_assistant_id_user_id_block_label_key UNIQUE (assistant_id, user_id, block_label);

ALTER TABLE ONLY public.assistant_core_memory
    ADD CONSTRAINT assistant_core_memory_pkey PRIMARY KEY (id);

CREATE INDEX idx_core_memory_lookup ON public.assistant_core_memory USING btree (user_id, assistant_id);
