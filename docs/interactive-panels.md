# Hyperlinks and Bookmarks panels

- hyperlinks, crates/ui-egui/src/panels/interactive.rs:22, reserves the Delete button before truncating names; tooltips retain full names and destinations. Uses: hyperlink.list, hyperlink.delete, hyperlink.goToSource, dest_text. Used by: dock panel dispatch.
- bookmarks, crates/ui-egui/src/panels/interactive.rs:76, reserves the Delete button before truncating names; tooltips retain full names and page labels. Uses: bookmark.list, bookmark.delete, canvas.go_to_page. Used by: dock panel dispatch.
- long_names_leave_delete_visible_in_bookmarks_and_hyperlinks, crates/ui-egui/src/panels/interactive.rs:119, checks accessible full names, truncated drawing, visible deletion controls and deletion in narrow panels. Uses: egui_kittest.Harness, bookmarks, hyperlinks. Used by: UI crate unit tests.
