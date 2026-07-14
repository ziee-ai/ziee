-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- citations module tables + indexes + triggers.

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

CREATE TABLE public.project_bibliography (
    project_id uuid NOT NULL,
    entry_id uuid NOT NULL,
    added_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.bibliography_entries
    ADD CONSTRAINT bibliography_entries_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.project_bibliography
    ADD CONSTRAINT project_bibliography_pkey PRIMARY KEY (project_id, entry_id);

CREATE INDEX idx_bibliography_tsv ON public.bibliography_entries USING gin (content_tsv);

CREATE INDEX idx_bibliography_user ON public.bibliography_entries USING btree (user_id);

CREATE INDEX idx_project_bibliography_entry_id ON public.project_bibliography USING btree (entry_id);

CREATE UNIQUE INDEX uq_bibliography_user_citation_key ON public.bibliography_entries USING btree (user_id, citation_key);

CREATE UNIQUE INDEX uq_bibliography_user_doi ON public.bibliography_entries USING btree (user_id, lower(doi)) WHERE (doi IS NOT NULL);

CREATE UNIQUE INDEX uq_bibliography_user_fingerprint ON public.bibliography_entries USING btree (user_id, dedup_fingerprint) WHERE ((doi IS NULL) AND (pmid IS NULL) AND (dedup_fingerprint IS NOT NULL));

CREATE UNIQUE INDEX uq_bibliography_user_pmid ON public.bibliography_entries USING btree (user_id, pmid) WHERE (pmid IS NOT NULL);
