-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- knowledge_base module tables + indexes + triggers.

CREATE TABLE public.conversation_knowledge_bases (
    conversation_id uuid NOT NULL,
    knowledge_base_id uuid NOT NULL,
    added_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.knowledge_base_documents (
    knowledge_base_id uuid NOT NULL,
    file_id uuid NOT NULL,
    added_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.knowledge_bases (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    name text NOT NULL,
    description text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.project_knowledge_bases (
    project_id uuid NOT NULL,
    knowledge_base_id uuid NOT NULL,
    added_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.conversation_knowledge_bases
    ADD CONSTRAINT conversation_knowledge_bases_pkey PRIMARY KEY (conversation_id, knowledge_base_id);

ALTER TABLE ONLY public.knowledge_base_documents
    ADD CONSTRAINT knowledge_base_documents_pkey PRIMARY KEY (knowledge_base_id, file_id);

ALTER TABLE ONLY public.knowledge_bases
    ADD CONSTRAINT knowledge_bases_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.project_knowledge_bases
    ADD CONSTRAINT project_knowledge_bases_pkey PRIMARY KEY (project_id, knowledge_base_id);

CREATE INDEX idx_conversation_knowledge_bases_kb ON public.conversation_knowledge_bases USING btree (knowledge_base_id);

CREATE INDEX idx_knowledge_base_documents_file ON public.knowledge_base_documents USING btree (file_id);

CREATE INDEX idx_knowledge_bases_user ON public.knowledge_bases USING btree (user_id);

CREATE UNIQUE INDEX idx_knowledge_bases_user_name ON public.knowledge_bases USING btree (user_id, lower(name));

CREATE INDEX idx_project_knowledge_bases_kb ON public.project_knowledge_bases USING btree (knowledge_base_id);
