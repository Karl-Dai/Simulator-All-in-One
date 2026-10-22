param([Parameter(Mandatory=$true)][string]$Installer)
$ErrorActionPreference = 'Stop'
$work = Join-Path ([IO.Path]::GetTempPath()) ('SimLab-installer-' + [Guid]::NewGuid())
$target = Join-Path $work 'installed'
$foreign = Join-Path $work 'unrelated'
New-Item -ItemType Directory -Path $foreign -Force | Out-Null
Set-Content -LiteralPath (Join-Path $foreign 'keep.txt') -Value 'unrelated data'
function Install-To([string]$directory) {
    $process = Start-Process -FilePath $Installer -ArgumentList @('/S', "/D=$directory") -Wait -PassThru
    return $process.ExitCode
}
if ((Install-To $foreign) -eq 0) { throw 'Installer accepted an unrelated nonempty directory' }
if ((Get-Content -LiteralPath (Join-Path $foreign 'keep.txt')).Trim() -ne 'unrelated data') { throw 'Installer changed unrelated data' }
if ((Install-To $target) -ne 0) { throw 'Initial suite installation failed' }
$manifest = Get-Content -LiteralPath (Join-Path $target 'suite-manifest.json') -Raw | ConvertFrom-Json
$component = $manifest.components[0]
$componentPath = Join-Path $target $component.executable
Remove-Item -LiteralPath $componentPath
if ((Install-To $target) -ne 0) { throw 'Installer could not repair a missing declared component' }
if ((Get-FileHash -LiteralPath $componentPath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $component.sha256) { throw 'Repaired component has the wrong checksum' }
$document = Join-Path $target 'preserve-document.txt'
Set-Content -LiteralPath $document -Value 'preserve'
if ((Install-To $target) -eq 0) { throw 'Installer overwrote an installation containing unrelated files' }
if ((Get-Content -LiteralPath $document).Trim() -ne 'preserve') { throw 'Installer changed an unrelated file' }
Remove-Item -LiteralPath $document
$env:SIMLAB_DATA_DIR = Join-Path $work 'data'
$launcher = Start-Process -FilePath (Join-Path $target 'SimLab.exe') -PassThru
try {
    Start-Sleep -Seconds 8
    if ($launcher.HasExited) { throw 'Installed launcher exited before active-instance test' }
    if ((Install-To $target) -eq 0) { throw 'Installer accepted an update while SimLab was running' }
} finally {
    taskkill /F /T /PID $launcher.Id 2>$null | Out-Null
}
python scripts/smoke.py $target
if ($LASTEXITCODE -ne 0) { throw 'Installed suite startup smoke failed' }
$uninstaller = Start-Process -FilePath (Join-Path $target 'Uninstall.exe') -ArgumentList '/S' -Wait -PassThru
if ($uninstaller.ExitCode -ne 0) { throw 'Suite uninstall failed' }
if (Test-Path -LiteralPath (Join-Path $target 'SimLab.exe')) { throw 'Uninstall left the launcher installed' }
Write-Host "PASS: isolated install, repair, unrelated-file protection, active-instance guard, startup, uninstall. Fixture retained at $work"
