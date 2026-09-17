/// Prometheus metric family kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricKind {
    Counter,
    Gauge,
    Histogram,
    Summary,
    Untyped,
}

impl MetricKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Counter => "counter",
            Self::Gauge => "gauge",
            Self::Histogram => "histogram",
            Self::Summary => "summary",
            Self::Untyped => "untyped",
        }
    }
}
