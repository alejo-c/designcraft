# Bookmarks and text anchors

- Bookmark, crates/doc/src/lib.rs:1065, optional anchor ID overrides the legacy page destination. Uses: TextAnchor. Used by: bookmark commands and PDF outlines.
- Session, crates/engine/src/lib.rs:396, tracks the source document of cut text. Uses: cut_text_document. Used by: clipboard commands.
- copy_text, crates/engine/src/cmd/edit.rs:51, captures formatted text and clears cut provenance. Uses: Story.extract. Used by: edit.copy and edit.cut.
- paste_text, crates/engine/src/cmd/edit.rs:126, preserves missing anchor IDs for same-document cut moves; copies allocate fresh IDs. Uses: Story.insert_story. Used by: edit.paste.
- specs, crates/engine/src/cmd/interactive.rs:10, registers bookmark creation and resolves live page destinations when listing. Uses: Cache.xref_index. Used by: command registry.
- bookmark_uses, crates/engine/src/cmd/xref.rs:187, checks nested bookmarks before deleting an anchor. Uses: Bookmark.anchor. Used by: anchor.delete.
- bookmarks, crates/engine/src/cmd/toc.rs:319, retains page destinations for generated TOC outlines. Uses: Bookmark. Used by: toc.generate and toc.update.
- outline, crates/pdf/src/links.rs:254, resolves anchor pages before mapping to exported sheets. Uses: Cache.xref_index and sheet_of_page. Used by: export_pdf_with_report in crates/pdf/src/export/pipeline.rs.
- export_pdf_with_report, crates/pdf/src/export/pipeline.rs:155, passes the composition cache to outline resolution. Uses: outline. Used by: export_pdf.
- bookmarks, crates/ui-egui/src/panels/interactive.rs:72, creates bookmarks from selected story text and navigates to resolved pages. Uses: bookmark.add and bookmark.list. Used by: dock.
- text_anchors, crates/ui-egui/src/panels/interactive.rs:114, toggles bookmark membership beside each anchor. Uses: bookmark.add and bookmark.delete. Used by: dock.
