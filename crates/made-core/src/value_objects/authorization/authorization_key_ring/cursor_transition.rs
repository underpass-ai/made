#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorTransition {
    Preserve,
    Invalidate,
}
