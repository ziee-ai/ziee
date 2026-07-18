-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- hub module tables + indexes + triggers.

CREATE TABLE public.hub_entities (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    entity_type character varying(50) NOT NULL,
    entity_id uuid NOT NULL,
    hub_id character varying(255) NOT NULL,
    hub_category character varying(50) NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by uuid,
    hub_version character varying(32),
    CONSTRAINT valid_entity_type CHECK ((entity_type IN ('assistant', 'mcp_server', 'llm_model', 'skill', 'workflow'))),
    CONSTRAINT valid_hub_category CHECK ((hub_category IN ('assistant', 'mcp_server', 'model', 'skill', 'workflow')))
);

CREATE TABLE public.hub_settings (
    id boolean DEFAULT true NOT NULL,
    pinned_version character varying(32),
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT hub_settings_id_check CHECK ((id = true))
);

ALTER TABLE ONLY public.hub_entities
    ADD CONSTRAINT hub_entities_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.hub_settings
    ADD CONSTRAINT hub_settings_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.hub_entities
    ADD CONSTRAINT unique_entity_hub_tracking UNIQUE (entity_type, entity_id);

CREATE INDEX idx_hub_entities_hub_id ON public.hub_entities USING btree (hub_id, entity_type);

CREATE INDEX idx_hub_entities_lookup ON public.hub_entities USING btree (entity_type, entity_id);

CREATE INDEX idx_hub_entities_user ON public.hub_entities USING btree (created_by) WHERE (created_by IS NOT NULL);

CREATE UNIQUE INDEX uniq_hub_system_mcp_install ON public.hub_entities USING btree (hub_id) WHERE (((entity_type)::text = 'mcp_server'::text) AND (created_by IS NULL));

CREATE UNIQUE INDEX uniq_hub_template_install ON public.hub_entities USING btree (hub_id) WHERE (((entity_type)::text = 'assistant'::text) AND (created_by IS NULL));
