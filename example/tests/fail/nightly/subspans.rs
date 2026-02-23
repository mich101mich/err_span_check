use example::*;

#[rustfmt::skip]
fn main() {
    ////////// basic macro: subspan //////////
    my_basic_macro!("a random string that contains a ! somewhere in the middle", u32, 42);

    //~~~~~~~~~~~~~~~~~~~~ errors ~~~~~~~~~~~~~~~~~~~~//

    // error: proc macro panicked
    //  --> nightly/subspans.rs:5:5
    //   |
    // 5 |     my_basic_macro!("a random string that contains a ! somewhere in the middle", u32, 42);
    //   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    //   |
    //   = help: message: the literal should not contain '!'

    ////////// advanced macro: subspan //////////
    my_advanced_macro!("a random string that contains a ! somewhere in the middle", u32, 42);
    //~                                                 ^ the literal should not contain '!'

    ////////////////////////////////////////
}
