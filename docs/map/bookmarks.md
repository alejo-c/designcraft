# Bookmarks and text anchors

- Bookmark, crates/doc/src/lib.rs:1065, optional anchor ID overrides the legacy page destination. Uses: TextAnchor. Used by: bookmark commands and PDF outlines.
- Session, crates/engine/src/lib.rs:396, tracks the source document of cut text. Uses: cut_text_document. Used by: clipboard commands.
- copy_text, crates/engine/src/cmd/edit.rs:51, captures formatted text and clears cut provenance. Uses: Story.extract. Used by: edit.copy and edit.cut.
- paste_text, crates/engine/src/cmd/edit.rs:126, preserves missing anchor IDs for same-document cut moves; copies allocate fresh IDs. Uses: Story.insert_story. Used by: edit.paste.
- specs, crates/engine/src/cmd/interactive.rs:10, registers bookmark creation and resolves live page destinations when listing. Uses: Cache.xref_index. Used by: command registry.
- AnchorPlace, crates/compose/src/xref.rs:15, composed destination of a text anchor: absolute page, caret point and line top in spread space. Uses: Point. Used by: XrefIndex and anchor.list.
- anchor_place, crates/compose/src/xref.rs:76, resolves a story byte to its page and spread points through caret geometry, path text and parent transforms. Uses: caret, PathTextMap, Document.parent_xf and text_xf. Used by: XrefIndex.build and anchor.list.
- XrefIndex.place, crates/compose/src/xref.rs:70, returns the composed AnchorPlace for an anchor. Uses: composition-built index. Used by: PDF links and outlines.
- delete_anchor, crates/engine/src/cmd/xref.rs:165, optionally removes linked bookmarks atomically while preserving surviving children and protecting hyperlink/xref destinations. Uses: bookmark_uses, Story.delete, Document.adjust_text_ranges. Used by: anchor.delete.
- bookmark_uses, crates/engine/src/cmd/xref.rs:203, checks nested bookmarks before deleting an anchor. Uses: Bookmark.anchor. Used by: anchor.delete.
- list_anchors, crates/engine/src/cmd/xref.rs:227, sorts resolved anchors by page/x/y with unresolved destinations last. Uses: Cache.get and xref::anchor_place. Used by: anchor.list.
- bookmarks, crates/engine/src/cmd/toc.rs:319, retains page destinations for generated TOC outlines. Uses: Bookmark. Used by: toc.generate and toc.update.
- anchor_destination, crates/pdf/src/links.rs:29, maps an anchor's composed line top to a sheet-relative PDF destination. Uses: XrefIndex.place and sheet_of_page. Used by: PDF link annotations and outlines.
- outline, crates/pdf/src/links.rs:266, resolves anchored bookmarks to their line and legacy page bookmarks to their page. Uses: Cache.xref_index and anchor_destination. Used by: export_pdf_with_report in crates/pdf/src/export/pipeline.rs.
- export_pdf_with_report, crates/pdf/src/export/pipeline.rs:155, passes the composition cache to outline resolution. Uses: outline. Used by: export_pdf.
- bookmarks, crates/ui-egui/src/panels/interactive.rs:72, creates text bookmarks, crops long names and navigates to text or legacy page destinations. Uses: bookmark.add, bookmark.list, go_to_anchor. Used by: dock.
- go_to_anchor, crates/ui-egui/src/panels/interactive.rs:118, selects the live text destination and centers its caret while preserving zoom. Uses: Cache.get, caret, text.select, canvas.go_to_spread_rect. Used by: bookmarks and text_anchors.
- text_anchors, crates/ui-egui/src/panels/interactive.rs:152, crops names beside visible deletion controls and toggles bookmark membership. Uses: anchor.list, anchor.delete with removeBookmarks, bookmark.add, bookmark.delete, go_to_anchor. Used by: dock.