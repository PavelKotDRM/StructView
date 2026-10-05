//! Direction semantics shared by graph importers and visualizations.

/// Arrowheads relative to the stored source and target of an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeDirection {
    Directed,
    Undirected,
    Bidirectional,
    Reverse,
}

impl EdgeDirection {
    pub fn from_directed(directed: bool) -> Self {
        if directed {
            Self::Directed
        } else {
            Self::Undirected
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "directed" | "forward" => Ok(Self::Directed),
            "undirected" | "none" => Ok(Self::Undirected),
            "bidirectional" | "mutual" | "both" => Ok(Self::Bidirectional),
            "reverse" | "back" => Ok(Self::Reverse),
            _ => Err(format!("Unsupported edge direction: {value}")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Directed => "directed",
            Self::Undirected => "undirected",
            Self::Bidirectional => "bidirectional",
            Self::Reverse => "reverse",
        }
    }

    pub fn arrow_at_source(self) -> bool {
        matches!(self, Self::Bidirectional | Self::Reverse)
    }

    pub fn arrow_at_target(self) -> bool {
        matches!(self, Self::Directed | Self::Bidirectional)
    }
}
