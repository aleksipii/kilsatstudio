//! Compact CMUdict lexicon (BSD-2-Clause, see data/CMUDICT-LICENSE), loaded once.
use super::Phoneme;
use flate2::read::GzDecoder;
use std::io::Read;
use std::sync::OnceLock;

static DATA: &[u8] = include_bytes!("../../data/cmudict.txt.gz");

/// Letter names and a few words CMUdict lacks, used only when CMUdict has no entry.
const SUPPLEMENT: &[(&str, &str)] = &[
    ("ay", "EY"),
    ("bee", "B IY"),
    ("see", "S IY"),
    ("dee", "D IY"),
    ("ee", "IY"),
    ("eff", "EH F"),
    ("gee", "JH IY"),
    ("aitch", "EY CH"),
    ("eye", "AY"),
    ("jay", "JH EY"),
    ("kay", "K EY"),
    ("el", "EH L"),
    ("em", "EH M"),
    ("en", "EH N"),
    ("oh", "OW"),
    ("pee", "P IY"),
    ("cue", "K Y UW"),
    ("ar", "AA R"),
    ("ess", "EH S"),
    ("tee", "T IY"),
    ("you", "Y UW"),
    ("vee", "V IY"),
    ("doubleyou", "D AH B AH L Y UW"),
    ("ex", "EH K S"),
    ("why", "W AY"),
    ("zee", "Z IY"),
];

/// Sorted `word PH PH` lines held decompressed, with line offsets for binary search.
pub struct Lexicon {
    text: String,
    lines: Vec<u32>,
}

impl Lexicon {
    /// The embedded CMUdict, decompressed on first use and shared afterwards.
    pub fn builtin() -> &'static Lexicon {
        static CELL: OnceLock<Lexicon> = OnceLock::new();
        CELL.get_or_init(|| {
            let mut text = String::new();
            GzDecoder::new(DATA)
                .read_to_string(&mut text)
                .expect("embedded CMUdict is valid gzip text");
            let mut lines = Vec::new();
            let mut start = 0usize;
            for (i, b) in text.bytes().enumerate() {
                if b == b'\n' {
                    lines.push(start as u32);
                    start = i + 1;
                }
            }
            Lexicon { text, lines }
        })
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    fn line(&self, i: usize) -> &str {
        let s = self.lines[i] as usize;
        let e = self
            .lines
            .get(i + 1)
            .map_or(self.text.len(), |n| *n as usize);
        self.text[s..e].trim_end_matches('\n')
    }

    /// First CMUdict pronunciation (stress removed) or the built-in supplement.
    pub fn lookup(&self, word: &str) -> Option<Vec<Phoneme>> {
        let (mut lo, mut hi) = (0, self.lines.len());
        while lo < hi {
            let mid = (lo + hi) / 2;
            let line = self.line(mid);
            let (w, rest) = line.split_once(' ').unwrap_or((line, ""));
            match w.cmp(word) {
                std::cmp::Ordering::Equal => return parse(rest),
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
            }
        }
        SUPPLEMENT
            .iter()
            .find(|(w, _)| *w == word)
            .and_then(|(_, p)| parse(p))
    }

    pub fn contains(&self, word: &str) -> bool {
        self.lookup(word).is_some()
    }
}

fn parse(phones: &str) -> Option<Vec<Phoneme>> {
    let v: Option<Vec<Phoneme>> = phones.split_whitespace().map(Phoneme::parse).collect();
    v.filter(|v| !v.is_empty())
}
