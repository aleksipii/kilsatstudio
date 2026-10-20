//! Deterministic text normalisation: written English to lowercase spoken words.
use super::lexicon::Lexicon;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pause {
    /// Comma, semicolon, colon, dash.
    Short,
    /// Sentence end, ellipsis, newline.
    Long,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    /// Lowercase ASCII letters and inner apostrophes only.
    Word(String),
    Pause(Pause),
}

const ONES: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
const TENS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];
const LETTER_NAMES: [&str; 26] = [
    "ay",
    "bee",
    "see",
    "dee",
    "ee",
    "eff",
    "gee",
    "aitch",
    "eye",
    "jay",
    "kay",
    "el",
    "em",
    "en",
    "oh",
    "pee",
    "cue",
    "ar",
    "ess",
    "tee",
    "you",
    "vee",
    "doubleyou",
    "ex",
    "why",
    "zee",
];
const ABBREVIATIONS: &[(&str, &str)] = &[
    ("mr", "mister"),
    ("mrs", "missus"),
    ("ms", "miss"),
    ("dr", "doctor"),
    ("prof", "professor"),
    ("sr", "senior"),
    ("jr", "junior"),
    ("vs", "versus"),
    ("etc", "et cetera"),
    ("approx", "approximately"),
    ("govt", "government"),
    ("dept", "department"),
    ("ok", "okay"),
];
/// Units spoken only directly after a number ("5kg", "10 ms").
const UNITS: &[(&str, &str)] = &[
    ("kg", "kilograms"),
    ("g", "grams"),
    ("km", "kilometers"),
    ("m", "meters"),
    ("cm", "centimeters"),
    ("mm", "millimeters"),
    ("ms", "milliseconds"),
    ("s", "seconds"),
    ("hz", "hertz"),
    ("khz", "kilohertz"),
    ("mb", "megabytes"),
    ("gb", "gigabytes"),
    ("am", "a m"),
    ("pm", "p m"),
    ("k", "thousand"),
    ("x", "times"),
];

/// Hard bound on produced tokens so adversarial text cannot exhaust memory.
pub const MAX_TOKENS: usize = 20_000;

fn fold(c: char) -> char {
    match c {
        '\u{2019}' | '\u{2018}' | '`' => '\'',
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ý' | 'ÿ' => 'y',
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
        'È' | 'É' | 'Ê' | 'Ë' => 'E',
        'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'O',
        'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
        'Ç' => 'C',
        'Ñ' => 'N',
        other => other,
    }
}

/// Spoken words for a non-negative integer below 10^12 (larger values are read digit by digit).
pub fn cardinal_words(n: u64) -> Vec<String> {
    let mut out = Vec::new();
    cardinal_into(n, &mut out);
    out
}

fn cardinal_into(n: u64, out: &mut Vec<String>) {
    let w = |s: &str| s.to_string();
    if n < 20 {
        out.push(w(ONES[n as usize]));
    } else if n < 100 {
        out.push(w(TENS[(n / 10) as usize]));
        if !n.is_multiple_of(10) {
            out.push(w(ONES[(n % 10) as usize]));
        }
    } else {
        let (unit, name) = match n {
            0..=999 => (100, "hundred"),
            1_000..=999_999 => (1_000, "thousand"),
            1_000_000..=999_999_999 => (1_000_000, "million"),
            _ => (1_000_000_000, "billion"),
        };
        cardinal_into(n / unit, out);
        out.push(w(name));
        if !n.is_multiple_of(unit) {
            cardinal_into(n % unit, out);
        }
    }
}

fn digits_words(digits: &str) -> Vec<String> {
    digits
        .bytes()
        .map(|b| ONES[(b - b'0') as usize].to_string())
        .collect()
}

fn ordinalize(words: &mut [String]) {
    if let Some(last) = words.last_mut() {
        *last = match last.as_str() {
            "one" => "first".into(),
            "two" => "second".into(),
            "three" => "third".into(),
            "five" => "fifth".into(),
            "eight" => "eighth".into(),
            "nine" => "ninth".into(),
            "twelve" => "twelfth".into(),
            s if s.ends_with('y') => format!("{}ieth", &s[..s.len() - 1]),
            s => format!("{s}th"),
        };
    }
}

