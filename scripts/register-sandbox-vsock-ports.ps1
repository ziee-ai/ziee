<#
.SYNOPSIS
    Register the AF_HYPERV vsock service IDs the WSL2 code sandbox uses.

.DESCRIPTION
    Despite Microsoft's documentation suggesting the `HV_GUID_VSOCK_TEMPLATE`
    family is auto-routable from the Windows host to a WSL2 distro's
    AF_VSOCK listener, in practice connections to port-templated GUIDs
    time out with WSA 10060 unless the GUID is registered under
    `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Virtualization\GuestCommunicationServices\<GUID>`.

    The vmcompute service reads the registry at VM-start time, so adding
    new GUIDs requires `wsl --shutdown` (executed at the end of this
    script) before the next WSL VM boot picks them up.

    The code sandbox allocates AF_VSOCK ports from a sliding pool that
    starts at 10001. This script pre-registers a generous range so the
    server can use any of them without further admin operations.

    Must be run as Administrator (HKLM write requires it).

.PARAMETER PortStart
    First port to register. Default 10001 (matches NEXT_VSOCK_PORT in
    src-app/server/src/modules/code_sandbox/backend/wsl2.rs).

.PARAMETER Count
    How many ports to register. Default 100.

.PARAMETER NoShutdown
    Skip the `wsl --shutdown` at the end. Use if you'll restart WSL
    manually. Default: shutdown executed automatically.

.EXAMPLE
    # Default - register 10001..10100, shut down WSL so the next boot
    # picks up the new registrations.
    .\register-sandbox-vsock-ports.ps1

.EXAMPLE
    # Register a custom range.
    .\register-sandbox-vsock-ports.ps1 -PortStart 20000 -Count 50

.NOTES
    The registry write requires Administrator. `wsl --shutdown` itself
    is user-level (no admin needed) and terminates ALL running distros
    on the user's WSL host - save work in any open distro shell first.
#>

[CmdletBinding()]
param(
    [int]$PortStart = 10001,
    [int]$Count = 100,
    [switch]$NoShutdown
)

$ErrorActionPreference = 'Stop'

# Admin check.
$isAdmin = ([Security.Principal.WindowsPrincipal](
    [Security.Principal.WindowsIdentity]::GetCurrent()
)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Error "Must run as Administrator. Right-click PowerShell -> Run as administrator, then re-run."
    exit 2
}

$base = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Virtualization\GuestCommunicationServices'
if (-not (Test-Path $base)) {
    Write-Error "$base not present. Is Hyper-V / WSL2 installed?"
    exit 2
}

$registered = 0
$alreadyPresent = 0

for ($p = $PortStart; $p -lt $PortStart + $Count; $p++) {
    # HV_GUID_VSOCK_TEMPLATE with data1 = port.
    $guid = ('{0:X8}-facb-11e6-bd58-64006a7986d3' -f $p).ToLower()
    $key = Join-Path $base $guid
    if (Test-Path $key) {
        $alreadyPresent++
        continue
    }
    New-Item -Path $key -Force | Out-Null
    New-ItemProperty -Path $key -Name ElementName `
        -Value "ziee-sandbox-vsock-$p" -PropertyType String -Force | Out-Null
    $registered++
}

Write-Host "Registered $registered new port(s); $alreadyPresent already present."
Write-Host "  Range: $PortStart .. $($PortStart + $Count - 1)"

if ($registered -eq 0) {
    Write-Host "No changes - vmcompute already has these registrations cached. No restart needed."
    exit 0
}

if ($NoShutdown) {
    Write-Host ""
    Write-Host 'NEW registrations require a `wsl --shutdown` before they take effect.'
    Write-Host 'Run: wsl --shutdown'
    exit 0
}

Write-Host ""
Write-Host "Restarting WSL so vmcompute picks up the new registrations..."
wsl --shutdown
Write-Host 'Done. The next wsl invocation will boot a fresh utility VM that sees the new ports.'
