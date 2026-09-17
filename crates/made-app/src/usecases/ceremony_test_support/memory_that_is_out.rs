/// A memory backend that is there and cannot answer.
///
/// The failure a session must survive: not "no memory configured",
/// which every read handles, but a configured one that refuses.
#[derive(Debug, Default)]
pub(in crate::usecases) struct MemoryThatIsOut;