fn pluralize(words: &mut [String]) {
    if let Some(last) = words.last_mut() {
        *last = if last.ends_with('y') {
            format!("{}ies", &last[..last.len() - 1])
        } else {
            format!("{last}s")
        };
    }
}

fn year_words(n: u64) -> Vec<String> {
    let mut out = Vec::new();
    if (2000..2010).contains(&n) {
        cardinal_into(n, &mut out);
        return out;
    }
    let (hi, lo) = (n / 100, n % 100);
    cardinal_into(hi, &mut out);
    match lo {
        0 => out.push("hundred".into()),
        1..=9 => {
            out.push("oh".into());
            cardinal_into(lo, &mut out);
        }
        _ => cardinal_into(lo, &mut out),
    }
    out
}

struct Scanner<'a> {
    chars: Vec<char>,
    i: usize,
    tokens: Vec<Token>,
    pending: Option<Pause>,
    lexicon: &'a Lexicon,
}

impl<'a> Scanner<'a> {
    fn peek(&self, k: usize) -> Option<char> {
        self.chars.get(self.i + k).copied()
    }

    fn push_word(&mut self, w: impl Into<String>) {
        if self.tokens.len() >= MAX_TOKENS {
            return;
        }
        if let Some(p) = self.pending.take() {
            if !self.tokens.is_empty() {
                self.tokens.push(Token::Pause(p));
            }
        }
        let w = w.into();
        for part in w.split_whitespace() {
            self.tokens.push(Token::Word(part.to_string()));
        }
    }

    fn push_words(&mut self, words: Vec<String>) {
        for w in words {
            self.push_word(w);
        }
    }

    fn pause(&mut self, p: Pause) {
        self.pending = Some(self.pending.map_or(p, |q| q.max(p)));
    }

    fn read_digits(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek(0).filter(char::is_ascii_digit) {
            s.push(c);
            self.i += 1;
        }
        s
    }

    fn letters_at(&self, mut at: usize) -> String {
        let mut s = String::new();
        while let Some(c) = self
            .chars
            .get(at)
            .copied()
            .filter(char::is_ascii_alphabetic)
        {
            s.push(c.to_ascii_lowercase());
            at += 1;
        }
        s
    }

