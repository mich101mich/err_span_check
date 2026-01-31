#[macro_use]
pub(crate) mod term;

use crate::*;

const LINE: &str = "------------------------------------------------------------";

pub(crate) fn fail(err: Error) {
    if err.already_printed() {
        return;
    }

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

pub(crate) fn ok() {
    println_col!(Green => "ok");
}

pub(crate) fn begin_test(test: &Path) {
    let display_name = test.display();

    print_col!("test ");
    print_col!(Bold => "{}", display_name);
    print_col!(" ... ");
}

pub(crate) fn should_not_have_compiled() {
    println_col!(BoldRed => "error");
    println_col!(Red => "Expected test case to fail to compile, but it succeeded.");
    println_col!();
}

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

pub(crate) fn fail_output(stdout: &str) {
    if stdout.is_empty() {
        println_col!();
        return;
    }

    let normalized = normalize::trim(stdout);
    println_col!(BoldRed => "STDOUT:");
    println_col!(Red => "{LINE}\n{normalized}\n{LINE}");
    println_col!();
}

pub(crate) fn warnings(warnings: &str) {
    if warnings.is_empty() {
        return;
    }

    println_col!(BoldYellow => "WARNINGS:");
    println_col!(Yellow => "{LINE}\n{}\n{LINE}", warnings);
    println_col!();
}
