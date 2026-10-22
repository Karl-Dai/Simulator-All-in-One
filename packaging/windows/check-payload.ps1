$ErrorActionPreference = 'Stop'
try {
    if (!$env:SIMLAB_CHECK_ROOT) { throw 'Missing installation root' }
    $root = [IO.Path]::GetFullPath($env:SIMLAB_CHECK_ROOT).TrimEnd('\')
    $mode = $env:SIMLAB_CHECK_MODE
    if ($mode -in @('active', 'target', 'owned')) {
        $prefix = $root + '\'
        $active = Get-CimInstance Win32_Process -ErrorAction Stop | Where-Object {
            $_.ExecutablePath -and $_.ExecutablePath.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)
        } | Select-Object -First 1
        if ($active) { throw ('Close all SimLab windows first: ' + $active.Name) }
    }
    if ($mode -eq 'active') { exit 0 }
    if ($mode -eq 'target') {
        if (!(Test-Path -LiteralPath $root)) { exit 0 }
        if (@(Get-ChildItem -LiteralPath $root -Force).Count -eq 0) { exit 0 }
    }
    $marker = Join-Path $root '.simlab-owned'
    if (!(Test-Path -LiteralPath $marker) -or (Get-Content -LiteralPath $marker -Raw).Trim() -ne 'SimLab-Desktop cloud.carldai.simlab') {
        throw 'The selected nonempty directory is not an owned SimLab installation; unrelated files were preserved'
    }
    if (@(Get-ChildItem -LiteralPath $root -Recurse -Force -Attributes ReparsePoint).Count) { throw 'Reparse points are not permitted in the installation directory' }
    if ($mode -ne 'target' -and !(Test-Path -LiteralPath (Join-Path $root 'SimLab.exe') -PathType Leaf)) { throw 'Missing launcher' }
    $manifest = Get-Content -LiteralPath (Join-Path $root 'suite-manifest.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    $expected = @('iec104.master','iec104.slave','modbus.master','modbus.slave','pmu.master','pmu.sub','opcua.master','opcua.server')
    $actual = @($manifest.components | ForEach-Object { $_.id } | Sort-Object -Unique)
    if ($manifest.schemaVersion -ne 1 -or $manifest.platform -ne 'windows' -or @($manifest.components).Count -ne 8 -or (Compare-Object ($expected | Sort-Object) $actual)) { throw 'Incomplete suite manifest' }
    $allowed = @{ 'suite-manifest.json' = $true; '.simlab-owned' = $true; 'Uninstall.exe' = $true }
    function Verify-File($relative, $expectedHash) {
        $path = [IO.Path]::GetFullPath((Join-Path $root $relative))
        if (!$path.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Payload path escapes suite' }
        if ($mode -ne 'target' -and (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expectedHash) { throw ('Payload checksum mismatch: ' + $relative) }
        $allowed[$relative.Replace('\', '/')] = $true
    }
    if (!$manifest.launcherFiles) { throw 'Missing launcher resource manifest' }
    foreach ($file in $manifest.launcherFiles.PSObject.Properties) {
        if ($file.Value.type -ne 'file') { throw 'Unsupported launcher resource type' }
        Verify-File $file.Name $file.Value.sha256
    }
    foreach ($component in $manifest.components) {
        if (!$component.suiteBuild -or $component.sourceCommit -notmatch '^[0-9a-f]{40}$' -or !$component.payloadFiles) { throw 'Invalid component identity or resource tree' }
        Verify-File $component.executable $component.sha256
        foreach ($file in $component.payloadFiles.PSObject.Properties) {
            if ($file.Value.type -ne 'file') { throw 'Unsupported component resource type' }
            Verify-File ($component.payloadRoot + '/' + $file.Name) $file.Value.sha256
        }
    }
    foreach ($file in Get-ChildItem -LiteralPath $root -Recurse -Force -File) {
        $relative = $file.FullName.Substring($root.Length + 1).Replace('\', '/')
        if (!$allowed.ContainsKey($relative)) { throw ('Unrelated file in installation directory; preserved: ' + $relative) }
    }
    exit 0
} catch {
    Write-Error $_ -ErrorAction Continue
    exit 1
}
