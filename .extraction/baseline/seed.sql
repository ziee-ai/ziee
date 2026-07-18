--
-- PostgreSQL database dump
--

\restrict Uqq1cvXBYQLP58clKgpOod6RtX5Y8Lww0t22T0JP1oLLXeSsagqVYjhZOWTd1tH

-- Dumped from database version 18.4 (Debian 18.4-1.pgdg12+1)
-- Dumped by pg_dump version 18.3

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET transaction_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Data for Name: users; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: assistants; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.assistants VALUES ('9a4be99b-931a-4e99-91aa-e96c2b41a244', 'Default Assistant', 'General-purpose AI assistant for everyday tasks', 'You are a helpful, harmless, and honest AI assistant. Provide clear, accurate, and concise responses to user queries. If you are unsure about something, say so rather than making up information.', '{"top_p": 0.9, "max_tokens": 2048, "temperature": 0.7}', NULL, true, true, true, '2026-07-14 15:09:55.599197+00', '2026-07-14 15:09:55.599197+00');


--
-- Data for Name: assistant_core_memory; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: auth_providers; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.auth_providers VALUES ('92e74a99-ffa4-4c49-a05f-4a2b3e2b2efe', 'google', 'oidc', false, '{"scopes": ["openid", "email", "profile"], "client_id": "", "issuer_url": "https://accounts.google.com", "display_name": "Sign in with Google", "client_secret": "", "attribute_mapping": {"email": "email", "user_id": "sub", "username": "email", "last_name": "family_name", "first_name": "given_name", "display_name": "name"}}', '2026-07-14 15:09:57.221494+00', '2026-07-14 15:09:57.221494+00', NULL, NULL, NULL, NULL);
INSERT INTO public.auth_providers VALUES ('824eb523-bda9-40dd-8a06-b71c5e1ced97', 'microsoft', 'oidc', false, '{"scopes": ["openid", "email", "profile"], "client_id": "", "issuer_url": "https://login.microsoftonline.com/common/v2.0", "display_name": "Sign in with Microsoft", "client_secret": "", "attribute_mapping": {"email": "email", "user_id": "sub", "username": "preferred_username", "display_name": "name"}, "allowed_tenant_ids": []}', '2026-07-14 15:09:57.221494+00', '2026-07-14 15:09:57.221494+00', NULL, NULL, NULL, NULL);
INSERT INTO public.auth_providers VALUES ('a159dee0-2977-4f8d-a8e5-68dc1a398c32', 'apple', 'apple', false, '{"key_id": "", "scopes": ["name", "email"], "team_id": "", "services_id": "", "private_key_path": ""}', '2026-07-14 15:09:57.221494+00', '2026-07-14 15:09:57.221494+00', NULL, NULL, NULL, NULL);


--
-- Data for Name: bibliography_entries; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: llm_runtime_versions; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: llm_providers; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.llm_providers VALUES ('79babd92-30fd-487f-b791-8c62708e7d6a', 'OpenAI', 'openai', false, NULL, 'https://api.openai.com/v1', true, '{}', '2026-07-14 15:09:55.450123+00', '2026-07-14 15:09:55.450123+00', '{"type": "local", "binary_path": null}', NULL, NULL);
INSERT INTO public.llm_providers VALUES ('06bda3ec-ad16-43e9-8220-c26206ccdf38', 'Anthropic', 'anthropic', false, NULL, 'https://api.anthropic.com/v1', true, '{}', '2026-07-14 15:09:55.450123+00', '2026-07-14 15:09:55.450123+00', '{"type": "local", "binary_path": null}', NULL, NULL);
INSERT INTO public.llm_providers VALUES ('29f92657-1082-4685-8885-b57635c5c8fa', 'Groq', 'groq', false, NULL, 'https://api.groq.com/openai/v1', true, '{}', '2026-07-14 15:09:55.450123+00', '2026-07-14 15:09:55.450123+00', '{"type": "local", "binary_path": null}', NULL, NULL);
INSERT INTO public.llm_providers VALUES ('4d5e2537-c03c-44d6-861a-0f7a76217c01', 'Google Gemini', 'gemini', false, NULL, 'https://generativelanguage.googleapis.com/v1beta', true, '{}', '2026-07-14 15:09:55.450123+00', '2026-07-14 15:09:55.450123+00', '{"type": "local", "binary_path": null}', NULL, NULL);
INSERT INTO public.llm_providers VALUES ('f198a396-1dc6-4766-b6a4-3aed4c938002', 'Mistral AI', 'mistral', false, NULL, 'https://api.mistral.ai/v1', true, '{}', '2026-07-14 15:09:55.450123+00', '2026-07-14 15:09:55.450123+00', '{"type": "local", "binary_path": null}', NULL, NULL);
INSERT INTO public.llm_providers VALUES ('d525f74b-8adc-495d-bbfb-8b1d42580da7', 'DeepSeek', 'deepseek', false, NULL, 'https://api.deepseek.com', true, '{}', '2026-07-14 15:09:55.450123+00', '2026-07-14 15:09:55.450123+00', '{"type": "local", "binary_path": null}', NULL, NULL);
INSERT INTO public.llm_providers VALUES ('18abd00d-e2d9-40b0-8e07-9f018ca3a8d5', 'Local', 'local', false, NULL, 'http://localhost:8080/v1', true, '{}', '2026-07-14 15:09:55.450123+00', '2026-07-14 15:09:55.450123+00', '{"type": "local", "binary_path": null}', NULL, NULL);
INSERT INTO public.llm_providers VALUES ('25258ca9-b3a0-4f4d-8b61-cb2b9010d9d6', 'OpenRouter', 'openrouter', false, NULL, 'https://openrouter.ai/api/v1', true, '{}', '2026-07-14 15:10:00.863533+00', '2026-07-14 15:10:00.863533+00', '{"type": "local", "binary_path": null}', NULL, NULL);


