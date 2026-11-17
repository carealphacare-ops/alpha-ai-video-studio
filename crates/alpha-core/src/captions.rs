//! Transcript words -> caption lines -> SRT / VTT / ASS (word-highlight) files.
//! Pure logic, no processes. Times are seconds.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Word {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Transcript {
    pub language: String,
    pub words: Vec<Word>,
}

#[derive(Debug, Clone)]
pub struct Line {
    pub words: Vec<Word>,
}

impl Line {
    pub fn start(&self) -> f64 {
        self.words.first().map(|w| w.start).unwrap_or(0.0)
    }
    pub fn end(&self) -> f64 {
        self.words.last().map(|w| w.end).unwrap_or(0.0)
    }
    pub fn text(&self) -> String {
        self.words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ")
    }
}

// ---------- parsing whisper.cpp JSON ----------

fn is_noise(text: &str) -> bool {
    let t = text.trim();
    // Whisper marks non-speech as "[BLANK_AUDIO]", "[MUSIC]" etc.
    t.is_empty() || (t.starts_with('[') && t.ends_with(']'))
}

/// Parses the file written by `whisper-cli -oj`. Each entry has `offsets.from/to` in milliseconds
/// and `text`. If an entry holds several words (when `-ml 1` was not honoured) the time is shared
/// between them in proportion to their length, so word timing is approximate in that case.
pub fn parse_whisper_json(json: &str) -> Result<Transcript, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("Whisper output is not valid JSON: {e}"))?;
    let entries = v["transcription"].as_array().ok_or("Whisper output has no 'transcription' list.")?;
    let language = v["result"]["language"].as_str().unwrap_or("").to_string();
    let mut words: Vec<Word> = Vec::new();
    for e in entries {
        let text = e["text"].as_str().unwrap_or("");
        if is_noise(text) {
            continue;
        }
        let (Some(from), Some(to)) = (e["offsets"]["from"].as_f64(), e["offsets"]["to"].as_f64()) else { continue };
        let (from, to) = (from / 1000.0, (to / 1000.0).max(from / 1000.0));
        // Whisper splits one word into several pieces (very common for Urdu/Hindi). A piece with no
        // leading space continues the previous word; a piece with a leading space starts a new one.
        let continues = !words.is_empty() && !text.starts_with(char::is_whitespace);
        let parts: Vec<&str> = text.split_whitespace().collect();
        let total: usize = parts.iter().map(|p| p.chars().count().max(1)).sum();
        let mut t = from;
        for (k, p) in parts.iter().enumerate() {
            let share = (to - from) * (p.chars().count().max(1) as f64 / total.max(1) as f64);
            if k == 0 && continues {
                if let Some(last) = words.last_mut() {
                    last.text.push_str(p);
                    last.end = t + share;
                }
            } else {
                words.push(Word { text: p.to_string(), start: t, end: t + share });
            }
            t += share;
        }
    }
    let words = collapse_repeats(words, 3);
    Ok(Transcript { language, words })
}

fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

/// Whisper sometimes gets stuck and repeats one word dozens of times. Keep at most `max_run` in a row.
pub fn collapse_repeats(words: Vec<Word>, max_run: usize) -> Vec<Word> {
    let mut out: Vec<Word> = Vec::new();
    let mut run = 0;
    for w in words {
        let same = out.last().map(|l| !norm(&l.text).is_empty() && norm(&l.text) == norm(&w.text)).unwrap_or(false);
        if same {
            run += 1;
            if run >= max_run {
                continue;
            }
        } else {
            run = 0;
        }
        out.push(w);
    }
    out
}

// ---------- grouping ----------

/// Splits words into caption lines: a new line starts after a sentence end, a long pause,
/// or when the line gets too many words / characters.
pub fn group_lines(words: &[Word], max_words: usize, max_chars: usize, max_gap: f64) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    let mut cur: Vec<Word> = Vec::new();
    let mut chars = 0;
    for w in words {
        let gap = cur.last().map(|l: &Word| w.start - l.end).unwrap_or(0.0);
        let ended = cur.last().map(|l| l.text.ends_with(['.', '!', '?', '؟', '۔'])).unwrap_or(false);
        let too_long = !cur.is_empty() && (cur.len() >= max_words || chars + w.text.chars().count() + 1 > max_chars);
        if !cur.is_empty() && (gap > max_gap || ended || too_long) {
            lines.push(Line { words: std::mem::take(&mut cur) });
            chars = 0;
        }
        chars += w.text.chars().count() + 1;
        cur.push(w.clone());
    }
    if !cur.is_empty() {
        lines.push(Line { words: cur });
    }
    lines
}

/// Keeps words inside [start, end] and moves them so the clip starts at 0.
pub fn clip_words(words: &[Word], start: f64, end: f64) -> Vec<Word> {
    words
        .iter()
        .filter(|w| w.end > start && w.start < end)
        .map(|w| Word { text: w.text.clone(), start: (w.start.max(start) - start), end: (w.end.min(end) - start) })
        .collect()
}

// ---------- SRT / VTT ----------

