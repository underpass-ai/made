use made_core::error::DomainError;

const ROUNDTABLE_FIXED_ORDER_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../api/examples/ceremonies/fragments/roundtable_fixed_order.yaml"
));

/// A shipped authoring preset backed by a discoverable definition fragment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CeremonyPatternPreset {
    RoundtableFixedOrder,
}

impl CeremonyPatternPreset {
    pub const ALL: [Self; 1] = [Self::RoundtableFixedOrder];

    pub fn parse(value: &str) -> Result<Self, DomainError> {
        match value {
            "roundtable_fixed_order" => Ok(Self::RoundtableFixedOrder),
            other => Err(DomainError::InvalidDocument {
                reason: format!("unknown ceremony design pattern `{other}`"),
            }),
        }
    }

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::RoundtableFixedOrder => "roundtable_fixed_order",
        }
    }

    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::RoundtableFixedOrder => {
                "One speaking turn per participant in declaration order; every turn after the first receives the prior transcript."
            }
        }
    }

    /// The shipped definition fragment behind this preset.
    ///
    /// The application layer treats it as input data. Parsing and rendering the
    /// YAML remain adapter responsibilities.
    #[must_use]
    pub const fn fragment_source(self) -> &'static str {
        match self {
            Self::RoundtableFixedOrder => ROUNDTABLE_FIXED_ORDER_FRAGMENT,
        }
    }
}
