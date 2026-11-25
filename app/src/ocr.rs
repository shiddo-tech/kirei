use std::sync::OnceLock;

use regex::Regex;
use tiny_skia::Pixmap;

use crate::model::Rect;

/// All recognized text, grouped into lines by vertical position.
pub fn text(pm: &Pixmap) -> String {
    #[cfg(target_os = "macos")]
    {
        let rec = crate::platform::vision::recognize(pm);
        let mut obs: Vec<(String, [f64; 4])> =
            rec.lines.iter().map(|l| (l.text.clone(), l.bbox)).collect();
        group_lines(&mut obs)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pm;
        String::new()
    }
}

fn group_lines(obs: &mut [(String, [f64; 4])]) -> String {
    let mid = |b: &[f64; 4]| b[1] + b[3] / 2.;
    obs.sort_by(|a, b| {
        let (ay, by) = (mid(&a.1), mid(&b.1));
        if (ay - by).abs() > a.1[3].min(b.1[3]) * 0.5 {
            by.partial_cmp(&ay).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            a.1[0]
                .partial_cmp(&b.1[0])
                .unwrap_or(std::cmp::Ordering::Equal)
        }
    });
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut last: Option<(f64, f64)> = None;
    for (s, b) in obs.iter() {
        let y = mid(b);
        match last {
            Some((ly, lh)) if (ly - y).abs() <= lh.min(b[3]) * 0.5 => {
                current.push_str("  ");
                current.push_str(s);
            }
            Some(_) => {
                lines.push(std::mem::take(&mut current));
                current = s.clone();
            }
            None => current = s.clone(),
        }
        last = Some((y, b[3]));
    }
    if last.is_some() {
        lines.push(current);
    }
    lines.join("\n")
}

fn patterns() -> &'static [Regex] {
    static P: OnceLock<Vec<Regex>> = OnceLock::new();
    P.get_or_init(|| {
        [
            r"(?i)[A-Z0-9._%+\-]+@[A-Z0-9.\-]+\.[A-Z]{2,}",
            r"\b(?:\d{1,3}\.){3}\d{1,3}\b",
            r"\b\d{3}\.?\d{3}\.?\d{3}-?\d{2}\b",
            r"\b\d{2}\.?\d{3}\.?\d{3}/?\d{4}-?\d{2}\b",
            r"(?i)\b(?:sk|pk|rk)[-_](?:live_|test_|proj-|ant-)?[A-Za-z0-9_\-]{16,}",
            r"(?i)\bAKIA[0-9A-Z]{16}\b",
            r"(?i)\bgh[pousr]_[A-Za-z0-9]{20,}\b",
            r"(?i)\bxox[abprs]-[A-Za-z0-9\-]{10,}",
            r"(?i)\bAIza[0-9A-Za-z_\-]{30,}",
            r"(?i)\beyJ[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{4,}",
        ]
        .iter()
        .map(|p| Regex::new(p).expect("regex"))
        .collect()
    })
}

/// Long opaque secrets: 32+ word characters with at least one letter and one digit.
fn opaque_secret() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\b[A-Za-z0-9_\-]{32,}\b").expect("regex"))
}

fn card() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\b(?:\d[ \-]?){12,18}\d\b").expect("regex"))
}

pub fn luhn(s: &str) -> bool {
    let digits: Vec<u32> = s.chars().filter_map(|c| c.to_digit(10)).collect();
    if !(13..=19).contains(&digits.len()) {
        return false;
    }
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            if i % 2 == 1 {
                let x = d * 2;
                if x > 9 { x - 9 } else { x }
            } else {
                d
            }
        })
        .sum();
    sum % 10 == 0
}

/// Byte ranges of sensitive data in a line of text.
pub fn sensitive_ranges(s: &str) -> Vec<std::ops::Range<usize>> {
    let mut out: Vec<std::ops::Range<usize>> = Vec::new();
    for re in patterns() {
        out.extend(re.find_iter(s).map(|m| m.range()));
    }
    out.extend(opaque_secret().find_iter(s).filter_map(|m| {
        let t = m.as_str();
        (t.chars().any(|c| c.is_ascii_digit()) && t.chars().any(|c| c.is_ascii_alphabetic()))
            .then(|| m.range())
    }));
    out.extend(
        card()
            .find_iter(s)
            .filter(|m| luhn(m.as_str()))
            .map(|m| m.range()),
    );
    out
}

fn utf16_offset(s: &str, byte: usize) -> usize {
    s[..byte].encode_utf16().count()
}

/// Rects (source pixels, y down) covering sensitive text.
pub fn sensitive_rects(pm: &Pixmap) -> Vec<Rect> {
    #[cfg(target_os = "macos")]
    {
        let (w, h) = (pm.width() as f64, pm.height() as f64);
        let rec = crate::platform::vision::recognize(pm);
        let mut rects = Vec::new();
        for line in &rec.lines {
            let s = &line.text;
            let mut ranges: Vec<(usize, usize)> = sensitive_ranges(s)
                .into_iter()
                .map(|r| {
                    let a = utf16_offset(s, r.start);
                    (a, utf16_offset(s, r.end) - a)
                })
                .collect();
            let units: Vec<u16> = s.encode_utf16().collect();
            ranges.extend(
                crate::platform::vision::phone_ranges(s)
                    .into_iter()
                    .filter(|(a, n)| {
                        let sub = String::from_utf16_lossy(&units[*a..(*a + *n).min(units.len())]);
                        sub.chars().filter(|c| c.is_numeric()).count() >= 8
                    }),
            );
            for (start, len) in ranges {
                let Some(b) = line.range_box(start, len) else {
                    continue;
                };
                let r = Rect::new(b[0] * w, (1. - b[1] - b[3]) * h, b[2] * w, b[3] * h);
                let pad = r.h * 0.2;
                rects.push(r.inset(-pad, -pad));
            }
        }
        rects
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pm;
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_sensitive_data() {
        let found = |s: &str| !sensitive_ranges(s).is_empty();
        assert!(found("contato: maria@exemplo.com.br"));
        assert!(found("CPF 123.456.789-09"));
        assert!(found("CNPJ 12.345.678/0001-95"));
        assert!(found("cartão 4111 1111 1111 1111"));
        assert!(found("ip 192.168.0.1"));
        assert!(found("key sk-proj-abcdefghijklmnopqrstuv"));
        assert!(!found("cartão 4111 1111 1111 1112"));
        assert!(!found("Olá Kirei, tudo bem?"));
    }

    #[test]
    fn groups_lines_top_to_bottom() {
        let mut obs = vec![
            ("b".to_string(), [0.5, 0.8, 0.1, 0.05]),
            ("c".to_string(), [0.1, 0.5, 0.1, 0.05]),
            ("a".to_string(), [0.1, 0.8, 0.1, 0.05]),
        ];
        assert_eq!(group_lines(&mut obs), "a  b\nc");
    }
}
