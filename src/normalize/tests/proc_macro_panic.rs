test_normalize! {
    INPUT="tests\\ui\\nonzero_fail.rs"
    OUTPUT="tests/ui/nonzero_fail.rs"
"
error[E0080]: evaluation of constant value failed
 --> tests\\ui\\nonzero_fail.rs:7:10
  |
7 | #[derive(NonZeroRepr)]
  |          ^^^^^^^^^^^ the evaluated program panicked at 'expected non-zero discriminant expression', tests\\ui\\nonzero_fail.rs:7:10
" "
error[E0080]: evaluation of constant value failed
 --> tests/ui/nonzero_fail.rs:7:10
  |
7 | #[derive(NonZeroRepr)]
  |          ^^^^^^^^^^^ the evaluated program panicked at 'expected non-zero discriminant expression', tests/ui/nonzero_fail.rs:7:10
"}