--
-- Data for Name: llm_models; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: conversations; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: branches; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: messages; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: branch_messages; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: code_sandbox_rootfs_artifacts; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: code_sandbox_settings; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.code_sandbox_settings VALUES (true, 536870912, 0, 256, '100000 100000', 4294967296, 268435456, 256, 1024, 1240, 620, 900, '2026-07-14 15:09:56.930187+00', '2026-07-14 15:09:56.930187+00', 2, 2048, 3, NULL);


--
-- Data for Name: workflows; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: workflow_runs; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: files; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: conversation_deliverables; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: knowledge_bases; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: conversation_knowledge_bases; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: conversation_memory_settings; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: skills; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: conversation_skill_overrides; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: conversation_summaries; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: conversation_summarization_settings; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: llm_repositories; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.llm_repositories VALUES ('cf7e2e2a-4d93-440f-90a7-8131e5a103ea', 'Hugging Face Hub', 'https://huggingface.co', 'api_key', '{"api_key": "", "auth_test_api_endpoint": "https://huggingface.co/api/whoami-v2"}', true, true, '2026-07-14 15:09:55.401746+00', '2026-07-14 15:09:55.401746+00', NULL, NULL, 'untested', NULL);
INSERT INTO public.llm_repositories VALUES ('8c64021b-c9db-49c8-b976-a979a312ec1a', 'GitHub', 'https://github.com', 'bearer_token', '{"token": "", "auth_test_api_endpoint": "https://api.github.com/user"}', true, true, '2026-07-14 15:09:55.401746+00', '2026-07-14 15:09:55.401746+00', NULL, NULL, 'untested', NULL);


--
-- Data for Name: download_instances; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: file_chunks; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: file_index_state; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: file_rag_admin_settings; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.file_rag_admin_settings VALUES (1, true, NULL, 768, 1200, 200, 5000, 8, 0.6, true, true, 'simple', 60, 4, 0, '2026-07-14 15:09:59.423393+00', NULL, false, 30, 2000, 2000, 160, 50);


--
-- Data for Name: file_versions; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: groups; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.groups VALUES ('f3a2b018-a0eb-45c1-98f9-a616599c9ee4', 'Administrators', 'System administrators with full access to all features', '{*,projects::create,projects::read,projects::edit,projects::delete,mcp_user_policy::edit}', true, true, false, '2026-07-14 15:09:55.356077+00', '2026-07-14 15:09:58.777409+00');
INSERT INTO public.groups VALUES ('79668b78-16ff-4d3f-80e6-afdbb3236dc9', 'Users', 'Default group for all users', '{profile::read,profile::edit,chat::read,chat::create,conversations::create,conversations::read,conversations::edit,conversations::delete,messages::create,messages::read,messages::delete,branches::create,branches::switch,assistants::create,assistants::read,assistants::edit,assistants::delete,mcp_servers::read,mcp_servers::create,mcp_servers::edit,mcp_servers::delete,hub::assistants::read,hub::assistants::read_version,hub::assistants::create,hub::mcp_servers::read,hub::mcp_servers::read_version,hub::mcp_servers::create,files::read,files::upload,files::download,files::delete,files::preview,files::generate_token,user_llm_providers::read,code_sandbox::execute,llm_models::downloads_read,llm_models::downloads_cancel,llm_models::downloads_delete,memory::read,memory::write,memory::core::read,memory::core::write,bio::query,web_search::use,lit_search::use,citations::use,citations::manage,workflows::read,workflows::execute,control::use,js_tool::use,scheduler::use,notifications::read,knowledge_base::use,knowledge_base::manage,voice::transcribe}', true, true, true, '2026-07-14 15:09:55.356917+00', '2026-07-14 15:10:01.610531+00');


--
-- Data for Name: group_skills; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: group_workflows; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: hub_entities; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: hub_settings; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.hub_settings VALUES (true, NULL, '2026-07-14 15:09:58.129427+00');


--
-- Data for Name: js_tool_settings; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.js_tool_settings VALUES (true, 134217728, 524288, 300, 300, 8, 6, 256, '2026-07-14 15:10:00.98768+00', '2026-07-14 15:10:00.98768+00');


