//! A reader of binary FBX files: the node tree ([`tree`]).
//!
//! Every offset, length and array size is checked against the file before it is used; a file
//! that does not fit is refused with the offset where it stopped fitting.
//!
//! ```no_run
//! let bytes = std::fs::read("model.fbx")?;
//! let doc = alice_fbx::read_tree(&bytes)?;
//! for node in &doc.nodes {
//!     println!("{} ({} properties, {} children)", node.name, node.props.len(), node.children.len());
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![forbid(unsafe_code)]

pub mod tree;

pub use tree::{read_tree, Document, Node, Prop};

/// Why a file was refused
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FbxError {
    /// The file does not start with the binary FBX magic
    NotFbx,
    /// The version is outside [`tree::VERSIONS`]
    UnsupportedVersion(u32),
    /// The file ends before a value read at this offset
    Truncated {
        /// Offset
        at: usize,
    },
    /// A record's end offset or lengths do not fit the file or its parent
    BadOffset {
        /// Offset of the record
        at: usize,
    },
    /// A record name that is not UTF-8
    BadName {
        /// Offset of the name
        at: usize,
    },
    /// A property that does not fit its record
    BadProperty {
        /// Offset of the property
        at: usize,
        /// What was wrong
        why: &'static str,
    },
    /// An array property that does not fit, does not inflate, or is too long
    BadArray {
        /// Offset of the property
        at: usize,
        /// What was wrong
        why: &'static str,
    },
    /// Records nested deeper than [`tree::MAX_DEPTH`]
    TooDeep {
        /// Offset of the list that is too deep
        at: usize,
    },
}

impl std::fmt::Display for FbxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFbx => write!(f, "not a binary FBX file"),
            Self::UnsupportedVersion(v) => write!(f, "FBX version {v} is not read"),
            Self::Truncated { at } => write!(f, "the file ends at {at}"),
            Self::BadOffset { at } => write!(f, "a record at {at} does not fit"),
            Self::BadName { at } => write!(f, "a record name at {at} is not UTF-8"),
            Self::BadProperty { at, why } => write!(f, "a property at {at}: {why}"),
            Self::BadArray { at, why } => write!(f, "an array at {at}: {why}"),
            Self::TooDeep { at } => write!(f, "records nested too deep at {at}"),
        }
    }
}

impl std::error::Error for FbxError {}
