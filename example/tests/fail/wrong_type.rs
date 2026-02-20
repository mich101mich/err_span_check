use example::*;

fn main() {
    ////////// basic macro: wrong type //////////
    my_basic_macro!("hi", String, 42);

    my_basic_macro!("hi", String, 1 + 1);

    //~~~~~~~~~~~~~~~~~~~~ errors ~~~~~~~~~~~~~~~~~~~~//

    // error[E0308]: mismatched types
    //  --> wrong_type.rs:4:5
    //   |
    // 4 |     my_basic_macro!("hi", String, 42);
    //   |     ^^^^^^^^^^^^^^^^^^^^^^------^^^^^
    //   |     |                     |
    //   |     |                     expected due to this
    //   |     expected `String`, found integer
    //   |
    //   = note: this error originates in the macro `my_basic_macro` (in Nightly builds, run with -Z macro-backtrace for more info)
    // help: try using a conversion method
    //   |
    // 4 |     my_basic_macro!("hi", String, 42).to_string();
    //   |                                      ++++++++++++

    // error[E0308]: mismatched types
    //  --> wrong_type.rs:6:5
    //   |
    // 6 |     my_basic_macro!("hi", String, 1 + 1);
    //   |     ^^^^^^^^^^^^^^^^^^^^^^------^^^^^^^^
    //   |     |                     |
    //   |     |                     expected due to this
    //   |     expected `String`, found integer
    //   |
    //   = note: this error originates in the macro `my_basic_macro` (in Nightly builds, run with -Z macro-backtrace for more info)
    // help: try using a conversion method
    //   |
    // 6 |     my_basic_macro!("hi", String, 1 + 1).to_string();
    //   |                                         ++++++++++++

    ////////// advanced macro: wrong type //////////
    my_advanced_macro!("hi", String, 42);
    //~                              ^^ error: mismatched types
    //~                                 label: expected `String`, found integer

    my_advanced_macro!("hi", String, 1 + 1);
    //~                              ^^^^^ error: mismatched types
    //~                                    label: expected `String`, found integer

    ////////////////////////////////////////
}
