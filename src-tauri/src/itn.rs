use once_cell::sync::Lazy;
use regex::{Captures, Regex};

static CN_NUM: &str = r"[负零〇一二两三四五六七八九十百千万亿]+";
static CN_DECIMAL: &str =
    r"[负零〇一二两三四五六七八九十百千万亿]+(?:点[零〇一二两三四五六七八九]+)?";

static PERCENT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(&format!(r"百分之({CN_DECIMAL})")).unwrap());
static MONEY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(
        r"({CN_NUM})(?:元|块)(?:(零|[一二两三四五六七八九])(?:角|毛))?(?:([一二两三四五六七八九])分?)?"
    ))
    .unwrap()
});
static DATE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(
        r"([零〇一二两三四五六七八九]{{4}})年({CN_NUM})月({CN_NUM})(日|号)"
    ))
    .unwrap()
});
static TIME_HALF_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"((?:上午|下午|晚上|早上|中午|凌晨)?)([零〇一二两三四五六七八九十]+)点半").unwrap()
});
static TIME_MINUTE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"((?:上午|下午|晚上|早上|中午|凌晨)?)([零〇一二两三四五六七八九十]+)点([零〇一二两三四五六七八九十]+)分",
    )
    .unwrap()
});
static TIME_PREFIX_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(上午|下午|晚上|早上|中午|凌晨)([零〇一二两三四五六七八九十]+)点([零〇一二两三四五六七八九十]+)",
    )
    .unwrap()
});
static RANGE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(&format!(r"({CN_DECIMAL})(?:到|至)({CN_DECIMAL})")).unwrap());
static DECIMAL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(&format!(r"({CN_NUM})点([零〇一二两三四五六七八九]+)")).unwrap());
static UNIT_NUMBER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(
        r"({CN_NUM})(秒|分钟|小时|天|次|个|米|公里|毫升|升|克|千克|公斤|斤|度|毫安时|mAh|MB|GB|TB|token|tokens)"
    ))
    .unwrap()
});

pub fn apply_itn(text: &str) -> String {
    let mut text = text.to_string();
    text = PERCENT_RE
        .replace_all(&text, |caps: &Captures| {
            parse_number(caps.get(1).unwrap().as_str())
                .map(|value| format!("{}%", value))
                .unwrap_or_else(|| caps.get(0).unwrap().as_str().to_string())
        })
        .to_string();
    text = MONEY_RE
        .replace_all(&text, |caps: &Captures| replace_money(caps))
        .to_string();
    text = DATE_RE
        .replace_all(&text, |caps: &Captures| replace_date(caps))
        .to_string();
    text = TIME_HALF_RE
        .replace_all(&text, |caps: &Captures| replace_time(caps, "30"))
        .to_string();
    text = TIME_MINUTE_RE
        .replace_all(&text, |caps: &Captures| {
            replace_time(caps, caps.get(3).unwrap().as_str())
        })
        .to_string();
    text = TIME_PREFIX_RE
        .replace_all(&text, |caps: &Captures| {
            replace_time(caps, caps.get(3).unwrap().as_str())
        })
        .to_string();
    text = RANGE_RE
        .replace_all(&text, |caps: &Captures| {
            let left = parse_number(caps.get(1).unwrap().as_str());
            let right = parse_number(caps.get(2).unwrap().as_str());
            match (left, right) {
                (Some(left), Some(right)) => format!("{left}-{right}"),
                _ => caps.get(0).unwrap().as_str().to_string(),
            }
        })
        .to_string();
    text = DECIMAL_RE
        .replace_all(&text, |caps: &Captures| {
            let integer = parse_integer(caps.get(1).unwrap().as_str());
            let decimal = parse_digit_sequence(caps.get(2).unwrap().as_str());
            match (integer, decimal) {
                (Some(integer), Some(decimal)) => format!("{integer}.{decimal}"),
                _ => caps.get(0).unwrap().as_str().to_string(),
            }
        })
        .to_string();
    UNIT_NUMBER_RE
        .replace_all(&text, |caps: &Captures| {
            parse_integer(caps.get(1).unwrap().as_str())
                .map(|value| format!("{}{}", value, caps.get(2).unwrap().as_str()))
                .unwrap_or_else(|| caps.get(0).unwrap().as_str().to_string())
        })
        .to_string()
}

fn replace_money(caps: &Captures) -> String {
    let Some(yuan) = parse_integer(caps.get(1).unwrap().as_str()) else {
        return caps.get(0).unwrap().as_str().to_string();
    };
    let jiao = caps
        .get(2)
        .and_then(|value| digit_value(value.as_str().chars().next().unwrap()))
        .unwrap_or(0);
    let fen = caps
        .get(3)
        .and_then(|value| digit_value(value.as_str().chars().next().unwrap()))
        .unwrap_or(0);
    if jiao == 0 && fen == 0 {
        format!("{yuan}元")
    } else {
        format!("{}.{:01}{:01}元", yuan, jiao, fen)
    }
}

