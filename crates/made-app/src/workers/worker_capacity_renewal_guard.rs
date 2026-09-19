/// Process-shared guard held while journal authority is renewed.
pub trait WorkerCapacityRenewalGuard: Send {}

impl<T: Send> WorkerCapacityRenewalGuard for T {}
