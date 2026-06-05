# Translation Benchmark

Voice IME can benchmark the configured translation backend without recording audio. This is meant to catch slow local LLM responses, prompt-like explanations, repeated "翻译结果" wrappers, and target-language misses.

## Run

Portable package:

```powershell
app\VoiceIME.exe --benchmark-translation
```

Custom samples:

```powershell
app\VoiceIME.exe --benchmark-translation D:\voice-ime-benchmarks\translation-samples.tsv
```

The GUI exposes the built-in benchmark from Settings / Data / `翻译基准`.

Portable packages also include an offline external-backend acceptance helper:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\app\tools\Translation-Acceptance.ps1
```

It uses the packaged `Mock-External-Translate.ps1` command with a temporary `VOICE_IME_APP_DIR`, so it verifies the external JSON pipeline without requiring a real MT model.

## Sample Format

The sample file can be TSV or simple CSV. Each non-empty, non-comment row is:

```text
target_language<TAB>source_text<TAB>expected_hint
```

`expected_hint` is optional. Multiple required hints can be separated with `|`.

Targets accept `zh`, `en`, `ja`, plus common labels such as `中文`, `english`, and `日语`.

Example:

```text
# target	source	expected_hint
en	非洲之星和海洋之泪	
ja	本地优先，不默认上传云端	
zh	翻译结果：非洲之星和海洋之泪	非洲之星
```

## Output

Results are written to:

```text
app/.voice_ime/logs/translation-benchmark-YYYYMMDD-HHMMSS.csv
```

Columns include target language, engine, model, timeout, elapsed seconds, language match, optional hint match, source, output, and error.

Backend errors are recorded as CSV rows instead of crashing the app. Prompt leaks, missing edit-target chatter, and explanatory translation chatter are treated as errors.
