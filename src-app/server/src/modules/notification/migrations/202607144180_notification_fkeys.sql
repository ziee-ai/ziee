-- App-side notification foreign key (deferred).
--
-- The SDK crate migration (ziee-notification/migrations/*_notification_schema.sql)
-- is domain-agnostic and FK-FREE so it applies standalone on a bare DB. ziee DOES
-- have a `users` table, so — per the crate's documented contract — it adds the
-- `user_id` FK here, sorted after BOTH the `notifications` table and the `users`
-- table exist.
--
-- The former domain FKs (conversation_id / scheduled_task_id / workflow_run_id)
-- were DROPPED in the R2 payload adoption: those columns no longer exist (the
-- kind-specific ids now ride the `payload jsonb` column with NO referential
-- integrity to chat/scheduler/workflow), so only the `user_id` cascade remains.

ALTER TABLE ONLY public.notifications
    ADD CONSTRAINT notifications_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;
