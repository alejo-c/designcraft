# Canvas caret

- start_id, crates/ui-egui/src/canvas/caret.rs:7, blink timestamp storage key. Uses: egui::Id. Used by: reset, visible.
- caret_id, crates/ui-egui/src/canvas/caret.rs:11, previous selection storage key. Uses: egui::Id. Used by: visible.
- reset, crates/ui-egui/src/canvas/caret.rs:15, restart the visible phase after existing input actions. Uses: start_id, egui input time. Used by: canvas/input.rs, canvas/tests.rs.
- visible, crates/ui-egui/src/canvas/caret.rs:20, determine visibility and schedule the next blink only for a focused window. Uses: start_id, caret_id, until_flip, TextSel, egui::Context. Used by: canvas/text_overlay.rs, canvas/tests.rs.
- until_flip, crates/ui-egui/src/canvas/caret.rs:43, delay until the next blink boundary. Uses: BLINK_HZ. Used by: visible, canvas/tests.rs.
- the_caret_blink_restarts_on_input_and_caret_changes, crates/ui-egui/src/canvas/tests.rs:3445, preserve existing restart behavior. Uses: reset, visible, until_flip. Used by: cargo test.
- the_caret_stops_requesting_repaints_in_an_unfocused_window, crates/ui-egui/src/canvas/tests.rs:3476, verify inactive windows schedule no caret redraws and active windows resume them. Uses: visible, egui::Context::repaint_causes. Used by: cargo test.
