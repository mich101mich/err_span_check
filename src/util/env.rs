use crate::*;

pub fn should_update() -> Result<bool> {
    let Some(var) = std::env::var_os("ERR_SPAN_CHECK") else {
        return Ok(true);
    };

    match var.as_os_str().to_str() {
        Some("overwrite" | "update") => Ok(true), // redundant, but ok
        Some("frozen" | "locked") => Ok(false),
        _ => anyhow::bail!(
            "unrecognized value of ERR_SPAN_CHECK: {:?}",
            var.to_string_lossy().into_owned()
        ),
    }
}
