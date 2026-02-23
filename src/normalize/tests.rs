macro_rules! unwrap_or {
    ( $( $value:literal )?, $default:literal ) => {
        {
            // If $value is not present, this will just be a block containing $default.
            // If $value is present, this will be a block containing a useless `$default;` statement,
            // followed by $value, which evaluates to $value.
            $default $(; $value)?
        }
    };
}

macro_rules! test_normalize {
    (
        $(WORKSPACE=$workspace:literal)?
        $(INPUT=$input:literal)?
        $(OUTPUT=$output:literal)?
        $(TARGET=$target:literal)?
        $original:literal
        $expected:literal
    ) => {
        #[test]
        fn test() {
            use std::path::PathBuf;
            let project = crate::Project {
                dir: PathBuf::new(),
                target_dir: PathBuf::from(unwrap_or!($($target)?, "/git/err_span_check/target")),
                name: "err_span_check000".to_string(),
                should_update: false,
                features: None,
                workspace: PathBuf::from(unwrap_or!($($workspace)?, "/git/err_span_check")),
                path_dependencies: vec![crate::project::PathDependency {
                    name: "diesel".to_string(),
                    normalized_path: PathBuf::from("/home/user/documents/rust/diesel/diesel"),
                }],
            };
            let local_path = PathBuf::from(unwrap_or!($($input)?, "tests/ui/error_1_2.rs"));
            let replaced_path = PathBuf::from(unwrap_or!($($output)?, "tests/ui/error.rs"));

            let normalizer = crate::normalize::Normalizer::new(&project, &local_path, &replaced_path);

            let normalized = normalizer.diagnostics($original);
            let expected = $expected;
            if normalized != expected {
                panic!("\nACTUAL: \"{}\"\nEXPECTED: \"{}\"", normalized, expected);
            }
        }
    };
}

mod and_n_others;
mod and_others_verbose;
mod basic;
mod cargo_registry;
mod cargo_registry_sparse;
mod consteval;
mod dir_backslash;
mod dropshot_required_by;
mod erased_serde_trait_bound;
mod gated_feature;
mod ghost_note_help;
mod long_file_names;
mod multiline_note;
mod proc_macro_panic;
mod py03_url;
mod right_aligned_line_number;
mod rust_lib;
mod rust_lib_with_githash;
mod strip_path_dependencies;
mod traits_must_be_implemented;
mod type_dir_backslash;
mod uniffi_out_dir;
