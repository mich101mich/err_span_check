use crate::*;

pub fn should_update() -> Result<bool> {
    let Some(var) = std::env::var_os("ERR_SPAN_CHECK") else {
        return Ok(false);
    };

    match var.as_os_str().to_str() {
        Some("overwrite") => Ok(true),
        _ => Err(Error::UpdateVar(var.to_string_lossy().into_owned())),
    }
}
