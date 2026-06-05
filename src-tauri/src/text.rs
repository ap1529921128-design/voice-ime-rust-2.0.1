use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};

static SPACE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[ \t]+").unwrap());
static MANY_NEWLINES_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\n{3,}").unwrap());
static REGEX_BACKREF_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\\([1-9][0-9]?)").unwrap());
static THINK_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)<think>.*?</think>\s*").unwrap());
static PUNCT_ONLY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^[\s,.;:!?，。！？；：、…~～\-_'"“”‘’()\[\]{}【】<>《》「」『』]+$"#).unwrap()
});
static LIST_PREFIX_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(?:[-*•]\s*|\d+[.)、]\s*)").unwrap());
static TRANSLATION_LABEL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)^\s*(?:翻译结果|翻译|译文|中文译文|英文译文|英语译文|日文译文|日语译文|简体中文|中文|英文|英语|日文|日语|chinese|english|japanese|translation|translated text|result)\s*(?:如下|如下所示)?\s*[:：]?\s*",
    )
    .unwrap()
});
static TRANSLATION_TAIL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(?:以下是|下面是)?[^：:\n]*(?:翻译为|翻译成|译为|译成|可译为|可以翻译为|可以翻译成|translated as|translation is|translate to)\s*[:：]\s*(.+)$")
        .unwrap()
});
static EXPLANATION_HEADING_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)^\s*(?:解释|说明|注释|备注|原因|分析|note|notes|explanation|commentary)\s*[:：]",
    )
    .unwrap()
});
static TRANSLATION_META_LINE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)^\s*(?:这个|这句|这句话|这段|该句|该短语|以上|如果|根据|可根据|由于|because|depending on|if you|this translation|the translation)\b?.*(?:翻译|译文|语境|含义|意思|意译|直译|保留|translation|context|meaning)",
    )
    .unwrap()
});
static WINDOWS_PATH_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)\b[a-z]:[\\/][^\s，。；；：:"'<>|]+"#).unwrap());
static UNIX_PATH_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)(?:^|\s)(?:[.~]{1,2}/|/)[^\s，。；；：]+"#).unwrap());
static ENV_PATH_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)(?:%[a-z0-9_]+%|\$env:[a-z0-9_]+|\$\{?[a-z0-9_]+\}?)"#).unwrap()
});
static URL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)\b(?:https?|file)://[^\s，。；；：]+"#).unwrap());
static SHELL_COMMAND_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)^\s*(?:cargo|npm|pnpm|yarn|git|gh|rustup|python|py|node|deno|uv|pip|docker|kubectl|ssh|scp|ffmpeg|winget|choco|scoop|powershell|pwsh|cmd|cd|dir|ls|mkdir|copy|xcopy|del|rm|mv|cp|set|start-process)\b",
    )
    .unwrap()
});
static CODE_KEYWORD_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)\b(?:fn|let|mut|const|var|function|class|struct|enum|impl|async|await|import|export|from|return|select|insert|update|delete|where)\b",
    )
    .unwrap()
});
static CODE_OPERATOR_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:->|=>|::|==|!=|<=|>=|&&|\|\||[{};])").unwrap());

pub fn default_corrections() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("PromadeAble".into(), "Prompt eval".into()),
        ("Prompt Able".into(), "Prompt eval".into()),
        ("Promote eval".into(), "Prompt eval".into()),
        ("prompt eval".into(), "Prompt eval".into()),
        ("20tokens每秒".into(), "~20 tokens/s".into()),
        ("20 tokens每秒".into(), "~20 tokens/s".into()),
        ("单token冷起动".into(), "单 token 冷启动".into()),
        ("miniCPM".into(), "minicpm".into()),
        ("mini CPM".into(), "minicpm".into()),
        ("LamaServer".into(), "llama-server".into()),
        ("Lama Server".into(), "llama-server".into()),
        ("llama server".into(), "llama-server".into()),
        ("本地agent".into(), "本地 agent".into()),
        ("项目跟目录".into(), "项目根目录".into()),
        ("有盘退不出".into(), "U盘退不出".into()),
        ("首选Try".into(), "首选 tray".into()),
        ("首选tray".into(), "首选 tray".into()),
        ("sherpa onnx".into(), "sherpa-onnx".into()),
        ("whisper CPP".into(), "whisper.cpp".into()),
    ])
}