fn replace_date(caps: &Captures) -> String {
    let year = parse_digit_sequence(caps.get(1).unwrap().as_str());
    let month = parse_integer(caps.get(2).unwrap().as_str());
    let day = parse_integer(caps.get(3).unwrap().as_str());
    match (year, month, day) {
        (Some(year), Some(month), Some(day)) => format!("{year}年{month}月{day}日"),
        _ => caps.get(0).unwrap().as_str().to_string(),
    }
}

fn replace_time(caps: &Captures, minute_text: &str) -> String {
    let prefix = caps.get(1).map(|item| item.as_str()).unwrap_or("");
    let hour = parse_integer(caps.get(2).unwrap().as_str());
    let minute = if minute_text == "30" {
        Some(30)
    } else {
        parse_integer(minute_text)
    };
    match (hour, minute) {
        (Some(hour), Some(minute)) if hour <= 24 && minute < 60 => {
            format!("{prefix}{hour}:{minute:02}")
        }
        _ => caps.get(0).unwrap().as_str().to_string(),
    }
}

fn parse_number(text: &str) -> Option<String> {
    if let Some((integer, decimal)) = text.split_once('点') {
        let integer = parse_integer(integer)?;
        let decimal = parse_digit_sequence(decimal)?;
        Some(format!("{integer}.{decimal}"))
    } else {
        parse_integer(text).map(|value| value.to_string())
    }
}

fn parse_integer(text: &str) -> Option<i64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let (negative, text) = text
        .strip_prefix('负')
        .map_or((false, text), |rest| (true, rest));
    if text.is_empty() {
        return None;
    }
    let value = if text.chars().all(|ch| digit_value(ch).is_some()) && text.chars().count() > 1 {
        parse_digit_sequence(text)?.parse::<i64>().ok()?
    } else {
        parse_unit_integer(text)?
    };
    Some(if negative { -value } else { value })
}

fn parse_unit_integer(text: &str) -> Option<i64> {
    let mut total = 0i64;
    let mut section = 0i64;
    let mut number: Option<i64> = None;
    let mut used = false;
    for ch in text.chars() {
        if let Some(value) = digit_value(ch) {
            number = Some(value);
            used = true;
            continue;
        }
        match ch {
            '十' | '百' | '千' => {
                let unit = small_unit(ch)?;
                section += number.unwrap_or(1) * unit;
                number = None;
                used = true;
            }
            '万' | '亿' => {
                let unit = large_unit(ch)?;
                section += number.take().unwrap_or(0);
                total += section.max(1) * unit;
                section = 0;
                used = true;
            }
            _ => return None,
        }
    }
    Some(total + section + number.unwrap_or(0)).filter(|_| used)
}

fn parse_digit_sequence(text: &str) -> Option<String> {
    let mut out = String::new();
    for ch in text.chars() {
        out.push(char::from_digit(digit_value(ch)? as u32, 10)?);
    }
    Some(out)
}

fn digit_value(ch: char) -> Option<i64> {
    match ch {
        '零' | '〇' => Some(0),
        '一' => Some(1),
        '二' | '两' => Some(2),
        '三' => Some(3),
        '四' => Some(4),
        '五' => Some(5),
        '六' => Some(6),
        '七' => Some(7),
        '八' => Some(8),
        '九' => Some(9),
        _ => None,
    }
}

fn small_unit(ch: char) -> Option<i64> {
    match ch {
        '十' => Some(10),
        '百' => Some(100),
        '千' => Some(1000),
        _ => None,
    }
}

fn large_unit(ch: char) -> Option<i64> {
    match ch {
        '万' => Some(10_000),
        '亿' => Some(100_000_000),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_common_numbers() {
        assert_eq!(apply_itn("一百二十三点四五"), "123.45");
        assert_eq!(apply_itn("一千毫安时"), "1000毫安时");
        assert_eq!(apply_itn("三到五个"), "3-5个");
    }

    #[test]
    fn converts_percent_money_date_and_time() {
        assert_eq!(apply_itn("百分之十二点五"), "12.5%");
        assert_eq!(apply_itn("一百二十三块四毛五"), "123.45元");
        assert_eq!(apply_itn("二零二六年六月五号"), "2026年6月5日");
        assert_eq!(apply_itn("下午三点半开会"), "下午3:30开会");
        assert_eq!(apply_itn("上午十点二十分"), "上午10:20");
    }
}
