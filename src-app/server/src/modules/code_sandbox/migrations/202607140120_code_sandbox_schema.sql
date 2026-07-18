-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- code_sandbox module tables + indexes + triggers.

CREATE TABLE public.code_sandbox_rootfs_artifacts (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    version text NOT NULL,
    arch text NOT NULL,
    flavor text NOT NULL,
    package text NOT NULL,
    sha256 text NOT NULL,
    artifact_path text NOT NULL,
    cosign_bundle text,
    status text DEFAULT 'installed'::text NOT NULL,
    downloaded_at timestamp with time zone DEFAULT now() NOT NULL,
    last_used_at timestamp with time zone
);

CREATE TABLE public.code_sandbox_settings (
    id boolean DEFAULT true NOT NULL,
    memory_max_bytes bigint DEFAULT 536870912 NOT NULL,
    memory_swap_max_bytes bigint DEFAULT 0 NOT NULL,
    pids_max integer DEFAULT 256 NOT NULL,
    cpu_max text DEFAULT '100000 100000'::text NOT NULL,
    address_space_bytes bigint DEFAULT '4294967296'::bigint NOT NULL,
    fsize_bytes bigint DEFAULT 268435456 NOT NULL,
    nproc_max integer DEFAULT 256 NOT NULL,
    nofile_max integer DEFAULT 1024 NOT NULL,
    cpu_secs_max integer DEFAULT 1240 NOT NULL,
    timeout_secs integer DEFAULT 620 NOT NULL,
    vm_idle_evict_secs integer DEFAULT 900 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    mac_vm_vcpus integer DEFAULT 2 NOT NULL,
    mac_vm_ram_mib integer DEFAULT 2048 NOT NULL,
    vm_max_concurrent_execs integer DEFAULT 3 NOT NULL,
    current_rootfs_version text,
    CONSTRAINT address_space_bytes_positive CHECK ((address_space_bytes >= 16777216)),
    CONSTRAINT code_sandbox_settings_id_check CHECK ((id = true)),
    CONSTRAINT cpu_max_shape CHECK ((cpu_max ~ '^[0-9]+ [0-9]+$'::text)),
    CONSTRAINT cpu_secs_max_positive CHECK (((cpu_secs_max >= 10) AND (cpu_secs_max <= 86400))),
    CONSTRAINT fsize_bytes_positive CHECK ((fsize_bytes >= 1048576)),
    CONSTRAINT mac_vm_ram_mib_range CHECK (((mac_vm_ram_mib >= 256) AND (mac_vm_ram_mib <= 262144))),
    CONSTRAINT mac_vm_vcpus_range CHECK (((mac_vm_vcpus >= 1) AND (mac_vm_vcpus <= 128))),
    CONSTRAINT memory_max_bytes_positive CHECK ((memory_max_bytes >= 16777216)),
    CONSTRAINT memory_swap_max_bytes_nonneg CHECK ((memory_swap_max_bytes >= 0)),
    CONSTRAINT nofile_max_positive CHECK (((nofile_max >= 64) AND (nofile_max <= 1048576))),
    CONSTRAINT nproc_max_positive CHECK (((nproc_max >= 8) AND (nproc_max <= 100000))),
    CONSTRAINT pids_max_positive CHECK (((pids_max >= 8) AND (pids_max <= 100000))),
    CONSTRAINT timeout_secs_positive CHECK (((timeout_secs >= 5) AND (timeout_secs <= 86400))),
    CONSTRAINT vm_idle_evict_secs_nonneg CHECK ((vm_idle_evict_secs >= 0)),
    CONSTRAINT vm_max_concurrent_execs_range CHECK (((vm_max_concurrent_execs >= 1) AND (vm_max_concurrent_execs <= 1000)))
);

CREATE TABLE public.sandbox_workspace_files (
    conversation_id uuid NOT NULL,
    workspace_relpath text NOT NULL,
    file_id uuid NOT NULL,
    base_version_id uuid NOT NULL
);

ALTER TABLE ONLY public.code_sandbox_rootfs_artifacts
    ADD CONSTRAINT code_sandbox_rootfs_artifacts_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.code_sandbox_rootfs_artifacts
    ADD CONSTRAINT code_sandbox_rootfs_artifacts_version_arch_flavor_package_key UNIQUE (version, arch, flavor, package);

ALTER TABLE ONLY public.code_sandbox_settings
    ADD CONSTRAINT code_sandbox_settings_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.sandbox_workspace_files
    ADD CONSTRAINT sandbox_workspace_files_pkey PRIMARY KEY (conversation_id, workspace_relpath);

CREATE INDEX idx_code_sandbox_rootfs_artifacts_arch_flavor ON public.code_sandbox_rootfs_artifacts USING btree (arch, flavor);

CREATE INDEX idx_code_sandbox_rootfs_artifacts_version ON public.code_sandbox_rootfs_artifacts USING btree (version);

CREATE INDEX idx_sandbox_workspace_files_file ON public.sandbox_workspace_files USING btree (file_id);
