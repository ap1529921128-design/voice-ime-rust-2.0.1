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
$source = [string]$payload.source
$target = [string]$payload.target_language

function Out-Translation {
    param([string]$Text)
    $json = [pscustomobject]@{ text = $Text } | ConvertTo-Json -Compress
    [Console]::Out.WriteLine($json)
}

function Join-Codepoints {
    param([int[]]$Codepoints)
    return (($Codepoints | ForEach-Object { [char]$_ }) -join "")
}

switch ($target) {
    "en" {
        if ($source -like "*非洲之星*") {
            Out-Translation "The Star of Africa and the Tear of the Ocean"
        }
        elseif ($source -like "*设置页*") {
            Out-Translation "The settings page can check the local LLM service."
        }
        else {
            Out-Translation "Local-first translation result"
        }
    }
    "ja" {
        Out-Translation (Join-Codepoints @(0x30ed, 0x30fc, 0x30ab, 0x30eb, 0x7ffb, 0x8a33, 0x3067, 0x3059, 0x3002))
    }
    "zh" {
        Out-Translation $source
    }
    default {
        Out-Translation $source
    }
}
