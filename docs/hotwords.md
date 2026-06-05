# Hotwords And Rules

Date: 2026-06-04

Voice IME supports two local text files for user-controlled replacements:

```text
app/.voice_ime/hot.txt
app/.voice_ime/hot-rule.txt
```

Open them from Settings with the `热词` and `规则` buttons. Changes apply on the next transcription. Settings / Data / `刷新词表` and Doctor show the current hotword entry count, alias count, valid rule count, and invalid regex examples. Settings / Data / `词表试算` can preview one sentence through normalization, built-in corrections, hotwords, rules, ITN, and final cleanup before real dictation.

## hot.txt

Use `hot.txt` for exact alias replacement. The first item is the final output, and aliases follow after `|`.

```text
Voice IME | voice ime | 语音 IME
CapsWriter | caps writer | Caps Rider
非洲之星 | 非州之星
OpenAI | open ai | 欧盆 AI
```

If ASR outputs `voice ime`, Voice IME changes it to `Voice IME`.

## hot-rule.txt

Use `hot-rule.txt` for regex replacement:

```text
pattern = replacement
```

Examples:

```text
毫安时 = mAh
赫兹 = Hz
艾特\s*(\w+)\s*点\s*(\w+) = @\1.\2
```

The replacement side supports `\1`, `\2`, and other capture references. Invalid regex rules are skipped and surfaced in Settings / Data / `刷新词表` and Doctor.

## Preview

Open Settings / Data, enter a sentence in `词表试算`, and click `试算`. The preview shows each deterministic stage, whether it changed the text, and the local match count.

## Boundary

This is exact alias replacement plus regex replacement. Phoneme/RAG-style fuzzy hotword matching is a later enhancement.
