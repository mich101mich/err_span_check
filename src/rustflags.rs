const IGNORED_LINTS: &[&str] = &["dead_code"];
const ALLOWED_FLAGS: &[&str] = &["instrument-coverage"];

pub(crate) fn toml() -> toml::Value {
    let mut rustflags = vec!["--cfg", "err_span_check", "--verbose"];

    for &lint in IGNORED_LINTS {
        rustflags.push("-A");
        rustflags.push(lint);
    }

    let flags = std::env::var_os("RUSTFLAGS").map(|s| s.to_string_lossy().into_owned());
    if let Some(flags) = flags.as_ref() {
        let mut iter = flags.split_whitespace();
        while let Some(option) = iter.next() {
            if option == "-C"
                && let Some(flag) = iter.next()
                && ALLOWED_FLAGS.contains(&flag)
            {
                rustflags.push("-C");
                rustflags.push(flag);
            }
        }
    }

    toml::Value::try_from(rustflags).unwrap()
}
