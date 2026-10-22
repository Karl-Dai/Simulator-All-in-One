param([string]$CheckScript = (Join-Path $PSScriptRoot '../packaging/windows/check-payload.ps1'))
$ErrorActionPreference = 'Stop'
if (!$env:SystemRoot) { throw 'This test requires native Windows PowerShell.' }
$nativeDirectory = if ([Environment]::Is64BitOperatingSystem -and ![Environment]::Is64BitProcess) { 'Sysnative' } else { 'System32' }
$windowsPowerShells = @((Join-Path $env:SystemRoot ($nativeDirectory + '/WindowsPowerShell/v1.0/powershell.exe')))
if ([Environment]::Is64BitOperatingSystem) { $windowsPowerShells += Join-Path $env:SystemRoot 'SysWOW64/WindowsPowerShell/v1.0/powershell.exe' }
foreach ($hostPath in $windowsPowerShells) {
    if (!(Test-Path -LiteralPath $hostPath -PathType Leaf)) { throw "Windows PowerShell 5.1 is required: $hostPath" }
}
$CheckScript = [IO.Path]::GetFullPath($CheckScript)
$unicodeLabel = ([char]0x4E2D).ToString() + ([char]0x6587).ToString()
$work = Join-Path ([IO.Path]::GetTempPath()) ('SimLab-validator-' + [Guid]::NewGuid())
$root = Join-Path $work ("installed $unicodeLabel space")
$utf8 = New-Object Text.UTF8Encoding($false)
$oldRoot = $env:SIMLAB_CHECK_ROOT
$oldMode = $env:SIMLAB_CHECK_MODE
$oldLog = $env:SIMLAB_CHECK_LOG
$diagnosticLog = Join-Path $work 'checker.log'
[IO.Directory]::CreateDirectory($root) | Out-Null

function New-FixtureFile([string]$relative, [string]$content) {
    $path = Join-Path $root $relative
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($path)) | Out-Null
    [IO.File]::WriteAllText($path, $content, $utf8)
    return @{type='file'; sha256=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()}
}

function Invoke-Validator([string]$mode) {
    $env:SIMLAB_CHECK_ROOT = $root
    $env:SIMLAB_CHECK_MODE = $mode
    $env:SIMLAB_CHECK_LOG = $diagnosticLog
    $info = New-Object Diagnostics.ProcessStartInfo
    $info.FileName = $windowsPowerShell
    $info.Arguments = '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' + $CheckScript + '"'
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $process = New-Object Diagnostics.Process
    $process.StartInfo = $info
    try {
        [void]$process.Start()
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (!$process.WaitForExit(30000)) {
            $process.Kill()
            $process.WaitForExit()
            throw "Validator timed out in $mode mode."
        }
        return [PSCustomObject]@{
            Code=$process.ExitCode
            Output=$stdout.GetAwaiter().GetResult() + $stderr.GetAwaiter().GetResult()
        }
    } finally {
        $process.Dispose()
    }
}

function Assert-Validator([string]$mode, [bool]$success, [string]$scenario) {
    $result = Invoke-Validator $mode
    if (($result.Code -eq 0) -ne $success) {
        $diagnostics = if (Test-Path -LiteralPath $diagnosticLog) { [IO.File]::ReadAllText($diagnosticLog, $utf8) } else { 'No validator diagnostics were produced.' }
        throw "${scenario}: unexpected validator exit $($result.Code) in $mode mode.`n$($result.Output)`n$diagnostics"
    }
    Write-Host "PASS $scenario ($mode)"
}

