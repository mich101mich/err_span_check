use super::*;

#[derive(PartialEq, Debug)]
pub(crate) enum Update {
    None,
    Overwrite,
}

impl Update {
    pub fn env() -> Result<Self> {
        let Some(var) = std::env::var_os("ERR_SPAN_CHECK") else {
            return Ok(Update::None);
        };

        match var.as_os_str().to_str() {
            Some("overwrite") => Ok(Update::Overwrite),
            _ => Err(Error::UpdateVar(var.to_string_lossy().into_owned())),
        }
    }
}
