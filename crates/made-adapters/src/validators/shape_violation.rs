#[derive(Debug)]
pub(super) struct ShapeViolation {
    pub(super) kind: &'static str,
    pub(super) path: String,
    pub(super) limit: usize,
    pub(super) actual: usize,
}

impl ShapeViolation {
    pub(super) fn summary(&self) -> String {
        format!(
            "{} at `{}`: {} exceeds limit {}",
            self.kind, self.path, self.actual, self.limit
        )
    }
}
