-- Extend the singleton resource-limits row with VM-sizing knobs (Plan 1 §6,
-- MAC-TODO closure). Three new columns replace the compile-time const trio
-- in `code_sandbox/backend/mac_vm.rs` + the matching const in
-- `code_sandbox/backend/wsl2.rs`. Defaults match the prior literals so a
-- fresh install behaves identically to before this migration.

ALTER TABLE code_sandbox_settings
    -- macOS libkrun microVM sizing. The launcher passes these directly into
    -- `krun_set_vm_config` (`num_vcpus`, `ram_mib`). The bounds are deliberate
    -- belt-and-suspenders: well below any sane hardware ceiling but high
    -- enough that an admin can tune for parallel-R / torch workloads.
    ADD COLUMN mac_vm_vcpus              INTEGER NOT NULL DEFAULT 2,
    ADD COLUMN mac_vm_ram_mib            INTEGER NOT NULL DEFAULT 2048,
    -- Cross-platform: cap on concurrent `execute_command`s sharing a single
    -- VM/distro. Bounds the per-VM RAM pressure even when individual execs
    -- are cgroup-capped (N execs × cgroup memory.max ≤ VM RAM ceiling).
    -- Applies to both the macOS libkrun VM and the Windows WSL2 distro.
    ADD COLUMN vm_max_concurrent_execs   INTEGER NOT NULL DEFAULT 3,
    ADD CONSTRAINT mac_vm_vcpus_range          CHECK (mac_vm_vcpus            BETWEEN 1 AND 128),
    ADD CONSTRAINT mac_vm_ram_mib_range        CHECK (mac_vm_ram_mib          BETWEEN 256 AND 262144),
    ADD CONSTRAINT vm_max_concurrent_execs_range
        CHECK (vm_max_concurrent_execs BETWEEN 1 AND 1000);

COMMENT ON COLUMN code_sandbox_settings.mac_vm_vcpus IS
    'macOS libkrun microVM vCPU count (krun_set_vm_config). Replaces the VM_VCPUS const in mac_vm.rs.';
COMMENT ON COLUMN code_sandbox_settings.mac_vm_ram_mib IS
    'macOS libkrun microVM RAM ceiling in MiB (krun_set_vm_config). Replaces VM_RAM_MIB.';
COMMENT ON COLUMN code_sandbox_settings.vm_max_concurrent_execs IS
    'Per-VM concurrent execute_command cap (macOS + WSL2). Replaces MAX_CONCURRENT_EXECS_PER_VM.';
