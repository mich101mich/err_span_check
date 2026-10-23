use crate::*;
use std::ffi::OsStr;

pub fn should_update() -> Result<bool> {
    parse_should_update(std::env::var_os("ERR_SPAN_CHECK").as_deref())
}

fn parse_should_update(var: Option<&OsStr>) -> Result<bool> {
    let Some(var) = var else {
        return Ok(true); // update by default
    };

    let normalized = var.to_string_lossy().to_lowercase();

    match normalized.as_str() {
        "overwrite" | "update" => Ok(true), // redundant, but ok
        "frozen" | "locked" => Ok(false),
        _ => anyhow::bail!(
            "unrecognized value of ERR_SPAN_CHECK: {:?}",
            var.to_string_lossy().into_owned()
        ),
    }
}

pub fn should_split_on_change() -> Result<bool> {
    parse_should_split_on_change(std::env::var_os("ERR_SPAN_CHECK_SPLIT_ON_CHANGE").as_deref())
}

fn parse_should_split_on_change(var: Option<&OsStr>) -> Result<bool> {
    let Some(var) = var else {
        return Ok(false); // don't split by default
    };

    let normalized = var.to_string_lossy().to_lowercase();

    match normalized.as_str() {
        "true" | "yes" | "y" | "1" => Ok(true),
        "false" | "no" | "n" | "0" => Ok(false),
        _ => anyhow::bail!(
            "unrecognized value of ERR_SPAN_CHECK_SPLIT_ON_CHANGE: {:?}",
            var.to_string_lossy().into_owned()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_should_update() {
        assert!(parse_should_update(None).unwrap());
        assert!(parse_should_update(Some(OsStr::new("overwrite"))).unwrap());
        assert!(parse_should_update(Some(OsStr::new("UPDATE"))).unwrap());
        assert!(!parse_should_update(Some(OsStr::new("frozen"))).unwrap());
        assert!(!parse_should_update(Some(OsStr::new("Locked"))).unwrap());
        assert_eq!(
            parse_should_update(Some(OsStr::new("invalid")))
                .unwrap_err()
                .to_string(),
            r#"unrecognized value of ERR_SPAN_CHECK: "invalid""#
        );
    }

    #[test]
    fn test_parse_should_split_on_change() {
        assert!(!parse_should_split_on_change(None).unwrap());
        for val in ["true", "TRUE", "yes", "Y", "1"] {
            assert!(parse_should_split_on_change(Some(OsStr::new(val))).unwrap());
        }
        for val in ["false", "FALSE", "no", "N", "0"] {
            assert!(!parse_should_split_on_change(Some(OsStr::new(val))).unwrap());
        }
        assert_eq!(
            parse_should_split_on_change(Some(OsStr::new("maybe")))
                .unwrap_err()
                .to_string(),
            r#"unrecognized value of ERR_SPAN_CHECK_SPLIT_ON_CHANGE: "maybe""#
        );
    }
}
