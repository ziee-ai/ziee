-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- project module tables + indexes + triggers.

CREATE TABLE public.project_conversations (
    conversation_id uuid NOT NULL,
    project_id uuid NOT NULL,
    attached_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.project_files (
    project_id uuid NOT NULL,
    file_id uuid NOT NULL,
    added_at timestamp with time zone DEFAULT now() NOT NULL
);

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

ALTER TABLE ONLY public.project_conversations
    ADD CONSTRAINT project_conversations_pkey PRIMARY KEY (conversation_id);

ALTER TABLE ONLY public.project_files
    ADD CONSTRAINT project_files_pkey PRIMARY KEY (project_id, file_id);

ALTER TABLE ONLY public.projects
    ADD CONSTRAINT projects_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.projects
    ADD CONSTRAINT projects_user_name_unique UNIQUE (user_id, name);

CREATE INDEX idx_project_conversations_project_id ON public.project_conversations USING btree (project_id);

CREATE INDEX idx_project_files_file_id ON public.project_files USING btree (file_id);

CREATE INDEX idx_projects_updated_at ON public.projects USING btree (updated_at DESC);

CREATE INDEX idx_projects_user_id ON public.projects USING btree (user_id);

CREATE TRIGGER update_projects_updated_at BEFORE UPDATE ON public.projects FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
