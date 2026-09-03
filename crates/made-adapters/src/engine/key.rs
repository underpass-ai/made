use super::KeyShape;

/// A borrowed key in one of the two shapes the tables use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Key<'a> {
    Str(&'a str),
    Bytes(&'a [u8]),
}

impl Key<'_> {
    pub(crate) const fn shape(&self) -> KeyShape {
        match self {
            Key::Str(_) => KeyShape::Str,
            Key::Bytes(_) => KeyShape::Bytes,
        }
    }
}
