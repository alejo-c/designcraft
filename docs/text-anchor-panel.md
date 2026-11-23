# Text anchor panel

- text_anchor_target, crates/ui-egui/src/panels/interactive.rs:152, resolves selected story text or the start of a selected frame's composed range and the initial name. Uses: TextSel, Cache.get, Item.text_frame. Used by: text_anchors.
- text_anchors, crates/ui-egui/src/panels/interactive.rs:167, scrolls single-row destinations above a fixed add footer and opens the shared name dialog. Uses: anchor.list, bookmark.list, bookmark.add, bookmark.delete, anchor.delete, go_to_anchor, text_anchor_target, icons.button, Dialog.new. Used by: dock.panel_body.
- scrolls_itself, crates/ui-egui/src/dock.rs:76, prevents outer scrolling around panels that keep their own fixed footers. Uses: panel ID. Used by: scrolled_body, PanelContent.body.
- show, crates/ui-egui/src/dialogs.rs:3250, renders the textAnchor name dialog with initial editor focus and Enter/Escape actions. Uses: Dialog.fields, TextEdit, confirm, cancel. Used by: application UI, panel interaction tests.
- confirm_part1, crates/ui-egui/src/dialogs.rs:4901, creates or renames a text anchor through commands, rejects a changed active document and retains command errors in the dialog. Uses: text.select, anchor.create, anchor.rename. Used by: confirm.

# Integration constraints

- The textAnchor dialog stores the original document UID and text selection; its numeric-looking names remain strings.
- Frame selection does not enter text editing until confirmation; cancelling preserves the selection.
- egui scroll areas enforce their minimum scrolling height even below max_height; the anchor list lowers that minimum to preserve the footer in short panels.
- Use cargo test -p designcraft-ui-egui text_anchor_panel_tests for panel layout, keyboard dialog, bookmark identity and creation regressions.
- DESIGNCRAFT_ANCHOR_PANEL_SHOT and DESIGNCRAFT_ANCHOR_NARROW_SHOT request offscreen panel screenshots from existing tests.
