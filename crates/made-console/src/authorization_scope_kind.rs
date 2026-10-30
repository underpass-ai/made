use clap::ValueEnum;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum AuthorizationScopeKind {
    Global,
    Protocol,
    ProtocolTree,
    Definition,
    Artifact,
    Council,
    Budget,
}
