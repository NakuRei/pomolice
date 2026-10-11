param(
    [Parameter(Mandatory = $true)]
    [string]$Installer,
    [Parameter(Mandatory = $true)]
    [string]$ExpectedVersion
)

$ErrorActionPreference = 'Stop'
$installerPath = (Resolve-Path -LiteralPath $Installer).Path
$sevenZip = Join-Path $env:ProgramFiles '7-Zip/7z.exe'
$workspace = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$extractionRoot = Join-Path $workspace 'artifacts/installer-checks'
$extraction = Join-Path $extractionRoot ([Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $extraction -Force | Out-Null

try {
    & $sevenZip x $installerPath "-o$extraction" -y
    if ($LASTEXITCODE -ne 0) { throw 'NSIS installer extraction failed.' }
    $binary = Join-Path $extraction 'pomolice.exe'
    $binaryVersion = (Get-Item -LiteralPath $binary).VersionInfo.ProductVersion
    if ($binaryVersion -ne $ExpectedVersion) {
        throw "Executable version mismatch: $binaryVersion; expected $ExpectedVersion."
    }
    $bytes = [IO.File]::ReadAllBytes($binary)
    $peOffset = [BitConverter]::ToInt32($bytes, 0x3c)
    if ([BitConverter]::ToUInt32($bytes, $peOffset) -ne 0x00004550 -or
        [BitConverter]::ToUInt16($bytes, $peOffset + 4) -ne 0x8664) {
        throw 'Expected an x64 PE executable.'
    }
    foreach ($sound in @('work', 'short_break', 'long_break')) {
        $original = Join-Path $workspace "src-tauri/resources/sounds/$sound.wav"
        $packaged = Join-Path $extraction "resources/sounds/$sound.wav"
        if ((Get-FileHash -LiteralPath $original).Hash -ne (Get-FileHash -LiteralPath $packaged).Hash) {
            throw "Packaged sound differs from source: $sound"
        }
    }
    $notice = Join-Path $extraction 'THIRD-PARTY-NOTICES.md'
    if ((Get-FileHash -LiteralPath $notice).Hash -ne
        (Get-FileHash -LiteralPath (Join-Path $workspace 'THIRD-PARTY-NOTICES.md')).Hash) {
        throw 'Packaged third-party notices differ from source.'
    }
    if (Test-Path -LiteralPath (Join-Path $extraction 'export_tray_icon_fixtures.exe')) {
        throw 'Test fixture executable was included in the installer.'
    }
    Write-Output "Verified Windows x64 installer version $ExpectedVersion."
} finally {
    $resolvedExtraction = (Resolve-Path -LiteralPath $extraction).Path
    $resolvedRoot = (Resolve-Path -LiteralPath $extractionRoot).Path
    if ([IO.Path]::GetDirectoryName($resolvedExtraction) -ne $resolvedRoot) {
        throw "Refusing to remove unexpected extraction directory: $resolvedExtraction"
    }
    Remove-Item -LiteralPath $resolvedExtraction -Recurse -Force
}
