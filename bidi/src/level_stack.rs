use crate::bidi_class::BidiClass;
use crate::level::{Level, MAX_DEPTH};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum Override {
    Neutral,
    LTR,
    RTL,
}

/// An implementation of the stack/STATUSSTACKELEMENT from bidiref
#[derive(Debug)]
pub(crate) struct LevelStack {
    embedding_level: [Level; MAX_DEPTH],
    override_status: [Override; MAX_DEPTH],
    isolate_status: [bool; MAX_DEPTH],
    /// Current index into the stack arrays above
    depth: usize,
}

impl LevelStack {
    pub fn new() -> Self {
        Self {
            embedding_level: [Level::default(); MAX_DEPTH],
            override_status: [Override::Neutral; MAX_DEPTH],
            isolate_status: [false; MAX_DEPTH],
            depth: 0,
        }
    }

    pub fn depth(&self) -> usize {
        self.depth
    }

    pub fn push(&mut self, level: Level, override_status: Override, isolate_status: bool) {
        let depth = self.depth;
        if depth >= MAX_DEPTH {
            return;
        }
        log::trace!(
            "pushing level={:?} override={:?} isolate={} at depth={}",
            level,
            override_status,
            isolate_status,
            depth
        );
        self.embedding_level[depth] = level;
        self.override_status[depth] = override_status;
        self.isolate_status[depth] = isolate_status;
        self.depth += 1;
    }

    pub fn pop(&mut self) {
        if self.depth > 0 {
            self.depth -= 1;
        }
    }

    pub fn embedding_level(&self) -> Level {
        self.embedding_level[self.depth - 1]
    }

    pub fn override_status(&self) -> Override {
        self.override_status[self.depth - 1]
    }

    pub fn apply_override(&self, bc: &mut BidiClass) {
        match self.override_status() {
            Override::LTR => *bc = BidiClass::LeftToRight,
            Override::RTL => *bc = BidiClass::RightToLeft,
            Override::Neutral => {}
        }
    }

    pub fn isolate_status(&self) -> bool {
        self.isolate_status[self.depth - 1]
    }
}
