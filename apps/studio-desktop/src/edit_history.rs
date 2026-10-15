//! Bounded whole-document history. The editor decides what constitutes one edit.

use std::collections::VecDeque;

const MAX_ENTRIES: usize = 64;
const MAX_BYTES: usize = 8 * 1024 * 1024;

/// Undo/redo snapshots are bounded separately from the current document, whose
/// size is controlled by the document loader. No disk or shared global state.
pub(crate) struct History {
    current: String,
    undo: VecDeque<String>,
    redo: VecDeque<String>,
}

impl History {
    pub(crate) fn new(initial: &str) -> Self {
        Self {
            current: initial.to_owned(),
            undo: VecDeque::new(),
            redo: VecDeque::new(),
        }
    }

    /// Record one committed edit. Untracked edits invalidate the old branch.
    pub(crate) fn record(&mut self, before: &str, after: &str) {
        if before == after {
            return;
        }
        if self.current != before {
            self.reset(before);
        }
        self.redo.clear();
        self.undo.push_back(std::mem::take(&mut self.current));
        self.current = after.to_owned();
        self.trim();
    }

    pub(crate) fn undo(&mut self, current: &str) -> Option<String> {
        if !self.matches(current) {
            return None;
        }
        let previous = self.undo.pop_back()?;
        self.redo
            .push_back(std::mem::replace(&mut self.current, previous));
        self.trim();
        Some(self.current.clone())
    }

    pub(crate) fn redo(&mut self, current: &str) -> Option<String> {
        if !self.matches(current) {
            return None;
        }
        let next = self.redo.pop_back()?;
        self.undo
            .push_back(std::mem::replace(&mut self.current, next));
        self.trim();
        Some(self.current.clone())
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Start a new document history (including opening a demo).
    pub(crate) fn reset(&mut self, source: &str) {
        self.current = source.to_owned();
        self.undo.clear();
        self.redo.clear();
    }

    fn matches(&mut self, current: &str) -> bool {
        if current == self.current {
            true
        } else {
            self.reset(current);
            false
        }
    }

    fn trim(&mut self) {
        // Remove farthest undo states first. Redo is a stack: its front is the
        // most distant future state, so removing it preserves contiguous redo.
        while self.undo.len() + self.redo.len() > MAX_ENTRIES
            || self
                .undo
                .iter()
                .chain(&self.redo)
                .map(String::len)
                .sum::<usize>()
                > MAX_BYTES
        {
            if self.undo.pop_front().is_none() {
                self.redo.pop_front();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_and_new_branch() {
        let mut history = History::new("a");
        history.record("a", "b");
        history.record("b", "c");
        assert_eq!(history.undo("c").as_deref(), Some("b"));
        assert_eq!(history.undo("b").as_deref(), Some("a"));
        assert!(!history.can_undo());
        assert_eq!(history.redo("a").as_deref(), Some("b"));
        history.record("b", "d");
        assert!(!history.can_redo());
        assert_eq!(history.undo("d").as_deref(), Some("b"));
        assert_eq!(history.redo("b").as_deref(), Some("d"));
    }

    #[test]
    fn reset_and_mismatch_do_not_apply_stale_states() {
        let mut history = History::new("a");
        history.record("a", "b");
        assert_eq!(history.undo("externally changed"), None);
        assert!(!history.can_undo());
        history.record("other", "edited");
        assert_eq!(history.undo("edited").as_deref(), Some("other"));
        history.reset("opened file");
        assert!(!history.can_redo());
        assert_eq!(history.undo("opened file"), None);
    }

    #[test]
    fn noop_preserves_redo() {
        let mut history = History::new("a");
        history.record("a", "b");
        history.undo("b");
        history.record("a", "a");
        assert_eq!(history.redo("a").as_deref(), Some("b"));
    }

    #[test]
    fn entry_limit_keeps_recent_edits() {
        let mut history = History::new("0");
        for n in 1..=100 {
            history.record(&(n - 1).to_string(), &n.to_string());
        }
        let mut current = "100".to_owned();
        let mut count = 0;
        while let Some(previous) = history.undo(&current) {
            current = previous;
            count += 1;
        }
        assert_eq!(count, MAX_ENTRIES);
        assert_eq!(current, "36");
    }

    #[test]
    fn byte_limit_and_oversized_documents_remain_bounded() {
        let large = "x".repeat(MAX_BYTES / 2 + 1);
        let mut history = History::new(&large);
        history.record(&large, "small");
        history.record("small", "newest");
        history.record("newest", &large);
        assert!(history.undo.iter().map(String::len).sum::<usize>() <= MAX_BYTES);
        assert_eq!(history.undo(&large).as_deref(), Some("newest"));
        let oversized = "z".repeat(MAX_BYTES + 1);
        history.reset(&oversized);
        history.record(&oversized, "usable");
        assert!(!history.can_undo());
        history.record("usable", "still usable");
        assert_eq!(history.undo("still usable").as_deref(), Some("usable"));
    }
}
