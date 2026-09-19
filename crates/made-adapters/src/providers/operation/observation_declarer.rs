/// Who declared an observation when it has no external authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObservationDeclarer {
    Provider,
    Adapter,
    Fixture,
}