pub fn load_corrections(path: &Path) -> BTreeMap<String, String> {
    let mut map = default_corrections();
    if let Ok(body) = fs::read_to_string(path) {
        if let Ok(Value::Object(extra)) = serde_json::from_str::<Value>(&body) {
            for (wrong, right) in extra {
                if let Some(right) = right.as_str() {
                    map.insert(wrong, right.to_string());
                }
            }
        }
    }
    map
}

pub fn normalize_text(text: &str) -> String {
    let mut out = text
        .chars()
        .map(|ch| match ch {
            '０' => '0',
            '１' => '1',
            '２' => '2',
            '３' => '3',
            '４' => '4',
            '５' => '5',
            '６' => '6',
            '７' => '7',
            '８' => '8',
            '９' => '9',
            '，' => '，',
            '；' => '；',
            '：' => '：',
            _ => ch,
        })
        .collect::<String>();
    out = SPACE_RE.replace_all(&out, " ").to_string();
    out = MANY_NEWLINES_RE.replace_all(&out, "\n\n").to_string();
    out = out
        .replace(" 。", "。")
        .replace(" ，", "，")
        .replace(" ；", "；")
        .replace(" ,", ",")
        .replace("\n ", "\n")
        .replace(" \n", "\n");
    out.trim().to_string()
}

