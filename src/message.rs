#[macro_use]
pub(crate) mod term;

use crate::*;

const LINE: &str = "------------------------------------------------------------";

pub(crate) fn fail(err: Error) {
    print_col!(BoldRed => "ERROR");
    println_col!(": {}", err);
    println_col!();
}

pub(crate) fn no_tests() {
    println_col!(Yellow => "There are no err_span_check tests.");
}

pub(crate) fn no_tests_enabled() {
    println_col!(Yellow => "No tests matched to provided filters.");
}

/// Print the beginning of a test case.
pub(crate) fn begin_test(name: &str, path: &Path, line: usize) {
    print_col!("test ");
    print_col!(Bold => "{name} ({}:{line})", path.display());
    print_col!(" ... ");
}

/// Complete the test case with an "ok" message.
pub(crate) fn ok() {
    println_col!(Green => "ok");
}

/// Complete the test case with a "failed" message because a compile-fail test built successfully.
pub(crate) fn should_not_have_compiled() {
    println_col!(BoldRed => "error");
    println_col!(Red => "Expected test case to fail to compile, but it succeeded.");
}

/// Complete the test case with a "failed" message because of a mismatch.
pub(crate) fn mismatch(expected: &str, actual: &str) {
    println_col!(BoldRed => "mismatch");
    println_col!();
    println_col!(BoldBlue => "EXPECTED:");
    println_col!(Blue => "{LINE}\n{expected}\n{LINE}");
    println_col!();
    println_col!(BoldRed => "ACTUAL OUTPUT:");
    println_col!(Red => "{LINE}\n{actual}\n{LINE}");
    print_col!("note: If the ");
    print_col!(Red => "actual output");
    println_col!(" is the correct output you can bless it by rerunning");
    println_col!("      your test with the environment variable ERR_SPAN_CHECK=overwrite");
    println_col!();
}

/// Complete the test case with an "updating" message because the test is being updated.
pub(crate) fn updated(path: &Path) {
    println_col!(BoldYellow => "updating");
    println_col!(Yellow => "Test at {} will be updated to match actual output.", path.display());
}
