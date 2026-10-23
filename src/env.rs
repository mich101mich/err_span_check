use crate::*;

pub fn should_update() -> Result<bool> {
    let Some(var) = std::env::var_os("ERR_SPAN_CHECK") else {
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
    let Some(var) = std::env::var_os("ERR_SPAN_CHECK_SPLIT_ON_CHANGE") else {
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
