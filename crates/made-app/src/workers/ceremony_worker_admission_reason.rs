use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CeremonyWorkerAdmissionReason {
    Admitted,
    Backpressure,
    Capacity,
    Budget,
    Permission,
    Draining,
}

impl fmt::Display for CeremonyWorkerAdmissionReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Admitted => "admitted",
            Self::Backpressure => "backpressure",
            Self::Capacity => "capacity",
            Self::Budget => "budget",
            Self::Permission => "permission",
            Self::Draining => "draining",
        })
    }
}
