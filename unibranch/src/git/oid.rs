use std::fmt::Display;

use serde::{de::Visitor, Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct Oid(git2::Oid);

impl From<git2::Oid> for Oid {
    fn from(value: git2::Oid) -> Self {
        Self(value)
    }
}

impl From<Oid> for git2::Oid {
    fn from(val: Oid) -> Self {
        val.0
    }
}

impl Display for Oid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Serialize for Oid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{}", self.0))
    }
}

impl<'de> Deserialize<'de> for Oid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(OidVisitor)
    }
}

pub struct OidVisitor;

impl Visitor<'_> for OidVisitor {
    type Value = Oid;

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Oid(v
            .parse()
            .map_err(|_| E::custom(format!("Invalid OID: '{v}'")))?))
    }

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(formatter, "Expecting an Oid as a String")
    }
}

#[cfg(test)]
mod test {

    use super::Oid;
    #[test]
    fn test_deserialize() {
        let oid_as_string = "52a4d284cd73150a5c62e5e546381db82182032c";
        let deserialized: Oid = serde_json::from_str(&format!(r#""{oid_as_string}""#)).unwrap();
        assert_eq!(deserialized, Oid(oid_as_string.parse().unwrap()));
    }

    #[test]
    fn test_deserialize_invalid_oid() {
        let oid_as_string = "Invalid";
        let deserialized: Result<Oid, _> = serde_json::from_str(&format!(r#""{oid_as_string}""#));
        assert!(deserialized.is_err());
    }

    #[test]
    fn test_deserialize_invalid_json() {
        let oid_as_string = "52a4d284cd73150a5c62e5e546381db82182032c";
        let deserialized: Result<Oid, _> = serde_json::from_str(oid_as_string);
        assert!(deserialized.is_err());
    }

    #[test]
    fn test_serialize() {
        let oid_as_string = "52a4d284cd73150a5c62e5e546381db82182032c";
        let serialized = serde_json::to_string(&Oid(oid_as_string.parse().unwrap())).unwrap();
        assert_eq!(serialized, format!(r#""{oid_as_string}""#));
    }
}
