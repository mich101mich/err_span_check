test_normalize! {"
error[E0277]: the trait bound `QueryParams: serde::de::Deserialize<'de>` is not satisfied
   --> tests\\ui\\error_1_2.rs:22:61
" "
error[E0277]: the trait bound `QueryParams: serde::de::Deserialize<'de>` is not satisfied
 --> tests/ui/error.rs:22:61
"}
