use super::ScriptObservation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptResolution {
    Observed(ScriptObservation),
    ReconciliationRequired,
}
