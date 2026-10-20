//! Deterministic rule-based English letter-to-sound fallback for out-of-vocabulary
//! words. Rules are hand written and approximate; callers must treat the output as
//! low confidence (names, slang and loanwords will often be wrong).
use super::Phoneme;

fn p(s: &str) -> Phoneme {
    Phoneme::parse(s).expect("valid ARPAbet in g2p rule table")
}

fn is_vowel(b: u8) -> bool {
    matches!(b, b'a' | b'e' | b'i' | b'o' | b'u')
}

const DH_WORDS: &[&str] = &[
    "the", "this", "that", "these", "those", "them", "then", "there", "they", "than", "though",
    "thus",
];

/// Pronounce `word` (ASCII letters and apostrophes) by rule. Always returns at least
/// one phoneme for a word containing a letter; returns an empty vector otherwise.
pub fn letter_to_sound(word: &str) -> Vec<Phoneme> {
    let w: Vec<u8> = word
        .bytes()
        .filter(u8::is_ascii_alphabetic)
        .map(|b| b.to_ascii_lowercase())
        .collect();
    let n = w.len();
    if n == 0 {
        return Vec::new();
    }
    let vowel_count = w.iter().filter(|b| is_vowel(**b) || **b == b'y').count();
    let silent_e = n >= 3 && w[n - 1] == b'e' && vowel_count >= 2 && !is_vowel(w[n - 2]);
    let end = if silent_e { n - 1 } else { n };
    let magic = |i: usize| silent_e && i + 2 == n - 1 && !is_vowel(w[i + 1]) && w[i + 1] != b'y';
    let mut out: Vec<Phoneme> = Vec::new();
    let mut i = 0;
    let starts = |i: usize, s: &str| w[i..end].starts_with(s.as_bytes());
    while i < end {
        let c = w[i];
        let next = if i + 1 < end { w[i + 1] } else { 0 };
        let soft_next = matches!(next, b'e' | b'i' | b'y');
        let mut take = 1;
        match c {
            b't' if starts(i, "tch") => {
                out.push(p("CH"));
                take = 3;
            }
            b't' if starts(i, "tion") => {
                out.extend([p("SH"), p("AH"), p("N")]);
                take = 4;
            }
            b's' if starts(i, "sion") => {
                out.extend([p("ZH"), p("AH"), p("N")]);
                take = 4;
            }
            b'c' if starts(i, "cian") || starts(i, "cial") => {
                out.extend([p("SH"), p("AH")]);
                take = 3;
            }
            b's' if starts(i, "sch") => {
                out.extend([p("S"), p("K")]);
                take = 3;
            }
            b's' if starts(i, "sh") => {
                out.push(p("SH"));
                take = 2;
            }
            b'c' if starts(i, "ch") => {
                out.push(p("CH"));
                take = 2;
            }
            b't' if starts(i, "th") => {
                let dh = i == 0 && DH_WORDS.contains(&std::str::from_utf8(&w).unwrap_or(""));
                out.push(p(if dh { "DH" } else { "TH" }));
                take = 2;
            }
            b'p' if starts(i, "ph") => {
                out.push(p("F"));
                take = 2;
            }
            b'c' if starts(i, "ck") => {
                out.push(p("K"));
                take = 2;
            }
            b'd' if next == b'g' && i + 2 < n && matches!(w[i + 2], b'e' | b'i' | b'y') => {
                out.push(p("JH"));
                take = 2;
            }
            b'g' if next == b'h' => {
                if i == 0 {
                    out.push(p("G"));
                }
                take = 2;
            }
            b'n' if starts(i, "ng") => {
                out.push(p("NG"));
                take = 2;
            }
            b'n' if starts(i, "nk") => {
                out.extend([p("NG"), p("K")]);
                take = 2;
            }
            b'w' if next == b'h' => {
                out.push(p("W"));
                take = 2;
            }
            b'k' if i == 0 && next == b'n' => {}
            b'w' if i == 0 && next == b'r' => {}
            b'g' if i == 0 && next == b'n' => {}
            b'b' if i + 1 == end && i > 0 && w[i - 1] == b'm' => {}
            b'q' => {
                out.extend([p("K"), p("W")]);
                if next == b'u' {
                    take = 2;
                }
            }
            b'x' => {
                if i == 0 {
                    out.push(p("Z"));
                } else {
                    out.extend([p("K"), p("S")]);
                }
            }
            b'c' => out.push(p(if soft_next { "S" } else { "K" })),
            b'g' => out.push(p(if soft_next { "JH" } else { "G" })),
            b'j' => out.push(p("JH")),
            b's' => {
                let prev = if i > 0 { w[i - 1] } else { 0 };
                let voiced_prev = is_vowel(prev)
                    || matches!(
                        prev,
                        b'b' | b'd' | b'g' | b'l' | b'm' | b'n' | b'r' | b'v' | b'z'
                    );
                let z = if i + 1 == end {
                    voiced_prev && i > 0
                } else {
                    i > 0 && is_vowel(prev) && is_vowel(next)
                };
                out.push(p(if z { "Z" } else { "S" }));
            }
            b'y' => {
                if i == 0 && is_vowel(next) {
                    out.push(p("Y"));
                } else if i + 1 == end {
                    out.push(p(if vowel_count > 1 { "IY" } else { "AY" }));
                } else {
                    out.push(p("IH"));
                }
            }
            b'b' => out.push(p("B")),
            b'd' => out.push(p("D")),
            b'f' => out.push(p("F")),
            b'h' => out.push(p("HH")),
            b'k' => out.push(p("K")),
            b'l' => out.push(p("L")),
            b'm' => out.push(p("M")),
            b'n' => out.push(p("N")),
            b'p' => out.push(p("P")),
            b'r' => out.push(p("R")),
            b't' => out.push(p("T")),
            b'v' => out.push(p("V")),
            b'w' => out.push(p("W")),
            b'z' => out.push(p("Z")),
            // Vowels.
            b'i' if starts(i, "igh") => {
                out.push(p("AY"));
                take = 3;
            }
            b'o' if starts(i, "oo") => {
                let short = i + 2 < end && matches!(w[i + 2], b'k');
                out.push(p(if short { "UH" } else { "UW" }));
                take = 2;
            }
            b'e' if starts(i, "ee") || starts(i, "ea") => {
                out.push(p("IY"));
                take = 2;
            }
            b'a' if starts(i, "ai") || starts(i, "ay") => {
                out.push(p("EY"));
                take = 2;
            }
            b'e' if starts(i, "ei") || starts(i, "ey") => {
                out.push(p("EY"));
                take = 2;
            }
            b'o' if starts(i, "oa") => {
                out.push(p("OW"));
                take = 2;
            }
            b'o' if starts(i, "ow") => {
                let long = i + 2 == end || starts(i, "own");
                out.push(p(if long { "OW" } else { "AW" }));
                take = 2;
            }
            b'o' if starts(i, "ou") => {
                out.push(p("AW"));
                take = 2;
            }
            b'o' if starts(i, "oi") || starts(i, "oy") => {
                out.push(p("OY"));
                take = 2;
            }
            b'a' if starts(i, "au") || starts(i, "aw") => {
                out.push(p("AO"));
                take = 2;
            }
            b'e' if starts(i, "ew") || starts(i, "eu") => {
                out.extend([p("Y"), p("UW")]);
                take = 2;
            }
            b'u' if starts(i, "ue") => {
                out.push(p("UW"));
                take = 2;
            }
            b'i' if starts(i, "ie") => {
                out.push(p("IY"));
                take = 2;
            }
            b'a' if next == b'r' => {
                out.extend([p("AA"), p("R")]);
                take = 2;
            }
            b'o' if next == b'r' => {
                out.extend([p("AO"), p("R")]);
                take = 2;
            }
            b'e' | b'i' | b'u' if next == b'r' => {
                out.extend([p("ER")]);
                take = 2;
            }
            b'a' | b'e' | b'i' | b'o' | b'u' => {
                let single_word = n == 1;
                let last = i + 1 == end;
                let phone = if magic(i) {
                    match c {
                        b'a' => "EY",
                        b'e' => "IY",
                        b'i' => "AY",
                        b'o' => "OW",
                        _ => "UW",
                    }
                } else if single_word && c == b'a' {
                    "AH"
                } else if last && n > 1 {
                    match c {
                        b'a' => "AH",
                        b'e' => "IY",
                        b'i' => "IY",
                        b'o' => "OW",
                        _ => "UW",
                    }
                } else {
                    match c {
                        b'a' => "AE",
                        b'e' => "EH",
                        b'i' => "IH",
                        b'o' => "AA",
                        _ => "AH",
                    }
                };
                out.push(p(phone));
            }
            _ => {}
        }
        // Skip a doubled consonant letter.
        if !is_vowel(c) && take == 1 && i + 1 < end && w[i + 1] == c {
            take = 2;
        }
        i += take;
    }
    if out.is_empty() {
        out.push(p("AH"));
    }
    out
}
