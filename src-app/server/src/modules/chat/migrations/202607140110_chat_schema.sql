-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- chat module tables + indexes + triggers.

CREATE TABLE public.branch_messages (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    branch_id uuid NOT NULL,
    message_id uuid NOT NULL,
    is_clone boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE TABLE public.branches (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    conversation_id uuid NOT NULL,
    parent_branch_id uuid,
    created_from_message_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    fork_level text DEFAULT 'user'::text NOT NULL,
    CONSTRAINT branches_fork_level_check CHECK ((fork_level = ANY (ARRAY['user'::text, 'assistant'::text])))
);

CREATE TABLE public.conversation_deliverables (
    conversation_id uuid NOT NULL,
    file_id uuid NOT NULL,
    pinned boolean DEFAULT true NOT NULL,
    title text,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.conversations (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    model_id uuid,
    title character varying(500),
    active_branch_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.message_assistant (
    message_id uuid NOT NULL,
    assistant_id uuid NOT NULL
);

CREATE TABLE public.message_contents (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    message_id uuid NOT NULL,
    content_type character varying(50) NOT NULL,
    content jsonb NOT NULL,
    sequence_order integer DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.message_mcp_servers (
    message_id uuid NOT NULL,
    server_id uuid NOT NULL
);

CREATE TABLE public.messages (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    role character varying(20) NOT NULL,
    originated_from_id uuid NOT NULL,
    edit_count integer DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    model_id uuid
);

ALTER TABLE ONLY public.branch_messages
    ADD CONSTRAINT branch_messages_branch_id_message_id_key UNIQUE (branch_id, message_id);

ALTER TABLE ONLY public.branch_messages
    ADD CONSTRAINT branch_messages_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.branches
    ADD CONSTRAINT branches_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.conversation_deliverables
    ADD CONSTRAINT conversation_deliverables_pkey PRIMARY KEY (conversation_id, file_id);

ALTER TABLE ONLY public.conversations
    ADD CONSTRAINT conversations_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.message_assistant
    ADD CONSTRAINT message_assistant_pkey PRIMARY KEY (message_id);

ALTER TABLE ONLY public.message_contents
    ADD CONSTRAINT message_contents_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.message_mcp_servers
    ADD CONSTRAINT message_mcp_servers_pkey PRIMARY KEY (message_id, server_id);

ALTER TABLE ONLY public.messages
    ADD CONSTRAINT messages_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.message_contents
    ADD CONSTRAINT uq_message_contents_message_sequence UNIQUE (message_id, sequence_order);

CREATE INDEX idx_branch_messages_branch_id ON public.branch_messages USING btree (branch_id, created_at);

CREATE INDEX idx_branch_messages_message_id ON public.branch_messages USING btree (message_id);

CREATE INDEX idx_branches_conversation_id ON public.branches USING btree (conversation_id);

CREATE INDEX idx_branches_created_from_message_id ON public.branches USING btree (created_from_message_id);

CREATE INDEX idx_branches_parent_branch_id ON public.branches USING btree (parent_branch_id);

CREATE INDEX idx_conversation_deliverables_file_id ON public.conversation_deliverables USING btree (file_id);

CREATE INDEX idx_conversations_created_at ON public.conversations USING btree (created_at DESC);

CREATE INDEX idx_conversations_model_id ON public.conversations USING btree (model_id);

CREATE INDEX idx_conversations_user_id ON public.conversations USING btree (user_id);

CREATE INDEX idx_message_contents_content ON public.message_contents USING gin (content);

CREATE INDEX idx_message_contents_message_id ON public.message_contents USING btree (message_id);

CREATE UNIQUE INDEX idx_message_contents_message_seq_unique ON public.message_contents USING btree (message_id, sequence_order);

CREATE INDEX idx_message_contents_type ON public.message_contents USING btree (content_type);

CREATE INDEX idx_messages_created_at ON public.messages USING btree (created_at DESC);

CREATE INDEX idx_messages_originated_from_id ON public.messages USING btree (originated_from_id);

CREATE INDEX idx_messages_role ON public.messages USING btree (role);
