use serde::{Deserialize, de};
use std::fmt;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InheritEdition {
    #[allow(dead_code)]
    pub workspace: True,
}

pub(crate) struct True;

impl<'de> Deserialize<'de> for True {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_bool(True)
    }
}

impl<'de> de::Visitor<'de> for True {
    type Value = True;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("bool")
    }

    fn visit_bool<E>(self, b: bool) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if b {
            Ok(True)
        } else {
            Err(de::Error::custom(
                "workspace=false is unsupported for package.edition",
            ))
        }
    }
}
