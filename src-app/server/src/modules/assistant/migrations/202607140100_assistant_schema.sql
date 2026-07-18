-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- assistant module tables + indexes + triggers.

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

ALTER TABLE ONLY public.assistants
    ADD CONSTRAINT assistants_pkey PRIMARY KEY (id);

CREATE INDEX idx_assistants_created_by ON public.assistants USING btree (created_by);

CREATE INDEX idx_assistants_default_lookup ON public.assistants USING btree (created_by) WHERE ((is_default = true) AND (enabled = true));

CREATE INDEX idx_assistants_enabled ON public.assistants USING btree (enabled);

CREATE INDEX idx_assistants_is_default ON public.assistants USING btree (is_default);

CREATE INDEX idx_assistants_is_template ON public.assistants USING btree (is_template);

CREATE INDEX idx_assistants_name ON public.assistants USING btree (name);

CREATE TRIGGER update_assistants_updated_at BEFORE UPDATE ON public.assistants FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
