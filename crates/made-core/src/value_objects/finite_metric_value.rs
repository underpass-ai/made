/// A metric sample value proven to be finite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FiniteMetricValue(f64);

impl FiniteMetricValue {
    pub(super) fn new(value: f64) -> Self {
        debug_assert!(value.is_finite());
        Self(value)
    }

    #[must_use]
    pub fn get(self) -> f64 {
        self.0
    }
}
