macro_rules! test_normalize {
    (
        $(DIR=$dir:literal)?
        $(WORKSPACE=$workspace:literal)?
        $(INPUT=$input:literal)?
        $(TARGET=$target:literal)?
        $original:literal
        $expected:literal
    ) => {
        #[test]
        fn test() {
            let context = crate::normalize::Context {
                krate: "err_span_check000",
                input_file: std::path::Path::new({ "tests/ui/error.rs" $(; $input)? }),
                source_dir: std::path::Path::new({ "/git/err_span_check/test_suite" $(; $dir)? }),
                workspace: std::path::Path::new({ "/git/err_span_check" $(; $workspace)? }),
                target_dir: std::path::Path::new({ "/git/err_span_check/target" $(; $target)? }),
                path_dependencies: &[crate::project::PathDependency {
                    name: String::from("diesel"),
                    normalized_path: std::path::PathBuf::from("/home/user/documents/rust/diesel/diesel"),
                }],
            };
            let original = $original;
            let variations = crate::normalize::diagnostics(original, context);
            let preferred = variations.preferred();
            let expected = $expected;
            if preferred != expected {
                panic!("\nACTUAL: \"{}\"\nEXPECTED: \"{}\"", preferred, expected);
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
