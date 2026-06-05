param()

$ErrorActionPreference = "Stop"

try {
    [Console]::InputEncoding = [System.Text.Encoding]::UTF8
    [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
}
catch {
}

$stdin = [Console]::In.ReadToEnd()
if ([string]::IsNullOrWhiteSpace($stdin)) {
    throw "missing stdin JSON"
}

$payload = $stdin | ConvertFrom-Json
if (-not $payload.wav_path) {
    throw "missing wav_path"
}

function Join-Codepoints {
    param([int[]]$Codepoints)
    return (($Codepoints | ForEach-Object { [char]$_ }) -join "")
}

$text = Join-Codepoints @(0x975e, 0x6d32, 0x4e4b, 0x661f, 0x548c, 0x6d77, 0x6d0b, 0x4e4b, 0x6cea)
$json = [pscustomobject]@{
    text = $text
    backend = "mock-external-asr"
} | ConvertTo-Json -Compress
[Console]::Out.WriteLine($json)
