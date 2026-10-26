//! The binary FBX node tree: records, properties and arrays, every offset and length checked.
//!
//! A file is a 27-byte header (the magic, then the version as a little-endian `u32`), a list of
//! node records ended by a null record, and a footer. A record is its end offset, its property
//! count and the byte length of its properties (each `u32` before version 7500, `u64` from it),
//! a name of `u8` length, the properties, then its children ended by a null record when it has
//! any. Array properties may be zlib-compressed.

use crate::FbxError;

/// The oldest and newest versions read
pub const VERSIONS: std::ops::RangeInclusive<u32> = 7100..=7700;
/// The deepest nesting of records read
pub const MAX_DEPTH: usize = 64;
/// The most bytes one array may hold once inflated
pub const MAX_ARRAY_BYTES: usize = 256 << 20;

const MAGIC: &[u8; 23] = b"Kaydara FBX Binary  \x00\x1a\x00";

/// A property of a record
#[derive(Debug, Clone, PartialEq)]
pub enum Prop {
    /// `Y`
    I16(i16),
    /// `C`
    Bool(bool),
    /// `I`
    I32(i32),
    /// `F`
    F32(f32),
    /// `D`
    F64(f64),
    /// `L`
    I64(i64),
    /// `f`
    F32s(Vec<f32>),
    /// `d`
    F64s(Vec<f64>),
    /// `l`
    I64s(Vec<i64>),
    /// `i`
    I32s(Vec<i32>),
    /// `b`
    Bools(Vec<bool>),
    /// `S` (bytes: FBX names hold `\0\x01` separators)
    Str(Vec<u8>),
    /// `R`
    Raw(Vec<u8>),
}

/// A node record
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// The name
    pub name: String,
    /// The properties in order
    pub props: Vec<Prop>,
    /// The children in order (empty when the record has none)
    pub children: Vec<Node>,
}

impl Node {
    /// The first child with this name
    #[must_use]
    pub fn child(&self, name: &str) -> Option<&Self> {
        self.children.iter().find(|c| c.name == name)
    }

    /// The children with this name, in order
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Self> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }
}

/// A whole file: its version and its top-level records
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    /// The version (7100 to 7700)
    pub version: u32,
    /// The top-level records
    pub nodes: Vec<Node>,
}

impl Document {
    /// The first top-level record with this name
    #[must_use]
    pub fn node(&self, name: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.name == name)
    }
}

struct Reader<'a> {
    b: &'a [u8],
    wide: bool,
}

