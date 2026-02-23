use crate::*;

#[derive(Deserialize)]
struct Build {
    #[serde(deserialize_with = "from_json")]
    features: Vec<String>,
}

pub(crate) fn find() -> Option<Vec<String>> {
    // This will look something like:
    //   /path/to/crate_name/target/debug/deps/test_name-HASH
    let test_binary = std::env::args_os().next()?;

    // The hash at the end is ascii so not lossy, rest of conversion doesn't
    // matter.
    let test_binary_lossy = test_binary.to_string_lossy();
    let test_binary_lossy = test_binary_lossy
        .strip_suffix(".exe")
        .unwrap_or(&test_binary_lossy);

    // '-' + 16 hex digits
    let dash_pos = test_binary_lossy.len().checked_sub(17)?;
    let hash = &test_binary_lossy[dash_pos..];
    if !hash.starts_with('-') || !hash[1..].bytes().all(is_lower_hex_digit) {
        return None;
    }

    let binary_path = PathBuf::from(&test_binary);

    // Feature selection is saved in:
    //   /path/to/crate_name/target/debug/.fingerprint/*-HASH/*-HASH.json
    let up = binary_path.parent()?.parent()?;
    let fingerprint_dir = up.join(".fingerprint");
    if !fingerprint_dir.is_dir() {
        return None;
    }

    let hash_matches: Vec<_> = fingerprint_dir
        .read_dir()
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_ok_and(|ft| ft.is_dir()))
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(hash))
        .map(|entry| entry.path())
        .collect();

    let [hash_match] = &hash_matches[..] else {
        return None;
    };

    let json_matches: Vec<_> = hash_match
        .read_dir()
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_ok_and(|ft| ft.is_file()))
        .filter(|entry| entry.path().extension().is_some_and(|s| s == "json"))
        .map(|entry| entry.path())
        .collect();

    let [json_match] = &json_matches[..] else {
        return None;
    };

    let build_json = std::fs::read_to_string(json_match).ok()?;
    let build: Build = serde_json::from_str(&build_json).ok()?;
    Some(build.features)
}

fn is_lower_hex_digit(byte: u8) -> bool {
    matches!(byte, b'0'..=b'9' | b'a'..=b'f')
}

fn from_json<'de, T, D>(deserializer: D) -> std::result::Result<T, D::Error>
where
    T: serde::de::DeserializeOwned,
    D: Deserializer<'de>,
{
    let json = String::deserialize(deserializer)?;
    serde_json::from_str(&json).map_err(serde::de::Error::custom)
}
