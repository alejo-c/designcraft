# Hyperlinks and Bookmarks panels

- hyperlinks, crates/ui-egui/src/panels/interactive.rs:21, reserves Delete before truncating names; tooltips retain full names and resolve text anchor destinations. Uses: anchor.list, hyperlink.list, hyperlink.delete, hyperlink.goToSource, dest_text. Used by: dock panel dispatch.
- bookmarks, crates/ui-egui/src/panels/interactive.rs:157, reserves Delete and Rename before truncating names; tooltips retain full names and destination status; unresolved anchors disable navigation. Uses: bookmark.list, bookmark.delete, bookmark.rename, go_to_anchor, canvas.go_to_page. Used by: dock panel dispatch.
- long_names_leave_delete_visible_in_bookmarks_and_hyperlinks, crates/ui-egui/src/panels/interactive.rs:514, checks accessible full names, truncated drawing, visible deletion controls and deletion in narrow panels; page bookmarks use an explicit page destination. Uses: egui_kittest.Harness, bookmarks, hyperlinks. Used by: UI crate unit tests.
