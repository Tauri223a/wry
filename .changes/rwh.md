---
"wry": minor
---

Refactor new method to take raw window handle instead. Following are APIs got affected:
  - `application` module is removed, and `webivew` module is moved to root module.
  - `WebviewBuilder::new`, `Webview::new` now take `RawWindowHandle` instead.
  - Attributes `ipc_handler`, `file_drop_handler`, `document_change_handler` don't have window parameter anymore.
  Users should use closure to capture the types they want to use.
  - Position field in `FileDrop` event is now `Position` instead of `PhysicalPosition`. Users need to handle scale factor
  depend on the situation they have.
  - `Webview::inner_size` is removed.
  - Added `rwh_04`, `rwh_05`, `rwh_06` feature flags.