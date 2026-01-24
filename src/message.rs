use crate::error::Error;
use crate::{Test, normalize, term};
use std::path::Path;
use termcolor::Color::{self, *};

pub(crate) enum Level {
    Fail,
    Warn,
}

pub(crate) use self::Level::*;

pub(crate) fn prepare_fail(err: Error) {
    if err.already_printed() {
        return;
    }

    term::bold_color(Red);
    print!("ERROR");
    term::reset();
    println!(": {}", err);
    println!();
}

pub(crate) fn test_fail(err: Error) {
    if err.already_printed() {
        return;
    }

    term::bold_color(Red);
    println!("error");
    term::color(Red);
    println!("{}", err);
    term::reset();
    println!();
}

pub(crate) fn no_tests_enabled() {
    term::color(Yellow);
    println!("There are no err_span_check tests enabled yet.");
    term::reset();
}

pub(crate) fn ok() {
    term::color(Green);
    println!("ok");
    term::reset();
}

pub(crate) fn begin_test(test: &Test) {
    let display_name = test.path.as_os_str().to_string_lossy();

    print!("test ");
    term::bold();
    print!("{}", display_name);
    term::reset();

    print!(" ... ");
}

pub(crate) fn should_not_have_compiled() {
    term::bold_color(Red);
    println!("error");
    term::color(Red);
    println!("Expected test case to fail to compile, but it succeeded.");
    term::reset();
    println!();
}

pub(crate) fn write_stderr_wip(wip_path: &Path, stderr_path: &Path, stderr: &str) {
    let wip_path = wip_path.to_string_lossy();
    let stderr_path = stderr_path.to_string_lossy();

    term::bold_color(Yellow);
    println!("wip");
    println!();
    print!("NOTE");
    term::reset();
    println!(": writing the following output to `{}`.", wip_path);
    println!(
        "Move this file to `{}` to accept it as correct.",
        stderr_path,
    );
    snippet(Yellow, stderr);
    println!();
}

pub(crate) fn overwrite_stderr(stderr_path: &Path, stderr: &str) {
    let stderr_path = stderr_path.to_string_lossy();

    term::bold_color(Yellow);
    println!("wip");
    println!();
    print!("NOTE");
    term::reset();
    println!(": writing the following output to `{}`.", stderr_path);
    snippet(Yellow, stderr);
    println!();
}

pub(crate) fn mismatch(expected: &str, actual: &str) {
    term::bold_color(Red);
    println!("mismatch");
    term::reset();
    println!();
    term::bold_color(Blue);
    println!("EXPECTED:");
    snippet(Blue, expected);
    println!();
    term::bold_color(Red);
    println!("ACTUAL OUTPUT:");
    snippet(Red, actual);
    print!("note: If the ");
    term::color(Red);
    print!("actual output");
    term::reset();
    println!(" is the correct output you can bless it by rerunning");
    println!("      your test with the environment variable TRYBUILD=overwrite");
    println!();
}

pub(crate) fn fail_output(level: Level, stdout: &str) {
    let color = match level {
        Fail => Red,
        Warn => Yellow,
    };

    if !stdout.is_empty() {
        term::bold_color(color);
        println!("STDOUT:");
        snippet(color, &normalize::trim(stdout));
        println!();
    }
}

pub(crate) fn warnings(warnings: &str) {
    if warnings.is_empty() {
        return;
    }

    term::bold_color(Yellow);
    println!("WARNINGS:");
    snippet(Yellow, warnings);
    println!();
}

fn snippet(color: Color, content: &str) {
    term::color(color);
    println!("{:┈<60}", "");

    print!("{content}");

    term::color(color);
    println!("{:┈<60}", "");
    term::reset();
}
