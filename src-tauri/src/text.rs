use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};

static SPACE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[ \t]+").unwrap());
static MANY_NEWLINES_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\n{3,}").unwrap());
static THINK_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)<think>.*?</think>\s*").unwrap());
static PUNCT_ONLY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^[\s,.;:!?，。！？；：、…~～\-_'"“”‘’()\[\]{}【】<>《》「」『』]+$"#).unwrap()
});

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
    let mut text = normalize_text(text);
    for (wrong, right) in load_corrections(corrections_path) {
        text = text.replace(&wrong, &right);
    }
    normalize_text(&text)
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
    fn detects_prompt_leak() {
        assert!(looks_like_prompt_leak(
            "好的，我将按照您的要求进行处理。请确认以下内容：1. 是否需要我整理个人词表。2. 是否需要我处理 ASR 文本。"
        ));
        assert!(!looks_like_prompt_leak("今天下午三点开会，记得带电脑。"));
    }
}
