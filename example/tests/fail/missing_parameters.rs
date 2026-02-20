use example::*;

fn main() {
    ////////// no parameters //////////
    my_basic_macro!();
    my_advanced_macro!();

    //~~~~~~~~~~~~~~~~~~~~ errors ~~~~~~~~~~~~~~~~~~~~//

    // error: unexpected end of input, expected string literal
    //  --> missing_parameters.rs:4:5
    //   |
    // 4 |     my_basic_macro!();
    //   |     ^^^^^^^^^^^^^^^^^
    //   |
    //   = note: this error originates in the macro `my_basic_macro` (in Nightly builds, run with -Z macro-backtrace for more info)

    // error: my_advanced_macro! requires 3 parameters: a string literal, a type, and an expression
    //  --> missing_parameters.rs:5:5
    //   |
    // 5 |     my_advanced_macro!();
    //   |     ^^^^^^^^^^^^^^^^^^^^
    //   |
    //   = note: this error originates in the macro `my_advanced_macro` (in Nightly builds, run with -Z macro-backtrace for more info)

    ////////// basic macro: one parameter //////////
    my_basic_macro!("hi");
    my_basic_macro!("hi",);

    //~~~~~~~~~~~~~~~~~~~~ errors ~~~~~~~~~~~~~~~~~~~~//

    // error: expected `,`
    //  --> missing_parameters.rs:4:5
    //   |
    // 4 |     my_basic_macro!("hi");
    //   |     ^^^^^^^^^^^^^^^^^^^^^
    //   |
    //   = note: this error originates in the macro `my_basic_macro` (in Nightly builds, run with -Z macro-backtrace for more info)

    // error: unexpected end of input, expected one of: `for`, parentheses, `fn`, `unsafe`, `extern`, identifier, `::`, `<`, `dyn`, square brackets, `*`, `&`, `!`, `impl`, `_`, lifetime
    //  --> missing_parameters.rs:5:5
    //   |
    // 5 |     my_basic_macro!("hi",);
    //   |     ^^^^^^^^^^^^^^^^^^^^^^
    //   |
    //   = note: this error originates in the macro `my_basic_macro` (in Nightly builds, run with -Z macro-backtrace for more info)

    ////////// basic macro: two parameters //////////
    my_basic_macro!("hi", u32);
    my_basic_macro!("hi", u32,);

    //~~~~~~~~~~~~~~~~~~~~ errors ~~~~~~~~~~~~~~~~~~~~//

    // error: expected `,`
    //  --> missing_parameters.rs:4:5
    //   |
    // 4 |     my_basic_macro!("hi", u32);
    //   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^
    //   |
    //   = note: this error originates in the macro `my_basic_macro` (in Nightly builds, run with -Z macro-backtrace for more info)

    // error: unexpected end of input, expected an expression
    //  --> missing_parameters.rs:5:5
    //   |
    // 5 |     my_basic_macro!("hi", u32,);
    //   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    //   |
    //   = note: this error originates in the macro `my_basic_macro` (in Nightly builds, run with -Z macro-backtrace for more info)

    ////////// basic macro: three parameters //////////
    my_basic_macro!("hi", u32, 42); // ok
    my_basic_macro!("hi", u32, 42, "extra");
    //~                          ^ unexpected token

    ////////// advanced macro: various parameters //////////
    my_advanced_macro!("hi");
    //~                    ^ expected 3 parameters, found 1. Missing: a type and an expression
    my_advanced_macro!("hi",);
    //~                     ^ expected 3 parameters, found 1. Missing: a type and an expression
    my_advanced_macro!("hi", u32);
    //~                         ^ expected 3 parameters, found 2. Missing: an expression
    my_advanced_macro!("hi", u32,);
    //~                          ^ expected 3 parameters, found 2. Missing: an expression
    my_advanced_macro!("hi", u32, 42); // ok
    my_advanced_macro!("hi", u32, 42, "extra");
    //~                             ^^^^^^^^^ expected exactly 3 parameters, but found extra inputs

    ////////////////////////////////////////
}
