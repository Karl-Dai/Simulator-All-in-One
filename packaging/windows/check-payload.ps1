$ErrorActionPreference = 'Stop'
$root = ''
$mode = $env:SIMLAB_CHECK_MODE
$inheritedModulePath = $env:PSModulePath
$nativeModulePath = [IO.Path]::Combine($PSHOME, 'Modules')
function Write-CheckDiagnostic([string]$status, [string]$detail = '') {
    if (!$env:SIMLAB_CHECK_LOG) { return }
    try {
        $logPath = [IO.Path]::GetFullPath($env:SIMLAB_CHECK_LOG)
        if ($root -and $logPath.StartsWith($root + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { return }
        [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($logPath)) | Out-Null
        $entry = @{time=[DateTime]::UtcNow.ToString('o'); mode=$mode; root=$root; status=$status; detail=$detail; processBits=([IntPtr]::Size * 8); powershellVersion=$PSVersionTable.PSVersion.ToString(); psHome=$PSHOME; modulePath=$env:PSModulePath; inheritedModulePath=$inheritedModulePath}
        [IO.File]::AppendAllText($logPath, (($entry | ConvertTo-Json -Compress) + [Environment]::NewLine), (New-Object Text.UTF8Encoding($false)))
    } catch {
        Write-Warning ('Could not write optional validator diagnostics: ' + $_.Exception.Message)
    }
}
try {
    if (!$env:SIMLAB_CHECK_ROOT) { throw 'Missing installation root' }
    $root = [IO.Path]::GetFullPath($env:SIMLAB_CHECK_ROOT).TrimEnd('\')
    Write-CheckDiagnostic 'start'
    # NSIS starts Windows PowerShell, which may inherit PowerShell 7 module
    # paths. Load this process's own shipped modules without changing the host.
    $env:PSModulePath = $nativeModulePath + [IO.Path]::PathSeparator + $inheritedModulePath
    Import-Module ([IO.Path]::Combine($nativeModulePath, 'Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1')) -ErrorAction Stop
    if ($mode -in @('active', 'target', 'owned')) {
        Import-Module ([IO.Path]::Combine($nativeModulePath, 'CimCmdlets/CimCmdlets.psd1')) -ErrorAction Stop
        $prefix = $root + '\'
        $active = Get-CimInstance Win32_Process -ErrorAction Stop | Where-Object {
            $_.ExecutablePath -and $_.ExecutablePath.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)
        } | Select-Object -First 1
        if ($active) { throw ('Close all Simulator All-in-One windows first: ' + $active.Name) }
    }
    if ($mode -eq 'active') { Write-CheckDiagnostic 'success'; exit 0 }
    if ($mode -eq 'target') {
        if (!(Test-Path -LiteralPath $root)) { Write-CheckDiagnostic 'success'; exit 0 }
        if (@(Get-ChildItem -LiteralPath $root -Force).Count -eq 0) { Write-CheckDiagnostic 'success'; exit 0 }
    }
    $marker = Join-Path $root '.simlab-owned'
    if (!(Test-Path -LiteralPath $marker) -or (Get-Content -LiteralPath $marker -Raw).Trim() -ne 'SimLab-Desktop cloud.carldai.simlab') {
        throw 'The selected nonempty directory is not an owned Simulator All-in-One installation; unrelated files were preserved'
    }
    if (@(Get-ChildItem -LiteralPath $root -Recurse -Force -Attributes ReparsePoint).Count) { throw 'Reparse points are not permitted in the installation directory' }
    if ($mode -ne 'target' -and !(Test-Path -LiteralPath (Join-Path $root 'SAIO.exe') -PathType Leaf) -and !(Test-Path -LiteralPath (Join-Path $root 'SimLab.exe') -PathType Leaf)) { throw 'Missing launcher' }
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
    Write-CheckDiagnostic 'success'
    exit 0
} catch {
    $failure = $_
    Write-CheckDiagnostic 'failure' $failure.Exception.Message
    Write-Error $failure -ErrorAction Continue
    exit 1
}
