# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-79 (OD-K6-3) - Windows counterpart of v2/tools/repro-build.sh (second machine psvn).
# Same measures as the POSIX script, expressed for Windows paths:
#   * -C strip=symbols                    (drop .symtab/.strtab; .text digest still readable)
#   * CARGO_INCREMENTAL=0, SOURCE_DATE_EPOCH=0
#   * toolchain pinned by rust-toolchain.toml (1.100.0)
#   * absolute source paths -> fixed virtual paths, via one --remap-path-prefix per directory
#     that contains a .rs file. Windows paths use '\', rustc stores the string verbatim in
#     panic locations (file!()), so each directory prefix is remapped to the canonical '/'
#     form. This produces exactly the same strings as the POSIX root remap:
#       v2/lib/<crate>/src/lib.rs  ->  /vifast/v2/lib/<crate>/src/lib.rs
#       vifast-core/src/lib.rs     ->  vifast-core/src/lib.rs   (relative, workspace member)
# No GNU toolchain needed: rustc 1.100.0 + riscv64gc target use the bundled rust-lld.
# ASCII-only on purpose: Windows PowerShell 5.1 reads .ps1 as ANSI unless a BOM is present.
#
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File v2\tools\repro-build-windows.ps1 `
#            [-Root C:\path\to\tree] [-Break] [-Quiet]
param(
    [string]$Root = "",
    [switch]$Break,
    [switch]$Quiet,
    [string]$Bin = "vifast-kbin-riscv64",
    [string]$Package = "vifast-kbin-riscv64"
)
$ErrorActionPreference = "Continue"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
if ([string]::IsNullOrWhiteSpace($Root)) { $Root = (Resolve-Path "$here\..\..").Path }
$Root = (Resolve-Path $Root).Path
$kernel = Join-Path $Root "v2\kernel"
$manifest = Join-Path $kernel "Cargo.toml"
if (-not (Test-Path $manifest)) { Write-Error "repro-build-windows: missing $manifest"; exit 2 }
if (-not (Test-Path (Join-Path $Root "v2\kernel\rust-toolchain.toml"))) {
    Write-Error "repro-build-windows: missing v2\kernel\rust-toolchain.toml (toolchain pin of the OS line)"; exit 2
}

$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
$env:PATH = "$cargoBin;" + $env:PATH
$cargo = Join-Path $cargoBin "cargo.exe"
$rustc = Join-Path $cargoBin "rustc.exe"

# cwd inside the tree; toolchain is pinned EXPLICITLY (A1-174: no pin file at repo root, so it
# cannot leak into crates/ or viscript/ of other lines).
Set-Location $Root
$env:RUSTUP_TOOLCHAIN = "1.100.0"

$flags = New-Object System.Collections.Generic.List[string]
$remapCount = 0
if (-not $Break) {
    $dirs = @()
    foreach ($base in @($kernel, (Join-Path $Root "v2\lib"))) {
        if (Test-Path $base) {
            $dirs += Get-ChildItem -Recurse -Filter *.rs -File $base | ForEach-Object { $_.DirectoryName }
        }
    }
    $dirs = $dirs | Sort-Object -Unique
    foreach ($d in $dirs) {
        $relRepo = $d.Substring($Root.Length).TrimStart('\')
        if ($relRepo -like "v2\lib\*") {
            # out of the kernel workspace -> cargo passes an ABSOLUTE path
            $flags.Add("--remap-path-prefix=$d\=/vifast/$($relRepo -replace '\\', '/')/")
            $remapCount++
        }
        if ($d.StartsWith($kernel)) {
            $relKernel = $d.Substring($kernel.Length).TrimStart('\')
            # workspace member -> cargo passes a path RELATIVE to v2/kernel
            if ($relKernel -ne "" -and $relKernel -notlike "target*") {
                $flags.Add("--remap-path-prefix=$relKernel\=$($relKernel -replace '\\', '/')/")
                $remapCount++
            }
        }
    }
}
$flags.Add("-C"); $flags.Add("strip=symbols")
$env:RUSTFLAGS = ($flags -join " ")
if ($Break) { $env:VIFAST_REPRO_BREAK = "1" } else { Remove-Item Env:VIFAST_REPRO_BREAK -ErrorAction SilentlyContinue }
$env:CARGO_INCREMENTAL = "0"
$env:SOURCE_DATE_EPOCH = "0"

if (-not $Quiet) {
    Write-Output ("# repro-build-windows: host=" + $env:COMPUTERNAME + " os=" + [System.Environment]::OSVersion.VersionString)
    Write-Output ("# repro-build-windows: " + (& $rustc -V) + " | " + (& $cargo -V))
    Write-Output ("# repro-build-windows: root=" + $Root + " Break=" + $Break + " remap_flags=" + $remapCount)
    Write-Output ("# repro-build-windows: RUSTFLAGS_LEN=" + $env:RUSTFLAGS.Length)
}

$log = Join-Path $env:TEMP "vifast-kt79-build.log"
$sw = [System.Diagnostics.Stopwatch]::StartNew()
& $cargo build --manifest-path "v2\kernel\Cargo.toml" -p $Package --bin $Bin --target riscv64gc-unknown-none-elf --release *> $log
$code = $LASTEXITCODE
$sw.Stop()
if ($code -ne 0) {
    Get-Content $log | Select-Object -Last 20
    Write-Error "repro-build-windows: cargo build FAILED (exit $code, log $log)"
    exit 1
}
$img = Join-Path $kernel "target\riscv64gc-unknown-none-elf\release\$Bin"
if (-not (Test-Path $img)) { Write-Error "repro-build-windows: image not found: $img"; exit 1 }
$sha = (Get-FileHash -Algorithm SHA256 $img).Hash.ToLower()
$size = (Get-Item $img).Length
$digest = "build=? text=?"
$digestPy = Join-Path $Root "v2\test\conformance\image_digest.py"
if (Test-Path $digestPy) {
    $py = (Get-Command python -ErrorAction SilentlyContinue)
    if ($py) { $digest = (& $py.Source $digestPy $img) }
}
Write-Output ("REPRO machine=Windows-" + $env:COMPUTERNAME + " root=" + $Root + " sha256=" + $sha + " size=" + $size + " wall_s=" + [int]($sw.Elapsed.TotalSeconds) + " " + $digest + " image=" + $img)
