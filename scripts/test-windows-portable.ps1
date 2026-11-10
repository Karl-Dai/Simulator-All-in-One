param([Parameter(Mandatory=$true)][string]$Package)
$ErrorActionPreference = 'Stop'
$work = Join-Path ([IO.Path]::GetTempPath()) ('SAIO-portable-' + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $work | Out-Null
$Package = [IO.Path]::GetFullPath($Package)
$env:LOCALAPPDATA = Join-Path $work 'cache'
$env:APPDATA = Join-Path $work 'userdata'
New-Item -ItemType Directory -Path $env:LOCALAPPDATA, $env:APPDATA | Out-Null
function Extract-Package {
    $stdout = Join-Path $work 'extract.txt'
    $stderr = Join-Path $work 'extract-error.txt'
    $process = Start-Process -FilePath $Package -ArgumentList '--extract-only' -Wait -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    if ($process.ExitCode -ne 0) { throw (Get-Content -LiteralPath $stderr -Raw) }
    return (Get-Content -LiteralPath $stdout -Raw).Trim()
}
$root = Extract-Package
if (!$root.StartsWith($env:LOCALAPPDATA + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Extraction escaped the user cache' }
if ((Extract-Package) -ne $root) { throw 'Second launch did not reuse its cache' }
$manifest = Get-Content -LiteralPath (Join-Path $root 'suite-manifest.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if (@($manifest.components).Count -ne 8) { throw 'Portable package is incomplete' }
foreach ($component in $manifest.components) {
    if ($component.version -ne $manifest.suiteVersion) { throw 'Component has an independent version' }
}
if (Test-Path (Join-Path $root 'Uninstall.exe')) { throw 'Portable package contains an uninstaller' }
if (Test-Path (Join-Path $root 'webview2')) { throw 'Portable package bundles WebView2' }
$keep = Join-Path $env:APPDATA 'SimLab\preserve-project.txt'
New-Item -ItemType Directory -Path ([IO.Path]::GetDirectoryName($keep)) -Force | Out-Null
Set-Content -LiteralPath $keep -Value 'user project'
$componentPath = Join-Path $root $manifest.components[0].executable
Remove-Item -LiteralPath $componentPath
if ((Extract-Package) -ne $root) { throw 'Cache repair changed its identity' }
if (!(Test-Path -LiteralPath $componentPath)) { throw 'Missing component was not repaired' }
if ((Get-Content -LiteralPath $keep).Trim() -ne 'user project') { throw 'Cache repair changed user data' }
if (@(Get-ChildItem -LiteralPath ([IO.Path]::GetDirectoryName($root)) -Directory | Where-Object Name -Like '*.damaged-*').Count -ne 1) { throw 'Damaged cache was not preserved' }
python scripts/smoke.py $root
if ($LASTEXITCODE -ne 0) { throw 'Portable suite startup smoke failed (the CI host must have WebView2 installed)' }
Write-Host "PASS: single EXE, cache reuse, repair, eight shared versions, preserved projects and nine-process startup. Fixture: $work"