impl Reader<'_> {
    fn bytes(&self, at: usize, n: usize) -> Result<&[u8], FbxError> {
        at.checked_add(n)
            .and_then(|end| self.b.get(at..end))
            .ok_or(FbxError::Truncated { at })
    }

    fn u8(&self, at: usize) -> Result<u8, FbxError> {
        Ok(self.bytes(at, 1)?[0])
    }

    fn u32(&self, at: usize) -> Result<u32, FbxError> {
        let s = self.bytes(at, 4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn word(&self, at: usize) -> Result<u64, FbxError> {
        if self.wide {
            let s = self.bytes(at, 8)?;
            Ok(u64::from_le_bytes([
                s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
            ]))
        } else {
            self.u32(at).map(u64::from)
        }
    }

    fn word_len(&self) -> usize {
        if self.wide {
            8
        } else {
            4
        }
    }

    /// The records from `at` to the null record that ends the list (every list has one, before
    /// `end`); returns them and the offset after the null record
    fn list(
        &self,
        mut at: usize,
        end: usize,
        depth: usize,
    ) -> Result<(Vec<Node>, usize), FbxError> {
        if depth > MAX_DEPTH {
            return Err(FbxError::TooDeep { at });
        }
        let null = 3 * self.word_len() + 1;
        let mut nodes = Vec::new();
        loop {
            if at.checked_add(null).is_none_or(|e| e > end) {
                return Err(FbxError::Truncated { at });
            }
            let head = self.bytes(at, null)?;
            if head.iter().all(|&x| x == 0) {
                return Ok((nodes, at + null));
            }
            let (node, next) = self.record(at, end, depth)?;
            nodes.push(node);
            at = next;
        }
    }

    fn record(&self, at: usize, limit: usize, depth: usize) -> Result<(Node, usize), FbxError> {
        let w = self.word_len();
        let end = usize::try_from(self.word(at)?).map_err(|_| FbxError::BadOffset { at })?;
        let count = self.word(at + w)?;
        let props_len =
            usize::try_from(self.word(at + 2 * w)?).map_err(|_| FbxError::BadOffset { at })?;
        let name_len = usize::from(self.u8(at + 3 * w)?);
        let name_at = at + 3 * w + 1;
        if end <= name_at || end > limit {
            return Err(FbxError::BadOffset { at });
        }
        let name = String::from_utf8(self.bytes(name_at, name_len)?.to_vec())
            .map_err(|_| FbxError::BadName { at: name_at })?;
        let props_at = name_at + name_len;
        let props_end = props_at
            .checked_add(props_len)
            .filter(|&e| e <= end)
            .ok_or(FbxError::BadOffset { at })?;
        let mut props = Vec::new();
        let mut p = props_at;
        for _ in 0..count {
            if p >= props_end {
                return Err(FbxError::BadProperty {
                    at: p,
                    why: "past the property list",
                });
            }
            let (prop, next) = self.prop(p, props_end)?;
            props.push(prop);
            p = next;
        }
        if p != props_end {
            return Err(FbxError::BadProperty {
                at: p,
                why: "the property list length does not match",
            });
        }
        let children = if props_end < end {
            let (children, after) = self.list(props_end, end, depth + 1)?;
            if after != end {
                return Err(FbxError::BadOffset { at });
            }
            children
        } else {
            Vec::new()
        };
        Ok((
            Node {
                name,
                props,
                children,
            },
            end,
        ))
    }

    fn prop(&self, at: usize, end: usize) -> Result<(Prop, usize), FbxError> {
        let code = self.u8(at)?;
        let v = at + 1;
        let fixed = |n: usize| -> Result<&[u8], FbxError> {
            if v + n > end {
                return Err(FbxError::BadProperty {
                    at,
                    why: "past the property list",
                });
            }
            self.bytes(v, n)
        };
        let prop = match code {
            b'Y' => (
                Prop::I16(i16::from_le_bytes(fixed(2)?.try_into().unwrap())),
                v + 2,
            ),
            b'C' => (Prop::Bool(fixed(1)?[0] != 0), v + 1),
            b'I' => (
                Prop::I32(i32::from_le_bytes(fixed(4)?.try_into().unwrap())),
                v + 4,
            ),
            b'F' => (
                Prop::F32(f32::from_le_bytes(fixed(4)?.try_into().unwrap())),
                v + 4,
            ),
            b'D' => (
                Prop::F64(f64::from_le_bytes(fixed(8)?.try_into().unwrap())),
                v + 8,
            ),
            b'L' => (
                Prop::I64(i64::from_le_bytes(fixed(8)?.try_into().unwrap())),
                v + 8,
            ),
            b'S' | b'R' => {
                let n = self.u32(v)? as usize;
                let data = v
                    .checked_add(4)
                    .and_then(|d| d.checked_add(n))
                    .filter(|&e| e <= end)
                    .ok_or(FbxError::BadProperty {
                        at,
                        why: "a string past the property list",
                    })?;
                let s = self.bytes(v + 4, n)?.to_vec();
                (
                    if code == b'S' {
                        Prop::Str(s)
                    } else {
                        Prop::Raw(s)
                    },
                    data,
                )
            }
            b'f' | b'd' | b'l' | b'i' | b'b' => self.array(code, at, end)?,
            _ => {
                return Err(FbxError::BadProperty {
                    at,
                    why: "an unknown property type",
                })
            }
        };
        Ok(prop)
    }

    fn array(&self, code: u8, at: usize, end: usize) -> Result<(Prop, usize), FbxError> {
        let v = at + 1;
        let len = self.u32(v)? as usize;
        let encoding = self.u32(v + 4)?;
        let stored = self.u32(v + 8)? as usize;
        let data_at = v + 12;
        let data_end =
            data_at
                .checked_add(stored)
                .filter(|&e| e <= end)
                .ok_or(FbxError::BadArray {
                    at,
                    why: "data past the property list",
                })?;
        let size = match code {
            b'f' | b'i' => 4,
            b'd' | b'l' => 8,
            _ => 1,
        };
        let bytes = len
            .checked_mul(size)
            .filter(|&n| n <= MAX_ARRAY_BYTES)
            .ok_or(FbxError::BadArray {
                at,
                why: "longer than the array limit",
            })?;
        let stored_bytes = self.bytes(data_at, stored)?;
        let raw = match encoding {
            0 => {
                if stored != bytes {
                    return Err(FbxError::BadArray {
                        at,
                        why: "the stored length does not match the element count",
                    });
                }
                stored_bytes.to_vec()
            }
            1 => {
                let out =
                    miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(stored_bytes, bytes)
                        .map_err(|_| FbxError::BadArray {
                            at,
                            why: "zlib data that does not inflate within the declared length",
                        })?;
                if out.len() != bytes {
                    return Err(FbxError::BadArray {
                        at,
                        why: "inflated length does not match the element count",
                    });
                }
                out
            }
            _ => {
                return Err(FbxError::BadArray {
                    at,
                    why: "an unknown encoding",
                })
            }
        };
        let prop = match code {
            b'f' => Prop::F32s(
                raw.chunks_exact(4)
                    .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                    .collect(),
            ),
            b'i' => Prop::I32s(
                raw.chunks_exact(4)
                    .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
                    .collect(),
            ),
            b'd' => Prop::F64s(
                raw.chunks_exact(8)
                    .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
                    .collect(),
            ),
            b'l' => Prop::I64s(
                raw.chunks_exact(8)
                    .map(|c| i64::from_le_bytes(c.try_into().unwrap()))
                    .collect(),
            ),
            _ => Prop::Bools(raw.iter().map(|&x| x != 0).collect()),
        };
        Ok((prop, data_end))
    }
}

/// Reads the node tree of a binary FBX file
///
/// # Errors
///
/// [`FbxError::NotFbx`] without the binary magic, [`FbxError::UnsupportedVersion`] outside
/// [`VERSIONS`], and the other variants for a record, property or array that does not fit the
/// file
pub fn read_tree(bytes: &[u8]) -> Result<Document, FbxError> {
    if bytes.len() < 27 || &bytes[..23] != MAGIC {
        return Err(FbxError::NotFbx);
    }
    let version = u32::from_le_bytes([bytes[23], bytes[24], bytes[25], bytes[26]]);
    if !VERSIONS.contains(&version) {
        return Err(FbxError::UnsupportedVersion(version));
    }
    let r = Reader {
        b: bytes,
        wide: version >= 7500,
    };
    let (nodes, _) = r.list(27, bytes.len(), 0)?;
    Ok(Document { version, nodes })
}
