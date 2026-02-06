test_normalize! {"
error: `self` parameter is only allowed in associated functions
  --> tests/ui/error_1_2.rs:11:23
   |
11 | async fn bad_endpoint(self) -> Result<HttpResponseOkObject<()>, HttpError> {
   |                       ^^^^ not semantically valid as function parameter

error: aborting due to 2 previous errors

For more information about this error, try `rustc --explain E0401`.
error: could not compile `err_span_check-tests`.

To learn more, run the command again with --verbose.
" "
error: `self` parameter is only allowed in associated functions
  --> tests/ui/error.rs:11:23
   |
11 | async fn bad_endpoint(self) -> Result<HttpResponseOkObject<()>, HttpError> {
   |                       ^^^^ not semantically valid as function parameter
"}
