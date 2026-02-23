use std::{convert::Infallible, path::Path};

pub(crate) trait ErrorExt<T> {
    fn path_context<P: AsRef<Path>>(self, path: P, text: &str) -> anyhow::Result<T>;
}

impl<T, E> ErrorExt<T> for Result<T, E>
where
    Result<T, E>: anyhow::Context<T, E>,
{
    fn path_context<P: AsRef<Path>>(self, path: P, text: &str) -> anyhow::Result<T> {
        anyhow::Context::with_context(self, || {
            text.replace("<path>", &path.as_ref().display().to_string())
        })
    }
}

impl<T> ErrorExt<T> for Option<T>
where
    Option<T>: anyhow::Context<T, Infallible>,
{
    fn path_context<P: AsRef<Path>>(self, path: P, text: &str) -> anyhow::Result<T> {
        anyhow::Context::with_context(self, || {
            text.replace("<path>", &path.as_ref().display().to_string())
        })
    }
}
