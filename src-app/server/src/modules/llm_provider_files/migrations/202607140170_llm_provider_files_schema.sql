-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- llm_provider_files module tables + indexes + triggers.

CREATE TABLE public.llm_provider_files (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    file_id uuid NOT NULL,
    provider_id uuid NOT NULL,
    provider_file_id character varying(512),
    provider_metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    upload_status character varying(50) DEFAULT 'pending'::character varying NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT llm_provider_files_upload_status_check CHECK ((upload_status IN ('pending', 'uploading', 'completed', 'failed', 'expired')))
);

ALTER TABLE ONLY public.llm_provider_files
    ADD CONSTRAINT llm_provider_files_file_id_provider_id_key UNIQUE (file_id, provider_id);

ALTER TABLE ONLY public.llm_provider_files
    ADD CONSTRAINT llm_provider_files_pkey PRIMARY KEY (id);

CREATE INDEX idx_llm_provider_files_expires_at ON public.llm_provider_files USING btree (((provider_metadata ->> 'expires_at'::text))) WHERE ((provider_metadata ->> 'expires_at'::text) IS NOT NULL);

CREATE INDEX idx_llm_provider_files_file_id ON public.llm_provider_files USING btree (file_id);

CREATE INDEX idx_llm_provider_files_metadata ON public.llm_provider_files USING gin (provider_metadata);

CREATE INDEX idx_llm_provider_files_provider_id ON public.llm_provider_files USING btree (provider_id);

CREATE INDEX idx_llm_provider_files_status ON public.llm_provider_files USING btree (upload_status);

CREATE TRIGGER update_llm_provider_files_updated_at BEFORE UPDATE ON public.llm_provider_files FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
