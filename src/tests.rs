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
                krate: "trybuild000",
                input_file: std::path::Path::new({ "tests/ui/error.rs" $(; $input)? }),
                source_dir: std::path::Path::new({ "/git/err_span_check/test_suite" $(; $dir)? }),
                workspace: std::path::Path::new({ "/git/err_span_check" $(; $workspace)? }),
                target_dir: std::path::Path::new({ "/git/err_span_check/target" $(; $target)? }),
                path_dependencies: &[crate::run::PathDependency {
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

mod tests {
    automod::dir!("src/tests");
}
