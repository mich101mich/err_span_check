#[test]
fn test() {
    let t = err_span_check::TestCases::new();
    t.test("tests/ui/compile-fail-0.rs");
    t.test("tests/ui/compile-fail-1.rs");
    t.test("tests/ui/compile-fail-2.rs");
    t.test("tests/ui/compile-fail-3.rs");
}
