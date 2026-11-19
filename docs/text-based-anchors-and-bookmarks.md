# Text-based anchors and bookmarks

This guide describes the implementation introduced by commit ea999c84, ANCHOR2: make text bookmarks follow their anchors. Integrations need a DesignCraft build containing that change. It does not describe a new native format version or promise support in older builds.

## What changed in the data model

- Bookmark gained anchor: Option<u64>. In serialized document JSON its key is anchor, containing the numeric ID of a TextAnchor. None defaults when the field is absent and is omitted when saving.
- Bookmark still has name: String, page: usize, children: Vec<Bookmark>, and generated_by_toc: bool. The JSON key for the ownership flag is generatedByToc.
- No bookmark destination enum was introduced. When anchor is present, it takes precedence over page. When anchor is absent or null, the bookmark is a legacy page bookmark.
- TextAnchor was not changed. Its persisted fields remain id: u64 and name: String. It has no page, position, selected-text string, or bookmark flag.
- Document.bookmarks remains Vec<Bookmark>. No top-level Document field was added. Its elements now support the additional anchor field.
- Story.anchors remains Vec<Arc<TextAnchor>>. Its position mechanism is unchanged: each anchor corresponds to an ANCHOR_MARK character, U+E00B, in Story.text, with payloads in marker order.
- Session gained cut_text_document: Option<u64> for clipboard move tracking. This is internal runtime state, not a saved Document field or an external integration parameter.

An anchor is a destination point in story text, not an anchored page object and not a stored text range. A bookmark names that destination through its ID. Creating an anchor alone does not create a bookmark.

## Recommended integration: create the anchor, then reference its ID

- Select the destination through text.select. Supply the story ID and anchor/focus endpoints as numeric UTF-8 byte offsets into the current story text. Equal endpoints select a caret; a range places the new anchor at the start of that range. Table-cell and footnote destinations are not supported by this workflow.
- The anchor parameter of text.select is a selection endpoint, not a TextAnchor ID. Do not confuse it with the anchor parameter of bookmark.add.
- Execute anchor.create with params {"name":"Introduction"}. It inserts the marker and returns an object whose id is the new anchor ID. The name must be nonempty and must not already belong to another anchor in the document.
- Execute bookmark.add with params containing anchor set to that returned numeric ID and, optionally, name set to the desired bookmark label. Omit page entirely. The command returns an object with the new bookmark's index.
- Keep the returned anchor ID as the destination identity. Do not derive it from the current page, byte position, or name. Preserve its integer precision as a u64.
- If name is omitted from bookmark.add when an existing anchor is supplied, the bookmark initially uses that anchor's name. Bookmark and anchor names are separate afterward; renaming the anchor does not rename the bookmark.

For an existing anchor, use anchor.list to retrieve its id instead of creating another anchor. bookmark.add also accepts an exact anchor name, so {"anchor":"Introduction","name":"Introduction"} is valid. Prefer the numeric ID: pasted or imported anchors can have duplicate names, and an ambiguous name is rejected. An unknown ID or name is also rejected.

Through the control channel, wrap commands in engine.execute. For example, after an anchor named Introduction exists:

{"method":"engine.execute","params":{"command":"bookmark.add","params":{"anchor":"Introduction","name":"Introduction"}}}

Check the response's ok field and use result only on success. The transport is documented in docs/control-protocol.md. An integration using Session directly executes the same command ID with the same params object.

## Shortcut: create a bookmark from selected text

- With a story-text selection or caret active, execute bookmark.add with params {"name":"Introduction"}, without anchor and without page.
- DesignCraft reuses an anchor exactly at the selection start or inserts one there, then creates a bookmark referencing its ID. Query bookmark.list afterward to obtain that ID; bookmark.add itself returns the bookmark index.
- Without an explicit anchor or page, a valid story-text selection is required. The command no longer defaults to a page bookmark when nothing is selected.
- Do not keep sending the old page parameter. Supplying page without an anchor explicitly requests a fixed page bookmark, even when text is selected. For text destinations, send anchor or use the selection shortcut and omit page.
- bookmark.add appends a bookmark on each successful call. It is not an idempotent upsert. Before retrying after an uncertain response, query bookmark.list and check whether the intended anchor is already referenced.

The Text Anchors panel's Bookmark checkbox uses bookmark.add with the existing anchor's ID. The Bookmarks panel's creation button uses the selected-text shortcut.

Clicking a text anchor or its bookmark selects the anchor's current text position and centers that position in the canvas while preserving zoom. Legacy page bookmarks still navigate to their page. Long names are truncated at the end to keep Delete visible.

In an exported PDF, a hyperlink to a text anchor and a bookmark anchored in text jump to the top of the anchor's composed line, at the anchor's x position, rather than to the top of the page. Page hyperlinks and page bookmarks keep their page destinations. A link or bookmark whose anchor is unresolved or on a page that was not exported is left out.

PDF text anchor bookmarks sort within each sibling list by absolute page, then increasing composed caret x, then increasing composed caret y, matching the Text Anchors panel. Anchor IDs, object IDs, names and creation order do not precede these coordinates. Bookmark hierarchy and legacy page bookmark positions stay intact; export does not reorder the saved document's bookmarks.

## Reading destinations correctly

