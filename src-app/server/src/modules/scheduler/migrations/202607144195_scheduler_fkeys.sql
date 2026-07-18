-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- scheduler foreign keys (deferred).

ALTER TABLE ONLY public.scheduled_task_runs
    ADD CONSTRAINT scheduled_task_runs_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.scheduled_task_runs
    ADD CONSTRAINT scheduled_task_runs_notification_id_fkey FOREIGN KEY (notification_id) REFERENCES public.notifications(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.scheduled_task_runs
    ADD CONSTRAINT scheduled_task_runs_scheduled_task_id_fkey FOREIGN KEY (scheduled_task_id) REFERENCES public.scheduled_tasks(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.scheduled_task_runs
    ADD CONSTRAINT scheduled_task_runs_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.scheduled_task_runs
    ADD CONSTRAINT scheduled_task_runs_workflow_run_id_fkey FOREIGN KEY (workflow_run_id) REFERENCES public.workflow_runs(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.scheduled_tasks
    ADD CONSTRAINT scheduled_tasks_assistant_id_fkey FOREIGN KEY (assistant_id) REFERENCES public.assistants(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.scheduled_tasks
    ADD CONSTRAINT scheduled_tasks_bound_conversation_id_fkey FOREIGN KEY (bound_conversation_id) REFERENCES public.conversations(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.scheduled_tasks
    ADD CONSTRAINT scheduled_tasks_model_id_fkey FOREIGN KEY (model_id) REFERENCES public.llm_models(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.scheduled_tasks
    ADD CONSTRAINT scheduled_tasks_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.scheduled_tasks
    ADD CONSTRAINT scheduled_tasks_workflow_id_fkey FOREIGN KEY (workflow_id) REFERENCES public.workflows(id) ON DELETE SET NULL;