fn hms(t: f64, sep: char) -> String {
    let ms = (t.max(0.0) * 1000.0).round() as u64;
    format!("{:02}:{:02}:{:02}{}{:03}", ms / 3_600_000, (ms / 60_000) % 60, (ms / 1000) % 60, sep, ms % 1000)
}

pub fn to_srt(lines: &[Line]) -> String {
    let mut s = String::new();
    for (i, l) in lines.iter().enumerate() {
        s += &format!("{}\n{} --> {}\n{}\n\n", i + 1, hms(l.start(), ','), hms(l.end(), ','), l.text());
    }
    s
}

pub fn to_vtt(lines: &[Line]) -> String {
    let mut s = String::from("WEBVTT\n\n");
    for l in lines {
        s += &format!("{} --> {}\n{}\n\n", hms(l.start(), '.'), hms(l.end(), '.'), l.text());
    }
    s
}

// ---------- ASS (styled, word highlight) ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptionStyle {
    pub name: String,
    pub font: String,
    /// Font size as a fraction of the video height (0.05 = 5%), so it works at any resolution.
    pub size_pct: f64,
    pub text_color: String,
    pub highlight_color: String,
    pub outline_color: String,
    pub outline: f64,
    pub shadow: f64,
    /// ASS alignment: 2 = bottom centre, 5 = middle centre, 8 = top centre.
    pub alignment: u8,
    /// Distance from the edge, as a fraction of video height.
    pub margin_pct: f64,
    pub bold: bool,
    pub uppercase: bool,
    /// Make the spoken word slightly bigger.
    pub pop: bool,
    pub max_words: usize,
}

pub fn preset(name: &str) -> CaptionStyle {
    let base = CaptionStyle {
        name: "bold_yellow".into(),
        font: "Arial".into(),
        size_pct: 0.052,
        text_color: "#FFFFFF".into(),
        highlight_color: "#FFD60A".into(),
        outline_color: "#000000".into(),
        outline: 5.0,
        shadow: 1.0,
        alignment: 2,
        margin_pct: 0.18,
        bold: true,
        uppercase: true,
        pop: true,
        max_words: 3,
    };
    match name {
        "white_outline" => CaptionStyle { name: name.into(), highlight_color: "#FFFFFF".into(), pop: false, uppercase: false, max_words: 5, ..base },
        "karaoke" => CaptionStyle { name: name.into(), text_color: "#FFFFFF".into(), highlight_color: "#00E5FF".into(), uppercase: false, max_words: 5, ..base },
        "minimal" => CaptionStyle { name: name.into(), size_pct: 0.036, bold: false, outline: 2.0, highlight_color: "#FFFFFF".into(), pop: false, uppercase: false, max_words: 7, margin_pct: 0.08, ..base },
        "center_large" => CaptionStyle { name: name.into(), size_pct: 0.075, alignment: 5, margin_pct: 0.0, max_words: 2, ..base },
        "gaming" => CaptionStyle { name: name.into(), highlight_color: "#39FF14".into(), size_pct: 0.058, font: "Impact".into(), bold: false, ..base },
        _ => CaptionStyle { name: "bold_yellow".into(), ..base },
    }
}

pub const PRESET_NAMES: &[&str] = &["bold_yellow", "white_outline", "karaoke", "minimal", "center_large", "gaming"];

/// "#RRGGBB" -> ASS "&H00BBGGRR". Bad input falls back to white.
pub fn ass_color(hex: &str) -> String {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()) {
        format!("&H00{}{}{}", &h[4..6], &h[2..4], &h[0..2]).to_uppercase()
    } else {
        "&H00FFFFFF".into()
    }
}

fn ass_time(t: f64) -> String {
    let cs = (t.max(0.0) * 100.0).round() as u64;
    format!("{}:{:02}:{:02}.{:02}", cs / 360_000, (cs / 6000) % 60, (cs / 100) % 60, cs % 100)
}

fn ass_escape(s: &str) -> String {
    s.replace('\\', "/").replace('{', "(").replace('}', ")").replace('\n', " ")
}

