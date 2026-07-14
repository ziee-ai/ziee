-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- skill module tables + indexes + triggers.

CREATE FUNCTION public.enforce_system_scope_for_group_skills() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF (SELECT scope FROM skills WHERE id = NEW.skill_id) <> 'system' THEN
        RAISE EXCEPTION 'group_skills: only system-scope skills can be assigned to groups (skill_id=%)', NEW.skill_id;
    END IF;
    RETURN NEW;
END;
$$;

CREATE TABLE public.conversation_skill_overrides (
    conversation_id uuid NOT NULL,
    skill_id uuid NOT NULL,
    hidden boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.group_skills (
    group_id uuid NOT NULL,
    skill_id uuid NOT NULL,
    assigned_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.skills (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name text NOT NULL,
    version text,
    display_name text,
    description text,
    when_to_use text,
    extracted_path text NOT NULL,
    bundle_sha256 text NOT NULL,
    bundle_size_bytes bigint NOT NULL,
    file_count integer NOT NULL,
    entry_point text NOT NULL,
    frontmatter_json jsonb DEFAULT '{}'::jsonb NOT NULL,
    tags jsonb DEFAULT '[]'::jsonb NOT NULL,
    scope character varying(10) DEFAULT 'user'::character varying NOT NULL,
    owner_user_id uuid,
    created_by uuid,
    enabled boolean DEFAULT true NOT NULL,
    is_dev boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT skills_scope_check CHECK ((scope IN ('user', 'system', 'built_in'))),
    CONSTRAINT skills_scope_owner_check CHECK (((((scope)::text = 'user'::text) AND (owner_user_id IS NOT NULL)) OR ((scope IN ('system', 'built_in')) AND (owner_user_id IS NULL))))
);

ALTER TABLE ONLY public.conversation_skill_overrides
    ADD CONSTRAINT conversation_skill_overrides_pkey PRIMARY KEY (conversation_id, skill_id);

ALTER TABLE ONLY public.group_skills
    ADD CONSTRAINT group_skills_pkey PRIMARY KEY (group_id, skill_id);

ALTER TABLE ONLY public.skills
    ADD CONSTRAINT skills_pkey PRIMARY KEY (id);

CREATE INDEX idx_conversation_skill_overrides_conv ON public.conversation_skill_overrides USING btree (conversation_id);

CREATE INDEX idx_group_skills_skill ON public.group_skills USING btree (skill_id);

CREATE INDEX idx_skills_enabled ON public.skills USING btree (enabled) WHERE (enabled = true);

CREATE INDEX idx_skills_name ON public.skills USING btree (name);

CREATE INDEX idx_skills_owner ON public.skills USING btree (owner_user_id) WHERE ((scope)::text = 'user'::text);

CREATE UNIQUE INDEX uniq_skills_builtin_name ON public.skills USING btree (name) WHERE ((scope)::text = 'built_in'::text);

CREATE UNIQUE INDEX uniq_skills_system_name_version ON public.skills USING btree (name, version) WHERE ((scope)::text = 'system'::text);

CREATE UNIQUE INDEX uniq_skills_user_name_version_owner ON public.skills USING btree (name, version, owner_user_id) WHERE ((scope)::text = 'user'::text);

CREATE TRIGGER group_skills_scope_check BEFORE INSERT OR UPDATE ON public.group_skills FOR EACH ROW EXECUTE FUNCTION public.enforce_system_scope_for_group_skills();
