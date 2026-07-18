-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- citations foreign keys (deferred).

ALTER TABLE ONLY public.bibliography_entries
    ADD CONSTRAINT bibliography_entries_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.project_bibliography
    ADD CONSTRAINT project_bibliography_entry_id_fkey FOREIGN KEY (entry_id) REFERENCES public.bibliography_entries(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.project_bibliography
    ADD CONSTRAINT project_bibliography_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;
