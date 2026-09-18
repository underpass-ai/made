use std::time::Duration;

use made_core::error::DomainError;

/// Bounded buffering and external-writer polling settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeremonyProgressSettings {
    catch_up_interval: Duration,
    channel_capacity: usize,
}

impl CeremonyProgressSettings {
    pub const DEFAULT_CATCH_UP_INTERVAL: Duration = Duration::from_millis(250);
    pub const DEFAULT_CHANNEL_CAPACITY: usize = 16;
    pub const MAX_CATCH_UP_INTERVAL: Duration = Duration::from_secs(30);
    pub const MAX_CHANNEL_CAPACITY: usize = 1_000;

    pub fn new(catch_up_interval: Duration, channel_capacity: usize) -> Result<Self, DomainError> {
        if catch_up_interval < Duration::from_millis(1)
            || catch_up_interval > Self::MAX_CATCH_UP_INTERVAL
        {
            return Err(DomainError::OutOfRange {
                field: "ceremony_progress_catch_up_interval_ms",
                value: catch_up_interval.as_secs_f64() * 1_000.0,
                min: 1.0,
                max: Self::MAX_CATCH_UP_INTERVAL.as_secs_f64() * 1_000.0,
            });
        }
        if channel_capacity == 0 || channel_capacity > Self::MAX_CHANNEL_CAPACITY {
            #[allow(clippy::cast_precision_loss)]
            return Err(DomainError::OutOfRange {
                field: "ceremony_progress_channel_capacity",
                value: channel_capacity as f64,
                min: 1.0,
                max: Self::MAX_CHANNEL_CAPACITY as f64,
            });
        }
        Ok(Self {
            catch_up_interval,
            channel_capacity,
        })
    }

    #[must_use]
    pub const fn catch_up_interval(self) -> Duration {
        self.catch_up_interval
    }

    #[must_use]
    pub const fn channel_capacity(self) -> usize {
        self.channel_capacity
    }
}

impl Default for CeremonyProgressSettings {
    fn default() -> Self {
        Self {
            catch_up_interval: Self::DEFAULT_CATCH_UP_INTERVAL,
            channel_capacity: Self::DEFAULT_CHANNEL_CAPACITY,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_reject_unbounded_channels_and_timer_intervals() {
        assert!(CeremonyProgressSettings::new(Duration::ZERO, 1).is_err());
        assert!(CeremonyProgressSettings::new(Duration::from_nanos(1), 1).is_err());
        assert!(CeremonyProgressSettings::new(Duration::from_millis(1), 0).is_err());
        assert!(CeremonyProgressSettings::new(
            CeremonyProgressSettings::MAX_CATCH_UP_INTERVAL + Duration::from_millis(1),
            1,
        )
        .is_err());
        assert!(CeremonyProgressSettings::new(
            Duration::from_millis(1),
            CeremonyProgressSettings::MAX_CHANNEL_CAPACITY + 1,
        )
        .is_err());

        let settings = CeremonyProgressSettings::new(
            CeremonyProgressSettings::MAX_CATCH_UP_INTERVAL,
            CeremonyProgressSettings::MAX_CHANNEL_CAPACITY,
        )
        .unwrap();
        assert_eq!(
            settings.channel_capacity(),
            CeremonyProgressSettings::MAX_CHANNEL_CAPACITY
        );
    }
}