--
-- Data for Name: knowledge_base_documents; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: lit_fulltext_cache; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: lit_search_connectors; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: lit_search_settings; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.lit_search_settings VALUES (true, true, '{europepmc,crossref,semanticscholar,pubmed,arxiv}', 25, 50, 30, true, '2026-07-14 15:09:59.468118+00', '2026-07-14 15:09:59.468118+00');


--
-- Data for Name: llm_model_files; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: llm_provider_files; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: llm_runtime_instances; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: llm_runtime_settings; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.llm_runtime_settings VALUES (true, 1800, 30, 30, '2026-07-14 15:09:58.009071+00', '2026-07-14 15:09:58.009071+00');


--
-- Data for Name: mcp_servers; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.mcp_servers VALUES ('865f06fa-c4e5-4eb3-9801-5804f67062c2', NULL, 'fetch', 'Web Fetch', 'Fetch content from web URLs', true, true, 'stdio', 'uvx', '["mcp-server-fetch"]', '{}', NULL, '{}', 30, '2026-07-14 15:09:55.65088+00', '2026-07-14 15:09:55.65088+00', false, 'auto', NULL, true, false, '{}', '{}', '{}', '{}', NULL, 'untested', NULL, 'full');


--
-- Data for Name: mcp_server_oauth_configs; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: projects; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: mcp_settings; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: mcp_tool_calls; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: mcp_user_policy; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.mcp_user_policy VALUES (1, '{http,stdio}', 'full', '2026-07-14 15:09:58.735031+00', NULL, 90);


--
-- Data for Name: memory_admin_settings; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.memory_admin_settings VALUES (1, NULL, 768, NULL, 8, 0.6, true, '2026-07-14 15:09:57.538587+00', 30, 200, 'simple', true, 60, 4, 0, NULL, NULL, true);


--
-- Data for Name: memory_audit_log; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: message_assistant; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: message_contents; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: message_mcp_servers; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: scheduled_tasks; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: notifications; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: oauth_sessions; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: pending_account_links; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: project_bibliography; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: project_conversations; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: project_files; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: project_knowledge_bases; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: refresh_tokens; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: sandbox_workspace_files; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: scheduled_task_runs; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: scheduler_admin_settings; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.scheduler_admin_settings VALUES (true, 20, 300, 5, 30, '2026-07-14 15:10:01.123198+00');


--
-- Data for Name: session_settings; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.session_settings VALUES (true, 24, 30, false, '2026-07-14 15:10:00.732948+00');


--
-- Data for Name: summarization_admin_settings; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.summarization_admin_settings VALUES (1, true, NULL, 12000, 3000, NULL, NULL, '2026-07-14 15:09:59.038044+00');


--
-- Data for Name: tool_use_approvals; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: user_auth_links; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: user_group_llm_providers; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: user_group_mcp_servers; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.user_group_mcp_servers VALUES ('79668b78-16ff-4d3f-80e6-afdbb3236dc9', '865f06fa-c4e5-4eb3-9801-5804f67062c2', '2026-07-14 15:09:55.651712+00');


--
-- Data for Name: user_groups; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: user_lit_search_connector_keys; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: user_llm_provider_api_keys; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: user_mcp_defaults; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: user_memories; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: user_memory_settings; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: user_onboarding; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: user_web_search_provider_keys; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: voice_models; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: voice_runtime_versions; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: voice_runtime_instance; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.voice_runtime_instance VALUES (true, NULL, NULL, NULL, NULL, 'stopped', 'stopped', '2026-07-14 15:10:01.567901+00', 0, NULL, NULL, '2026-07-14 15:10:01.567901+00', '2026-07-14 15:10:01.567901+00');


--
-- Data for Name: voice_runtime_settings; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.voice_runtime_settings VALUES (true, true, 'base', 'auto', 1800, 60, 30, 120, 33554432, '2026-07-14 15:10:01.569803+00', '2026-07-14 15:10:01.569803+00', true, 1000, 30, 'ggerganov/whisper.cpp');


--
-- Data for Name: web_search_providers; Type: TABLE DATA; Schema: public; Owner: -
--



--
-- Data for Name: web_search_settings; Type: TABLE DATA; Schema: public; Owner: -
--

INSERT INTO public.web_search_settings VALUES (true, true, '{searxng,brave}', 5, 5242880, 40000, 20, '2026-07-14 15:09:59.325581+00', '2026-07-14 15:09:59.325581+00');


--
-- Name: lit_fulltext_cache_id_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.lit_fulltext_cache_id_seq', 1, false);


--
-- Name: memory_audit_log_id_seq; Type: SEQUENCE SET; Schema: public; Owner: -
--

SELECT pg_catalog.setval('public.memory_audit_log_id_seq', 1, false);


--
-- PostgreSQL database dump complete
--

\unrestrict Uqq1cvXBYQLP58clKgpOod6RtX5Y8Lww0t22T0JP1oLLXeSsagqVYjhZOWTd1tH

