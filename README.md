Error Span Checker
========

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
       my_proc_macro!(usize); // passing a type to a macro that requires a string
   }
   ```

3. Declare and name the test cases within the code

   ```rust
   ////////// wrong parameter //////////
   fn main() {
       my_proc_macro!(usize);
   }
   ```

   Each test case has to start with at least 5 `/////`, followed by some text to name the test case.

4. (Optional): Add more test cases

   ```rust
   fn main() {
       ////////// wrong parameter //////////
       my_proc_macro!(usize);

       ////////// no parameters //////////
       my_proc_macro!();

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
   console, trying to look for a whitespace change. Instead, this crate overwrites the test files with the correct
   error annotations, and you can use the git diff viewer of your choice to inspect and accept/undo the changes.

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
   fn main() {
       ////////// wrong parameter //////////
       my_proc_macro!(usize);
       //~            ^^^^^ expected string literal

       ////////// no parameters //////////
       my_proc_macro!();

       //~~~~~~~~~~~~~~~~~~~~ errors ~~~~~~~~~~~~~~~~~~~~//

       // error: macro `my_proc_macro` requires a string literal as parameter
       //  --> missing_parameters.rs:5:5
       //   |
       // 5 |     my_proc_macro!();
       //   |     ^^^^^^^^^^^^^^^^
       //   |
       //   = note: this error originates in the macro `my_proc_macro` (in Nightly builds, run with -Z macro-backtrace for more info)

       ////////////////////////////////////////
   }
   ```

   The errors are placed in the line(s) below their occurrence if possible, using a line starting with `//~`.\
   If the error does not fit, it is instead placed in a separate `errors` block containing the full (normalized to
   somewhat resist compiler version changes) output of the compiler.

[example]: https://github.com/mich101mich/err_span_check/tree/master/example

## Specification

TODO: Things to talk about:
- Update by default, "frozen" for CI
- requires git
- file format
  - contain arbitrary code
  - at least 5 `/////` start a test case + text to name the case
  - next test can start immediately, or have meta-line without text to end it
  - test blocks are removed for other tests
- produces error annotations (not written by hand!)
  - `//~~~~~~~~~~~~~~~~~~~~ errors ~~~~~~~~~~~~~~~~~~~~//` to start external errors
  - `//~` marks an inline spanned message
    - line will be removed from actual test, so might appear in an invalid place
- stable/nightly


#### License

# License
Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