try {
    # All payloads are plain text fixtures. No launcher or role file is executed.
    [IO.File]::WriteAllText((Join-Path $root '.simlab-owned'), 'SimLab-Desktop cloud.carldai.simlab', $utf8)
    $launcherFiles = @{'SimLab.exe'=(New-FixtureFile 'SimLab.exe' 'non-executable launcher fixture')}
    $roles = @('iec104.master','iec104.slave','modbus.master','modbus.slave','pmu.master','pmu.sub','opcua.master','opcua.server')
    $components = @()
    foreach ($role in $roles) {
        $payloadRoot = 'apps/' + $role
        $resource = 'data/' + $unicodeLabel + '.fixture'
        $files = @{
            'payload.fixture'=(New-FixtureFile ($payloadRoot + '/payload.fixture') ('non-executable fixture ' + $role))
            $resource=(New-FixtureFile ($payloadRoot + '/' + $resource) ('resource ' + $role))
        }
        $components += @{
            id=$role; version='0.0.0-fixture'; sourceCommit=('a' * 40); suiteBuild=$true
            executable=($payloadRoot + '/payload.fixture'); sha256=$files['payload.fixture'].sha256
            payloadRoot=$payloadRoot; payloadFiles=$files
        }
    }
    $manifest = @{schemaVersion=1; suiteVersion='0.0.0-fixture'; platform='windows'; architecture='amd64'; launcherFiles=$launcherFiles; components=$components}
    [IO.File]::WriteAllText((Join-Path $root 'suite-manifest.json'), ($manifest | ConvertTo-Json -Depth 12), $utf8)
    foreach ($windowsPowerShell in $windowsPowerShells) {
        Write-Host "Testing native validator host: $windowsPowerShell"
        Assert-Validator 'verify' $true 'complete eight-role UTF-8 payload'
        Assert-Validator 'target' $true 'owned healthy installation'

        $firstPath = Join-Path $root $components[0].executable
        $firstBytes = [IO.File]::ReadAllBytes($firstPath)
        Remove-Item -LiteralPath $firstPath
        Assert-Validator 'target' $true 'missing declared file remains repairable'
        Assert-Validator 'verify' $false 'missing declared file cannot pass verification'
        [IO.File]::WriteAllBytes($firstPath, $firstBytes)
        Assert-Validator 'verify' $true 'restored declared file'

        $unknown = Join-Path $root 'preserve-unrelated.txt'
        [IO.File]::WriteAllText($unknown, 'preserve unrelated fixture', $utf8)
        Assert-Validator 'target' $false 'unknown file blocks replacement'
        Assert-Validator 'verify' $false 'unknown file blocks verification'
        if ([IO.File]::ReadAllText($unknown, $utf8) -ne 'preserve unrelated fixture') { throw 'Validator changed the unknown file.' }
        Remove-Item -LiteralPath $unknown
    }
    $records = @(Get-Content -LiteralPath $diagnosticLog -Encoding UTF8 | ForEach-Object { $_ | ConvertFrom-Json })
    if (!($records | Where-Object { $_.status -eq 'failure' -and $_.detail -like '*Unrelated file*' })) { throw 'Validator did not record its explicit rejection reason.' }
    if ($records | Where-Object { [IO.Path]::GetFileName($_.root) -cne [IO.Path]::GetFileName($root) -or $_.mode -notin @('target', 'verify') }) { throw 'Validator diagnostics lost the UTF-8 root or mode.' }
    if ($records | Where-Object { $_.powershellVersion -notlike '5.1.*' }) { throw 'The validator was not exercised under Windows PowerShell 5.1.' }
    $expectedBits = if ([Environment]::Is64BitOperatingSystem) { @(32, 64) } else { @(32) }
    if (Compare-Object $expectedBits @($records.processBits | Sort-Object -Unique)) { throw 'Not all native PowerShell process architectures were exercised.' }
    Write-Host 'PASS: native Windows PowerShell validation; eight synthetic roles; no payload executed.'
} finally {
    $env:SIMLAB_CHECK_ROOT = $oldRoot
    $env:SIMLAB_CHECK_MODE = $oldMode
    $env:SIMLAB_CHECK_LOG = $oldLog
    Remove-Item -LiteralPath $work -Recurse -Force
}
