use example::*;

fn main() {
    ////////// basic macro: wrong type //////////
    my_basic_macro!("hi", String, 42);
    //~                           ^^ error: mismatched types
    //~                              label: expected `String`, found integer

    my_basic_macro!("hi", String, 1 + 1);
    //~                           ^^^^^ error: mismatched types
    //~                                 label: expected `String`, found integer

    ////////// advanced macro: wrong type //////////
    my_advanced_macro!("hi", String, 42);
    //~                              ^^ error: mismatched types
    //~                                 label: expected `String`, found integer

    my_advanced_macro!("hi", String, 1 + 1);
    //~                              ^^^^^ error: mismatched types
    //~                                    label: expected `String`, found integer

    ////////////////////////////////////////
}
