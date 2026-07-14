-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- skill foreign keys (deferred).

ALTER TABLE ONLY public.conversation_skill_overrides
    ADD CONSTRAINT conversation_skill_overrides_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.conversation_skill_overrides
    ADD CONSTRAINT conversation_skill_overrides_skill_id_fkey FOREIGN KEY (skill_id) REFERENCES public.skills(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.group_skills
    ADD CONSTRAINT group_skills_group_id_fkey FOREIGN KEY (group_id) REFERENCES public.groups(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.group_skills
    ADD CONSTRAINT group_skills_skill_id_fkey FOREIGN KEY (skill_id) REFERENCES public.skills(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.skills
    ADD CONSTRAINT skills_created_by_fkey FOREIGN KEY (created_by) REFERENCES public.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.skills
    ADD CONSTRAINT skills_owner_user_id_fkey FOREIGN KEY (owner_user_id) REFERENCES public.users(id) ON DELETE CASCADE;
