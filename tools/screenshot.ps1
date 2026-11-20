# Takes the README picture: screenshots/veritas-main.png (light) and
# screenshots/veritas-main-dark.png (dark), from the example data in docs/examples.
#
#   pwsh tools/screenshot.ps1              # builds release and takes both pictures
#   pwsh tools/screenshot.ps1 -NoBuild     # with the existing release EXE
#   pwsh tools/screenshot.ps1 -Lang de     # another language (de, en, fr, es, it), default en
#
# Briefly opens a window twice (without focus); keys or clicks are not simulated.
# The example folder is copied to temp, because verify appends to its history; the picture
# shows only folder names (the tree starts at the parent folder), no path with a user name.
# Written for pwsh on Windows and Linux; tested on Windows.
param(
    [switch]$NoBuild,
    # Window size in points and pixels per point: 960 × 1.5 = 1440 pixels, 720 wide in the README.
    [string]$Size = '960x640',
    [double]$Scale = 1.5,
    # Interface language in the picture; only for the capture, nothing is stored. The
    # README picture is English; another language is written under the same file names.
    [ValidateSet('de', 'en', 'fr', 'es', 'it')]
    [string]$Lang = 'en'
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $repo ('target/release/veritas' + $(if ($IsWindows) { '.exe' } else { '' }))
$example = Join-Path $repo 'docs/examples/Urlaub 2026'
$target = Join-Path $repo 'screenshots'
$limit = 300KB

if (-not $NoBuild) {
    Push-Location $repo
    try {
        cargo build --release --locked
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed ($LASTEXITCODE)." }
    } finally { Pop-Location }
}
if (-not (Test-Path $exe)) { throw "$exe is missing – call without -NoBuild." }
New-Item -ItemType Directory -Force $target | Out-Null

$scaleText = $Scale.ToString([Globalization.CultureInfo]::InvariantCulture)
$shots = @(
    @{ Theme = 'light'; File = 'veritas-main.png' },
    @{ Theme = 'dark'; File = 'veritas-main-dark.png' }
)
foreach ($shot in $shots) {
    # Every capture with a fresh copy: the history then always shows the same runs.
    $work = Join-Path ([IO.Path]::GetTempPath()) "veritas-screenshot-$PID-$($shot.Theme)"
    $fotos = Join-Path $work 'Fotos'
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force (Join-Path $fotos 'Geburtstag 2025'), (Join-Path $fotos 'Urlaub 2025') | Out-Null
    Copy-Item -Recurse $example $fotos
    $folder = Join-Path $fotos 'Urlaub 2026'
    $out = Join-Path $target $shot.File
    $log = Join-Path $work 'stderr.txt'
    # The release EXE is a windowed program on Windows: only with -Wait does pwsh wait for it,
    # stderr is visible only when redirected.
    $argList = @('--screenshot', "`"$out`"", '--theme', $shot.Theme, '--lang', $Lang, '--verify',
        '--size', $Size, '--scale', $scaleText, "`"$folder`"")
    $process = Start-Process -FilePath $exe -ArgumentList $argList -Wait -PassThru `
        -RedirectStandardError $log
    $message = (Get-Content -Raw $log -ErrorAction SilentlyContinue)
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
    if ($process.ExitCode -ne 0) {
        throw "Capture '$($shot.Theme)' failed (exit $($process.ExitCode)): $message"
    }
    # Optionally shrink losslessly if oxipng is installed.
    if (Get-Command oxipng -ErrorAction SilentlyContinue) {
        oxipng -o max --strip safe --quiet $out
    }
    $bytes = (Get-Item $out).Length
    Write-Host ('{0}: {1:N0} KB' -f $shot.File, ($bytes / 1KB))
    if ($bytes -gt $limit) {
        Write-Warning "$($shot.File) is larger than 300 KB (repo standard §3.1)."
    }
}
