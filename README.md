# Error Span Checker

[![Tests](https://github.com/mich101mich/err_span_check/actions/workflows/test.yml/badge.svg)](https://github.com/mich101mich/err_span_check/actions/workflows/test.yml)
[![Crates.io](https://img.shields.io/crates/v/err_span_check.svg)](https://crates.io/crates/err_span_check)
[![Documentation](https://docs.rs/err_span_check/badge.svg)](https://docs.rs/err_span_check/)
[![Dependency status](https://deps.rs/repo/github/mich101mich/err_span_check/status.svg)](https://deps.rs/repo/github/mich101mich/err_span_check)

A test harness for checking and comparing compiler errors with a focus on error spans. Useful mainly for procedural
macros, but can also be used in other contexts.

This crate is a fork of [trybuild] with in-file error message syntax rather than separate `.stderr` files. It also has
a more specialized and rigid workflow, so if any part of this process does not fit your project, please check out the
more general-purpose [trybuild] instead.

[trybuild]: https://crates.io/crates/trybuild

## Goal

The rust compiler produces some of the best error messages of any comparable tool in the industry. And when you create
a [procedural macro], you are in a position where your errors are rendered as compiler errors. So as an author of
a proc-macro, you might want to put extra effort into making your errors as helpful as possible.

This can pose a serious challenge though, as proc-macro development is fairly niche and many of the required tools are
still waiting for stabilization (see [1], [2]). One of the core challenges is settings [`Span`]s correctly, aka the
region of the source code that is underlined by the error. This crate provides a simple way to verify that your
macro underlines the correct part of the code with its errors.

[procedural macro]: https://doc.rust-lang.org/stable/book/ch20-05-macros.html
[1]: https://github.com/rust-lang/rust/issues/54725
[2]: https://github.com/rust-lang/rust/issues/54140
[`Span`]: https://doc.rust-lang.org/proc_macro/struct.Span.html

## Workflow

 1. Have a Project where you want to check compiler errors
 2. Create (minimal) code that produces the desired compiler error

    This code should be placed in a `.rs` file somewhere within the `tests/fail` directory

    Example (see [example] directory for full code):

    `tests/fail/missing_parameters.rs`

    ```rust
    fn main() {
        my_crate::my_string_checker!(usize); // passing a type to a macro that requires a string
    }
    ```

 3. Declare and name the test cases within the code

    ```rust
    ////////// wrong parameter //////////
    fn main() {
        my_crate::my_string_checker!(usize);
    }
    ```

    Each test case has to start with at least 5 `/////`, followed by some text to name the test case.

 4. (Optional): Add more test cases

    ```rust
    use my_crate::my_string_checker;
    fn main() {
        ////////// wrong parameter //////////
        my_string_checker!(usize);

        ////////// no parameters //////////
        my_string_checker!();

        ////////////////////////////////////////
    }
    ```

    A test case ends at the start of the next test case or a line consisting of only `/////`...\
    Any code outside of a test case is shared between all test cases in the file. Note that test cases can't "see"
    each other.

 5. Stage/commit your changes

    **Note: This crate requires tracking your tests with git!**

    All test cases (aka the contents of the `tests/fail` dir) have to be at least staged in git. Otherwise, they won't
    be tested.

    ```sh
    git add tests/fail
    ```

    This step is necessary because there is little value in trying to compare large blocks of compiler errors in the
    console. Instead, this crate overwrites the test files with the correct error annotations, and you can use the git
    diff viewer of your choice to inspect and accept/undo the changes.

 6. Run the tests

    Add `err_span_check` to your **dev**-dependencies.

    ```sh
    cargo add --dev err_span_check
    ```

    Place the following test somewhere within your code or regular rust test files:

    ```rust
    #[test]
    fn compile_error_tests() {
        err_span_check::run_on_fail_dir();
    }
    ```

    Then run

    ```sh
    cargo test
    ```

 7. Inspect the error annotations

    Running the tests will overwrite the file to look something like this:

    ```rust
    use my_crate::my_string_checker;
    fn main() {
        ////////// wrong parameter //////////
        my_string_checker!(usize);
        //~                ^^^^^ expected string literal

        ////////// no parameters //////////
        my_string_checker!();

        //~~~~~~~~~~~~~~~~~~~~ errors ~~~~~~~~~~~~~~~~~~~~//

        // error: macro `my_string_checker` requires a string literal as parameter
        //  --> missing_parameters.rs:5:5
        //   |
        // 5 |     my_string_checker!();
        //   |     ^^^^^^^^^^^^^^^^^^^^
        //   |
        //   = note: this error originates in the macro `my_string_checker` (in Nightly builds, run with -Z macro-backtrace for more info)

        ////////////////////////////////////////
    }
    ```

    The errors are placed in the line(s) below their occurrence if possible, using a line starting with `//~`.\
    If the error does not fit, it is instead placed in a separate `errors` block containing the full (normalized to
    somewhat resist compiler version changes) output of the compiler.

[example]: https://github.com/mich101mich/err_span_check/tree/master/example

## Specification

### Project Structure

- There must be a directory named `fail` within the project's `tests` folder.
  - The `tests` folder must be adjacent to the `Cargo.toml` as per cargo's specification.
- There must be a git repository set up at some level above the `fail` folder.\
  (using the project or workspace root as the repo root is recommended)
- Within the `fail` folder there can be any number and depth of subfolders.
- Subfolders named `stable` or `nightly` indicate that the tests within them should only be run on the corresponding
  toolchain.
  - **NOTE**: err_span_check ensures that stable and nightly use the same tests by always copying them from the
    `stable` directories!\
    When you want to update the tests, update them in the `stable` folder and then run nightly tests
    (`cargo +nightly test`) to update the nightly tests.

### Test Files

- Any file under the `tests/fail` directory is counted as a test file.
- Test files must have the `.rs` extension and a non-empty utf-8 filename.
- Test files can contain arbitrary Rust code and any number of test cases.

### Test Cases

- Test files are scanned for lines starting with the special sequences `"/////"` and `//~`.\
  The sequences can be indented with any number of [whitespace] (including none).
- A test case starts with a line consisting of [whitespace], at least five `'/'` characters, a name, and optionally
  more whitespace and `'/'` chars.\
  Formally, they have to match the following regex: `^\s*/{5,}\s*(?<name>.*)\s*/*\s*$`\
  (whitespace, 5+ slashes, whitespace, name, whitespace, slashes, whitespace)\
  The extra steps are to allow padding the names and trailing slashes.\
  Examples:

  ```rust
  // valid starts:
  ///// a name /////
  //////////////////////////////any number of slashes, nothing after it
  ///// name with slash/ /////

  // The above cases would be named "a name", "any number of slashes, nothing after it", and "name with slash/".

  // invalid: Not enough slashes
  /// a name ///
  // invalid: No name
  ////////////////////////////////////////
  ```

- A test case ends with the next line starting with at least `/////`.
  - If that line contains another name, it also serves as the start of the next test case.
  - Otherwise, it is just treated as the end of the current test case.
- Any code outside of test cases is treated as setup code and shared between all test cases in the same file.
- _Up to here is what the user needs to write. Everything else is added by running the tests and should not be written_
  _by hand!_
- Inline error annotations are lines starting with `//~`. They are ignored for the purposes of testing.
  - Note that inline annotations might be placed within multiline strings or similar invalid contexts. This is ok since
    those lines are fully removed when running the tests.
- Full error annotations are placed in comments after `//~~~~~~~~~~~~~~~~~~~~ errors ~~~~~~~~~~~~~~~~~~~~//`.
  - That line and everything after it is fully removed when running the tests.

Take the following example from the [workflow](#workflow) chapter:

```rust
use my_crate::my_string_checker;
fn main() {
    ////////// wrong parameter //////////
    my_string_checker!(usize);
    //~                ^^^^^ expected string literal

    ////////// no parameters //////////
    my_string_checker!();

    //~~~~~~~~~~~~~~~~~~~~ errors ~~~~~~~~~~~~~~~~~~~~//

    // error: macro `my_string_checker` requires a string literal as parameter
    //  --> missing_parameters.rs:5:5
    //   |
    // 5 |     my_string_checker!();
    //   |     ^^^^^^^^^^^^^^^^^^^^
    //   |
    //   = note: this error originates in the macro `my_string_checker` (in Nightly builds, run with -Z macro-backtrace for more info)

    ////////////////////////////////////////
}
```

This test file turns into the following test cases:

Name: "wrong parameter"\
Tested code:

```rust
use my_crate::my_string_checker;
fn main() {
    my_string_checker!(usize);

}
```

Name: "no parameters"\
Tested code:

```rust
use my_crate::my_string_checker;
fn main() {
    my_string_checker!();

}
```

[whitespace]: https://doc.rust-lang.org/std/primitive.char.html#method.is_whitespace

### Environment Variables

- `ERR_SPAN_CHECK` can be set to the value `frozen` to have the tests fail with their expected output rather than
  updating the local files. This is useful for CI and similar contexts.

## License
Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
