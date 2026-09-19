use clap::ValueEnum;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum AuthorizationScopeKind {
    Global,
    Ceremony,
    CeremonyTree,
    Definition,
    Artifact,
    Council,
    Budget,
}