    /// Reads a number starting at a digit; `currency` is the unit noun for `$`, `£`, `€`.
    fn number(&mut self, currency: Option<(&str, &str)>, negative: bool) {
        let mut int = self.read_digits();
        let mut plain = true;
        // Thousands separators: ",ddd" groups only.
        while self.peek(0) == Some(',')
            && (1..=3).all(|k| self.peek(k).is_some_and(|c| c.is_ascii_digit()))
            && !self.peek(4).is_some_and(|c| c.is_ascii_digit())
        {
            self.i += 1;
            int.push_str(&self.read_digits());
            plain = false;
        }
        let mut frac = String::new();
        if self.peek(0) == Some('.') && self.peek(1).is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
            frac = self.read_digits();
            plain = false;
        }
        // Clock time "10:30".
        if plain
            && int.len() <= 2
            && self.peek(0) == Some(':')
            && self.peek(1).is_some_and(|c| c.is_ascii_digit())
            && self.peek(2).is_some_and(|c| c.is_ascii_digit())
            && !self.peek(3).is_some_and(|c| c.is_ascii_digit())
        {
            let minutes: u64 = format!("{}{}", self.chars[self.i + 1], self.chars[self.i + 2])
                .parse()
                .unwrap_or(0);
            self.i += 3;
            let mut words = cardinal_words(int.parse().unwrap_or(0));
            match minutes {
                0 => words.push("o'clock".into()),
                1..=9 => {
                    words.push("oh".into());
                    cardinal_into(minutes, &mut words);
                }
                _ => cardinal_into(minutes, &mut words),
            }
            self.push_words(words);
            return;
        }
        let mut words = Vec::new();
        if negative {
            words.push("minus".into());
        }
        let value: Option<u64> = if int.len() <= 12 && !(int.len() > 1 && int.starts_with('0')) {
            int.parse().ok()
        } else {
            None
        };
        let suffix = self.letters_at(self.i);
        let after = self.chars.get(self.i + suffix.len()).copied();
        let suffix_is_word_end = !suffix.is_empty() && !after.is_some_and(|c| c == '\'');
        let ordinal = suffix_is_word_end
            && ["st", "nd", "rd", "th"].contains(&suffix.as_str())
            && frac.is_empty();
        let decade = suffix == "s" && plain && int.len() == 4 && suffix_is_word_end;
        let is_year = plain
            && int.len() == 4
            && currency.is_none()
            && value.is_some_and(|v| (1100..=2099).contains(&v) && v % 1000 != 0)
            && !ordinal;
        match value {
            Some(v) if is_year => words.extend(year_words(v)),
            Some(v) => cardinal_into(v, &mut words),
            None => words.extend(digits_words(&int)),
        }
        if ordinal {
            ordinalize(&mut words);
            self.i += suffix.len();
        } else if decade {
            pluralize(&mut words);
            self.i += 1;
        }
        let mut cents = None;
        if !frac.is_empty() {
            if currency.is_some() && frac.len() <= 2 {
                let c = if frac.len() == 1 {
                    format!("{frac}0")
                } else {
                    frac.clone()
                };
                cents = c.parse::<u64>().ok().filter(|c| *c > 0);
            } else {
                words.push("point".into());
                words.extend(digits_words(&frac));
            }
        }
        if let Some((major, minor)) = currency {
            // "$5 million" reads "five million dollars".
            let mut j = self.i;
            while self.chars.get(j) == Some(&' ') {
                j += 1;
            }
            let next = self.letters_at(j);
            if ["thousand", "million", "billion", "trillion"].contains(&next.as_str()) {
                words.push(next.clone());
                self.i = j + next.len();
            }
            let singular = words.as_slice() == ["one"];
            words.push(if singular {
                major.to_string()
            } else {
                format!("{major}s")
            });
            if let Some(c) = cents {
                cardinal_into(c, &mut words);
                words.push(if c == 1 {
                    minor.to_string()
                } else {
                    format!("{minor}s")
                });
            }
        } else if self.peek(0) == Some('%') {
            self.i += 1;
            words.push("percent".into());
        } else if !ordinal && !decade {
            if let Some((_, spoken)) = UNITS.iter().find(|(u, _)| *u == suffix) {
                if suffix_is_word_end {
                    self.i += suffix.len();
                    words.push((*spoken).to_string());
                }
            }
        }
        self.push_words(words);
    }

    fn spell(&mut self, letters: &str) {
        for c in letters.chars() {
            if c.is_ascii_alphabetic() {
                self.push_word(LETTER_NAMES[(c.to_ascii_lowercase() as u8 - b'a') as usize]);
            }
        }
    }

    fn word(&mut self) {
        // Dotted acronym such as "U.S." or "a.m.".
        if self.peek(1) == Some('.')
            && self.peek(2).is_some_and(|c| c.is_ascii_alphabetic())
            && self.peek(3) == Some('.')
        {
            let mut letters = String::new();
            while self.peek(0).is_some_and(|c| c.is_ascii_alphabetic()) && self.peek(1) == Some('.')
            {
                letters.push(self.chars[self.i]);
                self.i += 2;
            }
            if letters.eq_ignore_ascii_case("eg") {
                self.push_words(vec!["for".into(), "example".into()]);
            } else if letters.eq_ignore_ascii_case("ie") {
                self.push_words(vec!["that".into(), "is".into()]);
            } else {
                self.spell(&letters);
            }
            return;
        }
        let start = self.i;
        let mut raw = String::new();
        while let Some(c) = self.peek(0) {
            let inner_apostrophe = c == '\''
                && !raw.is_empty()
                && self.peek(1).is_some_and(|n| n.is_ascii_alphabetic());
            if c.is_ascii_alphabetic() || inner_apostrophe {
                raw.push(c);
                self.i += 1;
            } else {
                break;
            }
        }
        debug_assert!(self.i > start);
        // Collapse runs of three or more identical letters to two ("sooooo").
        let mut word = String::with_capacity(raw.len());
        let mut run = 0usize;
        let mut prev = '\0';
        for c in raw.chars() {
            run = if c.to_ascii_lowercase() == prev {
                run + 1
            } else {
                1
            };
            prev = c.to_ascii_lowercase();
            if run <= 2 {
                word.push(c);
            }
        }
        let lower = word.to_ascii_lowercase();
        let has_lower = word.chars().any(|c| c.is_ascii_lowercase());
        // Abbreviation with trailing period consumes the period.
        if let Some((_, spoken)) = ABBREVIATIONS.iter().find(|(a, _)| *a == lower) {
            let with_period = self.peek(0) == Some('.');
            let always =
                ["ok", "etc", "vs", "mr", "mrs", "ms", "dr", "prof"].contains(&lower.as_str());
            if with_period || always {
                if with_period && lower != "etc" && lower != "ok" {
                    self.i += 1;
                }
                self.push_word(*spoken);
                return;
            }
        }
        if !has_lower && word.len() >= 2 && word.len() <= 6 && !self.lexicon.contains(&lower) {
            self.spell(&word);
            return;
        }
        self.push_word(lower);
    }

    fn run(&mut self) {
        while self.i < self.chars.len() && self.tokens.len() < MAX_TOKENS {
            let c = self.chars[self.i];
            let prev_is_boundary = self.i == 0
                || !(self.chars[self.i - 1].is_ascii_alphanumeric()
                    || self.chars[self.i - 1] == '.');
            match c {
                '0'..='9' => self.number(None, false),
                '$' | '£' | '€' if self.peek(1).is_some_and(|d| d.is_ascii_digit()) => {
                    self.i += 1;
                    let unit = match c {
                        '$' => ("dollar", "cent"),
                        '£' => ("pound", "penny"),
                        _ => ("euro", "cent"),
                    };
                    self.number(Some(unit), false);
                }
                '-' | '−'
                    if prev_is_boundary && self.peek(1).is_some_and(|d| d.is_ascii_digit()) =>
                {
                    self.i += 1;
                    self.number(None, true);
                }
                '#' if self.peek(1).is_some_and(|d| d.is_ascii_digit()) => {
                    self.i += 1;
                    self.push_word("number");
                }
                c if c.is_ascii_alphabetic() => self.word(),
                '&' => {
                    self.i += 1;
                    self.push_word("and");
                }
                '+' => {
                    self.i += 1;
                    self.push_word("plus");
                }
                '=' => {
                    self.i += 1;
                    self.push_word("equals");
                }
                '@' => {
                    self.i += 1;
                    self.push_word("at");
                }
                '%' => {
                    self.i += 1;
                    self.push_word("percent");
                }
                '/' => {
                    self.i += 1;
                    self.push_word("slash");
                }
                '.' | '!' | '?' | '…' | '\n' | '\r' => {
                    self.i += 1;
                    self.pause(Pause::Long);
                }
                ',' | ';' | ':' | '—' | '–' | '(' | ')' | '"' | '“' | '”' => {
                    self.i += 1;
                    if !matches!(c, '"' | '“' | '”' | '(' | ')') {
                        self.pause(Pause::Short);
                    }
                }
                _ => self.i += 1,
            }
        }
    }
}

/// Normalises written text into spoken-word and pause tokens. Never panics; at most
/// [`MAX_TOKENS`] tokens are produced. Leading and trailing pauses are dropped.
pub fn normalize(text: &str, lexicon: &Lexicon) -> Vec<Token> {
    let mut s = Scanner {
        chars: text.chars().map(fold).collect(),
        i: 0,
        tokens: Vec::new(),
        pending: None,
        lexicon,
    };
    s.run();
    s.tokens
}

/// Just the spoken words of [`normalize`].
pub fn spoken_words(text: &str, lexicon: &Lexicon) -> Vec<String> {
    normalize(text, lexicon)
        .into_iter()
        .filter_map(|t| match t {
            Token::Word(w) => Some(w),
            Token::Pause(_) => None,
        })
        .collect()
}
