///// explicit compile error /////
compile_error!("ERROR");

////////////////////////////////////////////////////////////////////////////////
// error: ERROR
//  --> tests/compile-fail-2_1_0.rs:1:1
//   |
// 1 | compile_error!("ERROR");
//   | ^^^^^^^^^^^^^^^^^^^^^^^
//
///// second error in the same file /////
fn nothing() {
    let _ = compile_error!("SECOND ERROR");
    //~     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ SECOND ERROR
}

////////////////////////////////////////////////////////////////////////////////