pub fn to_ass(lines: &[Line], style: &CaptionStyle, width: u32, height: u32) -> String {
    let h = height as f64;
    let size = (h * style.size_pct).round().max(8.0);
    let margin_v = (h * style.margin_pct).round();
    let outline = (style.outline * h / 1920.0).max(1.0);
    let mut s = format!(
        "[Script Info]\nScriptType: v4.00+\nPlayResX: {width}\nPlayResY: {height}\nWrapStyle: 2\nScaledBorderAndShadow: yes\n\n\
[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
Style: Default,{font},{size},{pc},{pc},{oc},&H80000000,{bold},0,0,0,100,100,0,0,1,{outline:.1},{shadow:.1},{al},{ml},{ml},{mv},1\n\n\
[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
        font = style.font,
        pc = ass_color(&style.text_color),
        oc = ass_color(&style.outline_color),
        bold = if style.bold { -1 } else { 0 },
        shadow = style.shadow,
        al = style.alignment,
        ml = (width as f64 * 0.06).round(),
        mv = margin_v,
    );
    let hi = ass_color(&style.highlight_color);
    let base = ass_color(&style.text_color);
    for line in lines {
        let n = line.words.len();
        for i in 0..n {
            let start = line.words[i].start;
            // Show each word's version of the line until the next word starts, so there are no flickers.
            let end = if i + 1 < n { line.words[i + 1].start } else { line.end() };
            if end <= start {
                continue;
            }
            let mut text = String::new();
            for (j, w) in line.words.iter().enumerate() {
                let t = if style.uppercase { w.text.to_uppercase() } else { w.text.clone() };
                let t = ass_escape(&t);
                if j == i {
                    if style.pop {
                        text += &format!("{{\\c{hi}&\\fscx112\\fscy112}}{t}{{\\c{base}&\\fscx100\\fscy100}}");
                    } else {
                        text += &format!("{{\\c{hi}&}}{t}{{\\c{base}&}}");
                    }
                } else {
                    text += &t;
                }
                if j + 1 < n {
                    text.push(' ');
                }
            }
            s += &format!("Dialogue: 0,{},{},Default,,0,0,0,,{}\n", ass_time(start), ass_time(end), text);
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"result":{"language":"en"},"transcription":[
      {"offsets":{"from":0,"to":400},"text":" Hello"},
      {"offsets":{"from":400,"to":900},"text":" world."},
      {"offsets":{"from":900,"to":1500},"text":" [BLANK_AUDIO]"},
      {"offsets":{"from":3000,"to":4000},"text":" two words"}]}"#;

    #[test]
    fn parses_words_and_skips_noise() {
        let t = parse_whisper_json(SAMPLE).unwrap();
        assert_eq!(t.language, "en");
        let texts: Vec<_> = t.words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, ["Hello", "world.", "two", "words"]);
        assert!((t.words[2].start - 3.0).abs() < 1e-9);
        assert!((t.words[3].end - 4.0).abs() < 1e-9); // shared time ends at the entry end
        assert!(parse_whisper_json("nope").is_err());
    }

    #[test]
    fn groups_on_sentence_and_gap() {
        let t = parse_whisper_json(SAMPLE).unwrap();
        let lines = group_lines(&t.words, 5, 40, 0.7);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text(), "Hello world.");
        assert_eq!(lines[1].text(), "two words");
    }

    #[test]
    fn srt_and_vtt_format() {
        let t = parse_whisper_json(SAMPLE).unwrap();
        let lines = group_lines(&t.words, 5, 40, 0.7);
        let srt = to_srt(&lines);
        assert!(srt.starts_with("1\n00:00:00,000 --> 00:00:00,900\nHello world.\n"));
        assert!(to_vtt(&lines).starts_with("WEBVTT\n\n00:00:00.000 --> 00:00:00.900"));
    }

    #[test]
    fn ass_has_one_event_per_word_and_yellow_highlight() {
        let t = parse_whisper_json(SAMPLE).unwrap();
        let lines = group_lines(&t.words, 5, 40, 0.7);
        let ass = to_ass(&lines, &preset("bold_yellow"), 1080, 1920);
        assert_eq!(ass.matches("Dialogue:").count(), 4);
        assert!(ass.contains("PlayResX: 1080"));
        assert!(ass.contains("&H000AD6FF")); // #FFD60A in BGR order
        assert!(ass.contains("HELLO"));
    }

    #[test]
    fn colors() {
        assert_eq!(ass_color("#FFD60A"), "&H000AD6FF");
        assert_eq!(ass_color("junk"), "&H00FFFFFF");
    }

    #[test]
    fn clips_and_shifts() {
        let t = parse_whisper_json(SAMPLE).unwrap();
        let c = clip_words(&t.words, 0.5, 3.5);
        assert_eq!(c[0].text, "world.");
        assert!((c[0].start - 0.0).abs() < 1e-9);
        assert!((c.last().unwrap().end - 3.0).abs() < 1e-9);
    }

    #[test]
    fn joins_pieces_of_one_word() {
        let j = r#"{"transcription":[
          {"offsets":{"from":0,"to":200},"text":" \u0633\u0644"},
          {"offsets":{"from":200,"to":400},"text":"\u0627\u0645"},
          {"offsets":{"from":400,"to":900},"text":" \u062f\u0646\u06cc\u0627"},
          {"offsets":{"from":900,"to":950},"text":"."}]}"#;
        let t = parse_whisper_json(j).unwrap();
        assert_eq!(t.words.len(), 2);
        assert_eq!(t.words[0].text, "\u{633}\u{644}\u{627}\u{645}");
        assert!((t.words[0].end - 0.4).abs() < 1e-9);
        assert_eq!(t.words[1].text, "\u{62f}\u{646}\u{6cc}\u{627}.");
    }

    #[test]
    fn stops_repeat_loops() {
        let w = |t: &str, i: usize| Word { text: t.into(), start: i as f64, end: i as f64 + 0.5 };
        let mut v: Vec<Word> = (0..50).map(|i| w("good", i)).collect();
        v.push(w("end", 50));
        let c = collapse_repeats(v, 3);
        assert_eq!(c.len(), 4);
        assert_eq!(c.last().unwrap().text, "end");
    }
}
