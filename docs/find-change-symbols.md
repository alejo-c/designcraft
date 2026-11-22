# Find/Change symbols

- FindContext, crates/engine/src/cmd/find.rs:393, retains the original search domain and whether the current selection was replaced. Uses: Selection, Matcher, Target. Used by: scope, change, find_next.
- FindContext::after_change, crates/engine/src/cmd/find.rs:420, adjusts search bounds and continuation after a replacement. Uses: DocState. Used by: change.
- FindContext::restart, crates/engine/src/cmd/find.rs:442, restarts a matcher without retaining replacement state. Uses: target_text. Used by: scope.
- scope, crates/engine/src/cmd/find.rs:497, captures or reuses search bounds and continuation. Uses: FindContext. Used by: find, change, find_next.
- change, crates/engine/src/cmd/find.rs:610, replaces matches and advances repeated single replacements. Uses: scope, find_next, reveal_match, FindContext::after_change. Used by: find.change.
- find_next, crates/engine/src/cmd/find.rs:759, selects and reveals the next match. Uses: scope, reveal_match. Used by: find.next, change.
- reveal_match, crates/engine/src/cmd/find.rs:809, resolves the containing frame and requests navigation to composed match coordinates. Uses: caret, cell_caret, note_caret, PathTextMap, UiRequest::View. Used by: find_next, change.
- apply_view_request, crates/ui-egui/src/canvas/view.rs:365, centers requested match bounds in the canvas area clear of the dialog while preserving zoom and rotation. Uses: CanvasLayout, Xf. Used by: DesignApp::after_engine.
- show, crates/ui-egui/src/dialogs.rs:3250, presents Find/Change controls and replacement feedback with a polite live region. Uses: DesignApp::run. Used by: DesignApp::ui.
- repeated_change_advances_past_replacements_in_the_original_selection, crates/engine/src/cmd/find/tests.rs:982, guards repeated replacements, deletion, unchanged replacements and exhaustion. Uses: scope_session, scope_select, scope_hit. Used by: engine tests.
- change_and_change_find_change_the_found_text, crates/ui-egui/src/dialogs/tests.rs:5668, guards dialog actions, repeated keyboard replacement, retained button focus and accessible feedback. Uses: Harness. Used by: UI tests.
- find_next_centres_the_match_on_its_threaded_page_and_preserves_zoom, crates/ui-egui/src/dialogs/tests.rs:5745, guards cross-page navigation at rotated views. Uses: CanvasLayout, Xf. Used by: UI tests.