pub fn apply_corrections(text: &str, corrections_path: &Path) -> String {
    correction_trace(text, corrections_path).final_text
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionTrace {
    pub raw_text: String,
    pub normalized_text: String,
    pub dictionary_text: String,
    pub hotword_text: String,
    pub rule_text: String,
    pub itn_text: String,
    pub final_text: String,
}

pub fn correction_trace(text: &str, corrections_path: &Path) -> CorrectionTrace {
    let raw_text = text.to_string();
    let mut current = normalize_text(text);
    let normalized_text = current.clone();
    for (wrong, right) in load_corrections(corrections_path) {
        current = current.replace(&wrong, &right);
    }
    let dictionary_text = current.clone();
    for (wrong, right) in load_hotword_replacements(&hotwords_path_for(corrections_path)) {
        current = current.replace(&wrong, &right);
    }
    let hotword_text = current.clone();
    current = apply_hot_rules(&current, &hot_rules_path_for(corrections_path));
    let rule_text = current.clone();
    current = crate::itn::apply_itn(&current);
    let itn_text = current.clone();
    let final_text = normalize_text(&current);
    CorrectionTrace {
        raw_text,
        normalized_text,
        dictionary_text,
        hotword_text,
        rule_text,
        itn_text,
        final_text,
    }
}

pub fn apply_punctuation_policy(text: &str, policy: &str) -> String {
    let mut text = normalize_text(text);
    if policy == "short-no-period"
        && !text.contains('\n')
        && text.chars().count() <= 30
        && text
            .chars()
            .last()
            .is_some_and(|ch| matches!(ch, '。' | '.'))
    {
        text.pop();
    }
    text
}

pub fn load_hotword_replacements(path: &Path) -> Vec<(String, String)> {
    let mut replacements = Vec::new();
    let Ok(body) = fs::read_to_string(path) else {
        return replacements;
    };
    for line in body.lines() {
        let line = strip_inline_comment(line).trim();
        if line.is_empty() {
            continue;
        }
        let parts = line
            .split('|')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>();
        let Some(target) = parts.first() else {
            continue;
        };
        for alias in parts.iter().skip(1) {
            if alias != target {
                replacements.push(((*alias).to_string(), (*target).to_string()));
            }
        }
    }
    replacements.sort_by_key(|item| std::cmp::Reverse(item.0.chars().count()));
    replacements
}

pub fn apply_hot_rules(text: &str, path: &Path) -> String {
    let Ok(body) = fs::read_to_string(path) else {
        return text.to_string();
    };
    let mut text = text.to_string();
    for line in body.lines() {
        let line = strip_inline_comment(line).trim();
        if line.is_empty() {
            continue;
        }
        let Some((pattern, replacement)) = line.split_once('=') else {
            continue;
        };
        let pattern = pattern.trim();
        if pattern.is_empty() {
            continue;
        }
        let replacement = normalize_rule_replacement(replacement.trim());
        if let Ok(regex) = Regex::new(pattern) {
            text = regex.replace_all(&text, replacement.as_str()).to_string();
        }
    }
    text
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UserDictionaryStats {
    pub hotwords_path: String,
    pub hot_rules_path: String,
    pub hotword_entries: usize,
    pub hotword_aliases: usize,
    pub hotword_entries_without_alias: usize,
    pub hot_rule_count: usize,
    pub hot_rule_invalid: usize,
    pub hot_rule_invalid_examples: Vec<String>,
}

pub fn user_dictionary_stats(hotwords_path: &Path, hot_rules_path: &Path) -> UserDictionaryStats {
    let (hotword_entries, hotword_aliases, hotword_entries_without_alias) =
        hotword_stats(hotwords_path);
    let (hot_rule_count, hot_rule_invalid, hot_rule_invalid_examples) =
        hot_rule_stats(hot_rules_path);
    UserDictionaryStats {
        hotwords_path: hotwords_path.to_string_lossy().to_string(),
        hot_rules_path: hot_rules_path.to_string_lossy().to_string(),
        hotword_entries,
        hotword_aliases,
        hotword_entries_without_alias,
        hot_rule_count,
        hot_rule_invalid,
        hot_rule_invalid_examples,
    }
}

fn hotword_stats(path: &Path) -> (usize, usize, usize) {
    let Ok(body) = fs::read_to_string(path) else {
        return (0, 0, 0);
    };
    let mut entries = 0;
    let mut aliases = 0;
    let mut without_alias = 0;
    for line in body.lines() {
        let line = strip_inline_comment(line).trim();
        if line.is_empty() {
            continue;
        }
        let parts = line
            .split('|')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>();
        if parts.is_empty() {
            continue;
        }
        entries += 1;
        let alias_count = parts.len().saturating_sub(1);
        aliases += alias_count;
        if alias_count == 0 {
            without_alias += 1;
        }
    }
    (entries, aliases, without_alias)
}

fn hot_rule_stats(path: &Path) -> (usize, usize, Vec<String>) {
    let Ok(body) = fs::read_to_string(path) else {
        return (0, 0, Vec::new());
    };
    let mut valid = 0;
    let mut invalid = 0;
    let mut examples = Vec::new();
    for (index, raw_line) in body.lines().enumerate() {
        let line = strip_inline_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        let Some((pattern, _replacement)) = line.split_once('=') else {
            invalid += 1;
            push_invalid_rule_example(&mut examples, index + 1, "缺少 =");
            continue;
        };
        let pattern = pattern.trim();
        if pattern.is_empty() {
            invalid += 1;
            push_invalid_rule_example(&mut examples, index + 1, "正则为空");
            continue;
        }
        match Regex::new(pattern) {
            Ok(_) => valid += 1,
            Err(err) => {
                invalid += 1;
                push_invalid_rule_example(
                    &mut examples,
                    index + 1,
                    &err.to_string().chars().take(80).collect::<String>(),
                );
            }
        }
    }
    (valid, invalid, examples)
}

fn push_invalid_rule_example(examples: &mut Vec<String>, line: usize, detail: &str) {
    if examples.len() < 3 {
        examples.push(format!("line {line}: {detail}"));
    }
}

fn strip_inline_comment(line: &str) -> &str {
    let line = line.trim();
    if line.starts_with('#') {
        ""
    } else {
        line
    }
}

fn normalize_rule_replacement(replacement: &str) -> String {
    REGEX_BACKREF_RE
        .replace_all(replacement, "$$$1")
        .to_string()
}

fn hotwords_path_for(corrections_path: &Path) -> std::path::PathBuf {
    corrections_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("hot.txt")
}

fn hot_rules_path_for(corrections_path: &Path) -> std::path::PathBuf {
    corrections_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("hot-rule.txt")
}

pub fn clean_asr_text(text: &str, corrections_path: &Path) -> String {
    let text = THINK_RE
        .replace_all(&apply_corrections(text, corrections_path), "")
        .trim()
        .to_string();
    if is_meaningless_asr_text(&text) {
        String::new()
    } else {
        text
    }
}

pub fn clean_llm_output(text: &str) -> String {
    let mut text = THINK_RE.replace_all(text, "").trim().to_string();
    text = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.contains("[原文]") && !line.contains("【原文】"))
        .collect::<Vec<_>>()
        .join("\n");
    text = text
        .replace("[/原文]", "")
        .replace("【/原文】", "")
        .trim()
        .to_string();
    if PUNCT_ONLY_RE.is_match(&text) {
        String::new()
    } else {
        text
    }
}

pub fn clean_translation_output(text: &str) -> String {
    let cleaned = clean_llm_output(text);
    let mut lines = Vec::new();
    for raw_line in cleaned.lines() {
        let mut line = LIST_PREFIX_RE
            .replace(raw_line.trim(), "")
            .trim()
            .to_string();
        line = strip_wrapping_quotes(&line);
        if line.is_empty() {
            continue;
        }
        if EXPLANATION_HEADING_RE.is_match(&line) {
            break;
        }
        if let Some(captures) = TRANSLATION_TAIL_RE.captures(&line) {
            line = captures
                .get(1)
                .map(|item| item.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
        } else if TRANSLATION_LABEL_RE.is_match(&line) {
            line = TRANSLATION_LABEL_RE.replace(&line, "").trim().to_string();
        }
        line = strip_wrapping_quotes(&line);
        if line.is_empty()
            || looks_like_translation_preamble(&line)
            || TRANSLATION_META_LINE_RE.is_match(&line)
            || line.starts_with("原文")
            || line.starts_with("目标语言")
        {
            continue;
        }
        lines.push(line);
    }
    let result = normalize_text(&lines.join("\n"));
    if PUNCT_ONLY_RE.is_match(&result) {
        String::new()
    } else {
        result
    }
}

pub fn looks_like_translation_chatter(text: &str) -> bool {
    normalize_text(text).lines().any(|line| {
        let compact = line
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_lowercase();
        compact.starts_with("翻译结果")
            || compact.starts_with("译文如下")
            || compact.starts_with("以下是")
            || compact.starts_with("下面是")
            || compact.starts_with("解释:")
            || compact.starts_with("解释：")
            || compact.starts_with("说明:")
            || compact.starts_with("说明：")
            || compact.starts_with("注释:")
            || compact.starts_with("注释：")
            || compact.starts_with("备注:")
            || compact.starts_with("备注：")
            || compact.starts_with("候选")
            || compact.starts_with("这个翻译")
            || compact.starts_with("这句话可以")
            || compact.starts_with("这句可以")
            || compact.starts_with("该短语可以")
            || compact.starts_with("可以根据语境")
            || compact.starts_with("如果需要")
            || compact.starts_with("根据语境")
            || compact.starts_with("translation:")
            || compact.starts_with("translatedtext:")
            || compact.starts_with("explanation:")
            || compact.starts_with("note:")
    })
}

pub fn has_translation_markup(text: &str) -> bool {
    normalize_text(text).lines().any(|line| {
        let line = LIST_PREFIX_RE.replace(line.trim(), "").trim().to_string();
        let line = strip_wrapping_quotes(&line);
        TRANSLATION_LABEL_RE.is_match(&line)
            || TRANSLATION_TAIL_RE.is_match(&line)
            || EXPLANATION_HEADING_RE.is_match(&line)
            || looks_like_translation_preamble(&line)
            || TRANSLATION_META_LINE_RE.is_match(&line)
    })
}

pub fn is_likely_chinese_text(text: &str) -> bool {
    let mut han = 0usize;
    let mut kana = 0usize;
    let mut latin = 0usize;
    for ch in text.chars() {
        if ('\u{4E00}'..='\u{9FFF}').contains(&ch) {
            han += 1;
        } else if ('\u{3040}'..='\u{30FF}').contains(&ch) {
            kana += 1;
        } else if ch.is_ascii_alphabetic() {
            latin += 1;
        }
    }
    han >= 2 && kana == 0 && han >= latin
}

pub fn looks_like_code_command_or_path(text: &str) -> bool {
    let normalized = normalize_text(text);
    if normalized.trim().is_empty() {
        return false;
    }
    if normalized.contains("```") || normalized.contains('`') {
        return true;
    }
    if WINDOWS_PATH_RE.is_match(&normalized)
        || UNIX_PATH_RE.is_match(&normalized)
        || ENV_PATH_RE.is_match(&normalized)
        || URL_RE.is_match(&normalized)
    {
        return true;
    }
    if normalized
        .lines()
        .any(|line| SHELL_COMMAND_RE.is_match(line))
    {
        return true;
    }
    let ascii_alnum = normalized
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .count();
    if ascii_alnum < 3 {
        return false;
    }
    CODE_OPERATOR_RE.is_match(&normalized)
        && (CODE_KEYWORD_RE.is_match(&normalized)
            || normalized.contains('=')
            || normalized.contains('(')
            || normalized.contains(')'))
}

pub fn join_transcript_chunks(chunks: &[String], corrections_path: &Path) -> String {
    let mut result = String::new();
    for chunk in chunks {
        let chunk = clean_asr_text(chunk, corrections_path);
        if chunk.is_empty() {
            continue;
        }
        if result
            .chars()
            .last()
            .is_some_and(|c| c.is_ascii_alphanumeric())
            && chunk
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric())
        {
            result.push(' ');
        }
        result.push_str(&chunk);
    }
    normalize_text(&result)
}

fn looks_like_translation_preamble(text: &str) -> bool {
    let compact = normalize_text(text)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase();
    compact.is_empty()
        || [
            "翻译结果如下",
            "翻译结果:",
            "翻译结果：",
            "译文如下",
            "如下:",
            "如下：",
            "以下是翻译",
            "下面是翻译",
            "thetranslationis",
            "hereisthetranslation",
        ]
        .iter()
        .any(|marker| compact.contains(marker))
}

fn strip_wrapping_quotes(text: &str) -> String {
    text.trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches('“')
        .trim_matches('”')
        .trim_matches('‘')
        .trim_matches('’')
        .trim()
        .to_string()
}

pub fn is_confirmation_edit_command(text: &str) -> bool {
    let normalized = normalize_text(text);
    let compact: String = normalized.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.is_empty() || compact.chars().count() > 120 {
        return false;
    }
    let patterns = [
        r"帮我.*(润色|改写|优化|整理|精简|扩写|正式|口吻|语气)",
        r"(润色|改写|优化|整理|精简|扩写)(一下|下|这段|上面|前面|它|这个)?",
        r"(改得|改的|改成|改为|变得|调整成).*(正式|自然|口语|书面|礼貌|简洁|专业|邮件|日语|英语|中文)",
        r"(更正式|更自然|更口语|更书面|更礼貌|更简洁|更专业)(一点|些)?",
    ];
    if !patterns
        .iter()
        .any(|pattern| Regex::new(pattern).unwrap().is_match(&compact))
    {
        return false;
    }
    for separator in ['：', ':', '\n'] {
        if let Some((_, tail)) = normalized.split_once(separator) {
            if tail.trim().chars().count() >= 12 {
                return false;
            }
        }
    }
    true
}

pub fn looks_like_missing_edit_target(text: &str) -> bool {
    let compact: String = normalize_text(text)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if compact.is_empty() {
        return false;
    }
    Regex::new(r"(请|需要|麻烦).*(提供|输入|给我).*(内容|文本|原文)|没有.*(内容|文本|原文)|无法.*(润色|改写|修改)")
        .unwrap()
        .is_match(&compact)
}

pub fn looks_like_prompt_leak(text: &str) -> bool {
    let normalized = normalize_text(text);
    let compact: String = normalized.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.is_empty() {
        return false;
    }
    let leak_markers = [
        "个人词表",
        "纠错表",
        "ASR文本",
        "语音编辑指令",
        "当前确认栏文本",
        "请确认以下内容",
        "请提供您的确认内容",
        "我将按照您的要求",
        "以便我继续处理",
    ];
    let marker_hits = leak_markers
        .iter()
        .filter(|marker| compact.contains(*marker))
        .count();
    marker_hits >= 1
        || Regex::new(r"确认您是否需要我.*(整理|处理|提供|继续)")
            .unwrap()
            .is_match(&compact)
}

fn is_meaningless_asr_text(text: &str) -> bool {
    let compact = Regex::new(r#"[\s,.;:!?，。！？；：、…~～\-_'"“”‘’()\[\]{}【】<>《》]+"#)
        .unwrap()
        .replace_all(text, "")
        .to_lowercase();
    if compact.is_empty() {
        return true;
    }
    if compact.chars().count() <= 2
        && Regex::new(r"^[嗯呃额啊哦噢喔唔呐哎]+$")
            .unwrap()
            .is_match(&compact)
    {
        return true;
    }
    let fillers = [
        "嗯", "嗯嗯", "呃", "呃呃", "额", "额额", "啊", "啊啊", "哦", "噢", "喔", "唔", "呐", "哎",
        "哎呀", "那个", "这个", "就是", "然后", "em", "um", "uh", "er", "hmm",
    ];
    let mut reduced = compact.to_string();
    let mut changed = true;
    while changed && !reduced.is_empty() {
        changed = false;
        for unit in fillers {
            if reduced.starts_with(unit) {
                reduced = reduced[unit.len()..].to_string();
                changed = true;
                break;
            }
        }
    }
    reduced.is_empty() && compact.chars().count() <= 12
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn detects_edit_commands() {
        assert!(is_confirmation_edit_command("帮我改得更正式一点"));
        assert!(is_confirmation_edit_command("润色一下"));
        assert!(!is_confirmation_edit_command(
            "帮我改成邮件口吻：这是正文，比较长，应该当作带正文的输入"
        ));
    }

    #[test]
    fn joins_alnum_chunks_with_space() {
        let joined =
            join_transcript_chunks(&["hello".into(), "world".into()], Path::new("missing.json"));
        assert_eq!(joined, "hello world");
    }

    #[test]
    fn applies_hotword_aliases_and_rules() {
        let temp = tempfile::tempdir().unwrap();
        let corrections = temp.path().join("corrections.json");
        let hotwords = temp.path().join("hot.txt");
        let rules = temp.path().join("hot-rule.txt");
        fs::write(&corrections, "{}").unwrap();
        fs::write(
            &hotwords,
            "# aliases\nVoice IME | voice ime | 语音 IME\n非洲之星 | 非州之星\n",
        )
        .unwrap();
        fs::write(
            &rules,
            r"# regex rules
毫安时 = mAh
艾特\s*(\w+)\s*点\s*(\w+) = @\1.\2
",
        )
        .unwrap();

        assert_eq!(
            apply_corrections(
                "打开 voice ime，非州之星，一千毫安时，艾特qq点com",
                &corrections
            ),
            "打开 Voice IME，非洲之星，1000mAh，@qq.com"
        );
    }

    #[test]
    fn records_correction_trace_stages() {
        let temp = tempfile::tempdir().unwrap();
        let corrections = temp.path().join("corrections.json");
        let hotwords = temp.path().join("hot.txt");
        let rules = temp.path().join("hot-rule.txt");
        fs::write(&corrections, r#"{"mini CPM":"minicpm"}"#).unwrap();
        fs::write(&hotwords, "非洲之星 | 非州之星\n").unwrap();
        fs::write(&rules, "毫安时 = mAh\n").unwrap();

        let trace = correction_trace("mini CPM 非州之星 一千毫安时", &corrections);
        assert_eq!(trace.normalized_text, "mini CPM 非州之星 一千毫安时");
        assert_eq!(trace.dictionary_text, "minicpm 非州之星 一千毫安时");
        assert_eq!(trace.hotword_text, "minicpm 非洲之星 一千毫安时");
        assert_eq!(trace.rule_text, "minicpm 非洲之星 一千mAh");
        assert_eq!(trace.final_text, "minicpm 非洲之星 1000mAh");
    }

    #[test]
    fn reports_hotword_and_rule_stats() {
        let temp = tempfile::tempdir().unwrap();
        let hotwords = temp.path().join("hot.txt");
        let rules = temp.path().join("hot-rule.txt");
        fs::write(
            &hotwords,
            "Voice IME | voice ime | 语音 IME\n孤立目标词\n非洲之星 | 非州之星\n",
        )
        .unwrap();
        fs::write(&rules, "毫安时 = mAh\n( = bad\nbad line\n").unwrap();

        let stats = user_dictionary_stats(&hotwords, &rules);

        assert_eq!(stats.hotword_entries, 3);
        assert_eq!(stats.hotword_aliases, 3);
        assert_eq!(stats.hotword_entries_without_alias, 1);
        assert_eq!(stats.hot_rule_count, 1);
        assert_eq!(stats.hot_rule_invalid, 2);
        assert_eq!(stats.hot_rule_invalid_examples.len(), 2);
    }

    #[test]
    fn applies_punctuation_policy() {
        assert_eq!(
            apply_punctuation_policy("好的。", "short-no-period"),
            "好的"
        );
        assert_eq!(
            apply_punctuation_policy("你确定吗？", "short-no-period"),
            "你确定吗？"
        );
        assert_eq!(apply_punctuation_policy("好的。", "keep"), "好的。");
    }

    #[test]
    fn detects_prompt_leak() {
        assert!(looks_like_prompt_leak(
            "好的，我将按照您的要求进行处理。请确认以下内容：1. 是否需要我整理个人词表。2. 是否需要我处理 ASR 文本。"
        ));
        assert!(!looks_like_prompt_leak("今天下午三点开会，记得带电脑。"));
    }

    #[test]
    fn cleans_translation_labels_and_explanations() {
        assert_eq!(
            clean_translation_output("翻译结果：非洲之星和海洋之泪\n解释：这是意译。"),
            "非洲之星和海洋之泪"
        );
        assert_eq!(
            clean_translation_output("以下是翻译结果：\n1. The Star of Africa and the Tear of the Ocean\n说明：保留诗意。"),
            "The Star of Africa and the Tear of the Ocean"
        );
        assert_eq!(
            clean_translation_output("翻译结果如下：\n非洲之星和海洋之泪"),
            "非洲之星和海洋之泪"
        );
        assert_eq!(
            clean_translation_output("\"翻译结果：非洲之星和海洋之泪\""),
            "非洲之星和海洋之泪"
        );
        assert_eq!(
            clean_translation_output("中文：非洲之星和海洋之泪\n如果需要更诗意的翻译，可以调整。"),
            "非洲之星和海洋之泪"
        );
        assert_eq!(
            clean_translation_output(
                "The phrase can be translated as: The Star of Africa and the Tear of the Ocean"
            ),
            "The Star of Africa and the Tear of the Ocean"
        );
        assert!(!looks_like_translation_chatter("这是一本说明书。"));
        assert!(looks_like_translation_chatter("说明：这是意译。"));
        assert!(looks_like_translation_chatter(
            "如果需要，我可以根据语境继续调整翻译。"
        ));
        assert!(has_translation_markup("翻译结果：非洲之星和海洋之泪"));
        assert!(has_translation_markup(
            "中文：非洲之星和海洋之泪\n说明：这是意译。"
        ));
        assert!(!has_translation_markup("这是一本说明书。"));
    }

    #[test]
    fn detects_likely_chinese_text() {
        assert!(is_likely_chinese_text("非洲之星和海洋之泪"));
        assert!(!is_likely_chinese_text("アフリカの星と海の涙"));
        assert!(!is_likely_chinese_text("The Star of Africa"));
    }

    #[test]
    fn detects_code_commands_and_paths() {
        assert!(looks_like_code_command_or_path("cargo test -- --nocapture"));
        assert!(looks_like_code_command_or_path(
            r#"D:\voice-ime-build-rust\src-tauri\Cargo.toml"#
        ));
        assert!(looks_like_code_command_or_path(
            "./packaging/package-portable.ps1"
        ));
        assert!(looks_like_code_command_or_path(
            "fn main() { println!(\"hi\"); }"
        ));
        assert!(looks_like_code_command_or_path(
            "https://github.com/ap1529921128-design/voice-ime-rust-2.0.1"
        ));

        assert!(!looks_like_code_command_or_path(
            "今天下午三点开会，记得带电脑。"
        ));
        assert!(!looks_like_code_command_or_path("打开设置里面的路径配置"));
    }
}
