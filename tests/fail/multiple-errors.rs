///// explicit compile error /////
compile_error!("ERROR");

//~~~~~~~~~~~~~~~~~~~~ errors ~~~~~~~~~~~~~~~~~~~~//

// error: ERROR
//  --> multiple-errors.rs:1:1
//   |
// 1 | compile_error!("ERROR");
//   | ^^^^^^^^^^^^^^^^^^^^^^^

///// second error in the same file /////
fn nothing() {
    let _ = compile_error!("SECOND ERROR");
    //~     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ SECOND ERROR
}

///// and a third one /////
compile_error!("THIRD ERROR");
//~~~~~~~~~~~~~~~~~~~~ errors ~~~~~~~~~~~~~~~~~~~~//

// error: THIRD ERROR
//  --> multiple-errors.rs:1:1
//   |
// 1 | compile_error!("THIRD ERROR");
//   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
