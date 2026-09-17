use super::FiniteMetricValue;

/// A Prometheus sample value, including the non-finite values its text
/// exposition permits but JSON numbers do not.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MetricValue {
    Finite(FiniteMetricValue),
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
            Self::Finite(FiniteMetricValue::new(value))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_finite_and_non_finite_values() {
        let MetricValue::Finite(finite) = MetricValue::from_f64(1.25) else {
            panic!("a finite input must remain finite");
        };
        assert_eq!(finite.get(), 1.25);
        assert_eq!(MetricValue::from_f64(f64::NAN), MetricValue::Nan);
        assert_eq!(
            MetricValue::from_f64(f64::INFINITY),
            MetricValue::PositiveInfinity
        );
        assert_eq!(
            MetricValue::from_f64(f64::NEG_INFINITY),
            MetricValue::NegativeInfinity
        );
    }
}
