# ASR Benchmark Samples

Use this file as the reference list for repeatable local ASR timing. Record each sentence as a separate wav file, then save a same-name `.txt` file with the expected text.

Generate the 10 reference `.txt` files and a local README:

```powershell
app\VoiceIME.exe --write-asr-benchmark-template D:\voice-ime-benchmarks\asr
```

The command writes `001.txt` through `010.txt` and does not overwrite existing files. Record matching `001.wav` through `010.wav` in the same folder before running benchmark commands.

The same template can be created from the GUI: open Settings / Data, click `ASR 样本`, and choose the target sample folder.

Example:

```text
001.wav
001.txt
```

Run:

```powershell
app\VoiceIME.exe --benchmark-asr D:\voice-ime-benchmarks\asr
```

To compare profiles without changing the saved config:

```powershell
app\VoiceIME.exe --benchmark-asr-profile fast D:\voice-ime-benchmarks\asr
app\VoiceIME.exe --benchmark-asr-profile balanced D:\voice-ime-benchmarks\asr
app\VoiceIME.exe --benchmark-asr-profile fallback D:\voice-ime-benchmarks\asr
app\VoiceIME.exe --benchmark-asr-profile accurate D:\voice-ime-benchmarks\asr
```

The `accurate` profile is experimental and uses `asr.accurate_external_command`. The command receives UTF-8 JSON on stdin:

```json
{
  "wav_path": "C:/Temp/voice_ime_accurate_asr.wav",
  "sample_rate": 16000,
  "language": "zh",
  "profile": "accurate",
  "prompt": ""
}
```

It may print plain transcript text or JSON such as `{"text":"..."}` / `{"transcript":"..."}`. This lets Qwen3-ASR, FunASR, or a local ASR service be wrapped by a small script without putting large experimental models into the core app package.

For deterministic release or CI checks, set `.voice_ime/config.json` temporarily:

```json
{
  "asr": {
    "default_engine": "mock",
    "profile": "balanced"
  }
}
```

In mock mode the app does not load ASR model files. During benchmark, each same-name `.txt` file is used as the transcript for that `.wav`, so the CSV pipeline can be checked without a real model. This only validates app plumbing and scoring; it does not measure recognition quality.

Suggested Chinese sample set:

1. 今天下午三点半我们开一个十分钟的短会。
2. 请把非洲之星和海洋之泪加入个人词表。
3. 这个判断很准，输入法的边界就是不要替我说话。
4. 订单金额是一百二十三点四五元，折扣是百分之十二点五。
5. 二零二六年六月五号早上九点提醒我检查模型目录。
6. Voice IME 的 fast 模型应该优先保证响应速度。
7. 帮我把这句话改得更正式一点，但不要改变原意。
8. 我明天要在单位的老电脑上测试麦克风和快捷键。
9. 如果光标定位失败，就回到主窗口确认栏。
10. 这段语音比较长，需要测试三十秒以上的连续转写。

CSV output fields:

```text
file,duration_seconds,profile,worker_mode,backend,model,transcribe_seconds,rtf,expected_chars,edit_distance,cer,accuracy,expected,text,error
```

`cer` is character error rate after lowercasing and removing whitespace only. Punctuation and numbers remain counted, so the score still catches decimal points, units, and sentence marks.
