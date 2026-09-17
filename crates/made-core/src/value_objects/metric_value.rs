/// A Prometheus sample value, including the non-finite values its text
/// exposition permits but JSON numbers do not.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MetricValue {
    Finite(f64),
    Nan,
    PositiveInfinity,
    NegativeInfinity,
}

impl MetricValue {
    #[must_use]
    pub fn from_f64(value: f64) -> Self {
        if value.is_nan() {
            Self::Nan
        } else if value == f64::INFINITY {
            Self::PositiveInfinity
        } else if value == f64::NEG_INFINITY {
            Self::NegativeInfinity
        } else {
            Self::Finite(value)
        }
    }
}
