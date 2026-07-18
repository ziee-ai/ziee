-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- llm_repository seed data.

INSERT INTO public.llm_repositories VALUES ('cf7e2e2a-4d93-440f-90a7-8131e5a103ea', 'Hugging Face Hub', 'https://huggingface.co', 'api_key', '{"api_key": "", "auth_test_api_endpoint": "https://huggingface.co/api/whoami-v2"}', true, true, '2026-07-14 15:09:55.401746+00', '2026-07-14 15:09:55.401746+00', NULL, NULL, 'untested', NULL);
INSERT INTO public.llm_repositories VALUES ('8c64021b-c9db-49c8-b976-a979a312ec1a', 'GitHub', 'https://github.com', 'bearer_token', '{"token": "", "auth_test_api_endpoint": "https://api.github.com/user"}', true, true, '2026-07-14 15:09:55.401746+00', '2026-07-14 15:09:55.401746+00', NULL, NULL, 'untested', NULL);