- bookmark.list returns name, anchor, page, generatedByToc, and children. Its page is the current absolute page index, starting at zero, or null when an anchor cannot be resolved. Its anchor is the numeric target ID or null for a page bookmark. These destination rules also apply to returned children.
- anchor.list returns id, name, story, pos, and page. Its pos is the marker's current UTF-8 byte offset. Its page is a page label string, or null, not the numeric page index returned by bookmark.list.
- anchor.list orders resolved anchors by absolute page index, then increasing composed x coordinate, then increasing composed y coordinate. Unresolved anchors follow resolved destinations; story, marker position and anchor identity break ties deterministically.
- A page label can include section numbering. Do not convert anchor.list.page into an absolute page index or assume it is a decimal number.
- Treat both pos and the resolved page as current observations. Re-query after edits; do not persist either as the identity of the destination.
- Internally, Cache.xref_index(document).place(anchor_id) resolves the composed destination page, caret x and line top for bookmark queries and PDF outlines. Panel navigation resolves the current caret geometry from composition. Both ignore Bookmark.page for anchored bookmarks.
- If the marker is deleted or its location is overset, the bookmark can become unresolved. Display that state and disable navigation; do not redirect to its stored page. PDF export skips an unresolved destination, and also skips a destination whose page was not exported.

## Saved document JSON and direct Rust consumers

Prefer engine commands for creation and editing. They insert markers, maintain text ranges and selection, and participate in undo. Writing an anchor payload without its corresponding story marker does not create a valid text destination.

- In a saved bookmark object, anchor is the numeric destination ID. page remains a required numeric field in the current Rust schema even though anchored bookmarks ignore it. The command currently stores zero there when page is omitted; that is not the resolved destination page.
- Direct Rust code constructing Bookmark literals must now provide anchor: Some(destination_id) for text bookmarks or anchor: None for page bookmarks, along with the existing fields. Manual bookmarks use generated_by_toc: false.
- A direct document writer must preserve the story marker, its payload ordering, globally unique anchor IDs, valid UTF-8 offsets, formatting runs, and affected text ranges. Do not strip private-use marker characters when normalizing story text.
- Do not serialize bookmark.list results directly over saved bookmark objects. The query returns resolved pages and can return page: null, whereas the persisted model requires a numeric page.
- Readers and writers that model their own document JSON must preserve the new anchor field, including inside children. Dropping it changes a text bookmark into a page bookmark. An older consumer may silently ignore the field and use the stored page instead.
- Existing native page bookmarks still load with anchor absent. They are not automatically converted: the old data does not identify the intended text location. Create or choose an anchor and deliberately replace the old bookmark. There is no bookmark.edit or retarget command in this implementation; bookmark.rename changes only its label.
- Bookmark indices are positions in the current top-level list, not stable IDs. Deleting an earlier bookmark shifts later indices. Re-query before issuing later bookmark.rename or bookmark.delete operations.

## Moving, copying, deleting and interchange

- Reflow follows the anchor marker through the story's threaded frames. There is no need to rewrite a bookmark's page after pagination changes.
- For a cut/paste move, include the anchor marker with the text and use DesignCraft's formatted edit.cut/edit.paste workflow within the same document. The engine preserves an absent original anchor ID for that move. A plain-text transfer, external delete/reinsert, or a move to another document does not provide that identity guarantee.
- Copy/paste allocates fresh anchor IDs for the copy. Existing bookmarks continue targeting the original anchor; they are not duplicated or redirected automatically. The receiving app must intentionally create a bookmark for a copied destination if desired.
- Anchor creation inserts a marker, so byte offsets at and after the insertion shift. Re-query positions rather than reusing an offset map calculated before creating anchors.
- anchor.delete accepts id and optional removeBookmarks. By default it refuses anchors referenced by bookmarks, hyperlinks or cross-references. With removeBookmarks set to true, it deletes the anchor and every linked bookmark atomically, promoting surviving bookmark children into the removed bookmark's parent. Hyperlinks and cross-references still block deletion without changing the document. The Text Anchors panel uses this option; one undo restores the anchor and bookmarks. Deleting story text containing the marker can still leave a bookmark unresolved.
- Native save/reopen preserves the anchor relationship. Bookmarks are not currently imported or exported through IDML. Do not use an IDML round trip to preserve the bookmark list.
- TOC-generated bookmarks still use page destinations with anchor absent. To create text-based bookmarks from another app, use the explicit anchor workflow rather than assuming TOC generation produces them.

## Integration checks

- Create an anchor and bookmark, then insert enough text before it to move it to another page. Confirm bookmark.list reports the new page and the exported PDF outline targets that page, with its destination at the anchor's line rather than the page origin.
- Save and reopen the native document, then confirm the same anchor ID is still referenced.
- Cut and paste the text with its marker within the document; confirm the bookmark follows. Copy it instead; confirm the bookmark remains on the original destination.
- Delete the marker or make its destination overset; confirm page becomes null rather than falling back to the stored page.
- Load an old page bookmark and confirm the integration preserves its page semantics until it is deliberately migrated.

Implementation references: crates/doc/src/lib.rs (Bookmark and Document.bookmarks), crates/doc/src/story.rs (Story.anchors), crates/doc/src/xref.rs (TextAnchor and marker positions), crates/engine/src/cmd/interactive.rs (bookmark commands), crates/engine/src/cmd/xref.rs (anchor commands), crates/engine/src/cmd/edit.rs (clipboard identity), crates/compose/src/xref.rs (anchor placement), and crates/pdf/src/links.rs (PDF destinations and outlines).
