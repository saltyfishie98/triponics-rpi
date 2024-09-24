use std::sync::Arc;

use serde::Deserialize;

fn deserialize_arc_str<'de, D>(deserializer: D) -> Result<Arc<str>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(String::deserialize(deserializer)?.into())
}

fn serialize_arc_str<S>(v: &Arc<str>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(v)
}

fn deserialize_arc_bytes<'de, D>(deserializer: D) -> Result<Arc<[u8]>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Vec::deserialize(deserializer)?.into())
}

fn serialize_arc_bytes<S>(v: &Arc<[u8]>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_bytes(v.as_ref())
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct AtomicFixedBytes(
    #[serde(
        serialize_with = "serialize_arc_bytes",
        deserialize_with = "deserialize_arc_bytes"
    )]
    Arc<[u8]>,
);
impl From<&'static [u8]> for AtomicFixedBytes {
    fn from(value: &'static [u8]) -> Self {
        Self(value.into())
    }
}
impl From<Arc<[u8]>> for AtomicFixedBytes {
    fn from(value: Arc<[u8]>) -> Self {
        Self(value)
    }
}
impl From<Vec<u8>> for AtomicFixedBytes {
    fn from(value: Vec<u8>) -> Self {
        Self(Arc::<[u8]>::from(value))
    }
}
impl AsRef<[u8]> for AtomicFixedBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Eq, PartialEq, Hash)]
pub struct AtomicFixedString(
    #[serde(
        serialize_with = "serialize_arc_str",
        deserialize_with = "deserialize_arc_str"
    )]
    Arc<str>,
);
impl From<&'static str> for AtomicFixedString {
    fn from(value: &'static str) -> Self {
        Self(value.into())
    }
}
impl From<String> for AtomicFixedString {
    fn from(value: String) -> Self {
        Self(value.into())
    }
}
impl From<AtomicFixedString> for Arc<str> {
    fn from(value: AtomicFixedString) -> Self {
        value.0
    }
}
impl AsRef<str> for AtomicFixedString {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}
impl std::fmt::Display for AtomicFixedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}
impl std::fmt::Debug for AtomicFixedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}
