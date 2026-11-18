//! Type › Hyperlinks & Cross-References: text anchors, cross-references and their formats.

use designcraft_doc::{CrossRef, StoryId, TextSel, XrefFormat};
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, has_doc, str_param};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "xref.insert",
            "Insert Cross-Reference…",
            ["Type", "Hyperlinks & Cross-References"],
            None,
            "{anchor? | paragraph?: text to find (first paragraph containing it) | story?: with para? | para?: 0-based paragraph of `story`, format?: name (default Full Paragraph & Page Number)} — at the insertion point; the text updates itself as the destination moves",
            in_story_text,
            insert
        ),
        cmd!(
            "anchor.create",
            "New Hyperlink Destination…",
            ["Type", "Hyperlinks & Cross-References"],
            None,
            "{name} — a text anchor at the insertion point; the name is the document's, so it must not be one another anchor already has",
            in_story_text,
            create_anchor
        ),
        cmd!(query "xref.list", "Cross-References", [], None, "{} → [{story, pos, target, format, text}]", has_doc, list),
        cmd!(query "anchor.list", "Text Anchors", [], None, "{} → [{id, name, story, pos, page}]", has_doc, list_anchors),
        cmd!("anchor.rename", "Rename Text Anchor", [], None, "{id, name}", has_doc, rename_anchor),
        cmd!(
            "anchor.delete",
            "Delete Text Anchor",
            [],
            None,
            "{id, removeBookmarks?: bool} — optionally removes linked bookmarks; refuses anchors used by hyperlinks or cross-references",
            has_doc,
            delete_anchor
        ),
        cmd!(query "xref.formats", "Cross-Reference Formats", [], None, "{} → [{name, definition}]", has_doc, |s, _| {
            Ok(serde_json::to_value(&s.doc()?.doc.xref_formats).unwrap_or_default())
        }),
        cmd!(
            "xref.defineFormat",
            "Define Cross-Reference Format…",
            ["Type", "Hyperlinks & Cross-References"],
            None,
            "{name, definition} — building blocks: <fullPara />, <paraText />, <paraNum />, <pageNum />, <txtAnchrName />, <chapNum />, <fileName />, <partialPara delim=\":\" includeDelim=\"false\" />; creates or replaces",
            has_doc,
            define_format
        ),
        cmd!("xref.setFormat", "Set Cross-Reference Format", [], None, "{story, index, format}", has_doc, set_format),
    ]
}

fn in_story_text(s: &Session) -> std::result::Result<(), String> {
    super::has_text(s)?;
    match s.active().and_then(|d| d.selection.text) {
        Some(t) if t.cell.is_some() => Err("cross-references go in story text (not table cells or footnotes yet)".into()),
        _ => Ok(()),
    }
}

/// The destination anchor from params, creating a paragraph anchor when needed.
fn destination(d: &mut designcraft_doc::Document, p: &Value) -> Result<u64> {
    if let Some(a) = p.get("anchor").and_then(Value::as_u64) {
        return d.find_anchor(a).map(|_| a).ok_or_else(|| bad("xref.insert", format!("no anchor {a}")));
    }
    let (sid, pos) = if let Some(text) = str_param(p, "paragraph") {
        let hit = d.stories.values().find_map(|st| st.text.find(text).map(|i| (st.id, i)));
        hit.ok_or_else(|| bad("xref.insert", format!("no paragraph contains `{text}`")))?
    } else if let (Some(sid), Some(pi)) = (p.get("story").and_then(Value::as_u64), p.get("para").and_then(Value::as_u64)) {
        let st = d.story(StoryId(sid)).ok_or_else(|| bad("xref.insert", "no such story"))?;
        let r = st.para_ranges().get(pi as usize).cloned().ok_or_else(|| bad("xref.insert", "no such paragraph"))?;
        (StoryId(sid), r.start)
    } else {
        return Err(bad("xref.insert", "give `anchor`, `paragraph` or `story` + `para`"));
    };
    let before = d.story(sid).map_or(0, |st| st.len());
    let id = d.paragraph_anchor(sid, pos).ok_or_else(|| bad("xref.insert", "could not anchor the paragraph"))?;
    let grew = d.story(sid).map_or(0, |st| st.len()).saturating_sub(before);
    if grew > 0
        && let Some((asid, apos)) = d.find_anchor(id)
    {
        d.adjust_text_ranges(asid, apos..apos, grew);
    }
    Ok(id)
}

fn insert(s: &mut Session, p: &Value) -> Result<Value> {
    let p = p.clone();
    s.edit(|d, sel| {
        let format = str_param(&p, "format").unwrap_or("Full Paragraph & Page Number").to_string();
        if d.xref_format(&format).is_none() {
            return Err(bad("xref.insert", format!("no cross-reference format `{format}`")));
        }
        let t = sel.text.ok_or_else(|| bad("xref.insert", "no insertion point"))?;
        // A new paragraph anchor before the caret in the same story shifts the caret.
        let len0 = d.story(t.story).map_or(0, |st| st.len());
        let target = destination(d, &p)?;
        let mut t2 = t;
        if let Some((asid, apos)) = d.find_anchor(target)
            && asid == t.story
            && apos <= t.range().start
        {
            let grew = d.story(t.story).map_or(0, |st| st.len()) - len0;
            t2.anchor += grew;
            t2.focus += grew;
        }
        let st = d.story_mut(t.story).ok_or(designcraft_doc::DocError::NoStory(t.story))?;
        let r = t2.range();
        let r = r.start.min(st.len())..r.end.min(st.len());
        st.delete(r.clone());
        st.insert_xref(r.start, CrossRef { target, format, ..Default::default() });
        if t.cell.is_none() {
            d.adjust_text_ranges(t.story, r.clone(), designcraft_doc::XREF_MARK.len_utf8());
        }
        let pos = r.start + designcraft_doc::XREF_MARK.len_utf8();
        sel.text = Some(TextSel { anchor: pos, focus: pos, ..t2 });
        Ok(json!({"target": target, "pos": r.start}))
    })
}

fn create_anchor(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("anchor.create", "missing name"))?.to_string();
    if name.trim().is_empty() {
        return Err(bad("anchor.create", "a text anchor needs a name (a cross-reference and a hyperlink find it by that name)"));
    }
    // Two anchors of the same name made every reference to it ambiguous: `xref.insert` and
    // `hyperlink.create` take the first they find, and a file writes both under one name.
    if s.doc()?.doc.stories.values().any(|st| st.anchors.iter().any(|a| a.name == name)) {
        return Err(bad("anchor.create", format!("a text anchor named `{name}` is already in this document (anchor.list names them)")));
    }
    s.edit(|d, sel| {
        let t = sel.text.ok_or_else(|| bad("anchor.create", "no insertion point"))?;
        let st = d.story_mut(t.story).ok_or(designcraft_doc::DocError::NoStory(t.story))?;
        let at = t.range().start.min(st.len());
        let id = st.insert_anchor(at, &name);
        if t.cell.is_none() {
            d.adjust_text_ranges(t.story, at..at, designcraft_doc::ANCHOR_MARK.len_utf8());
        }
        let shift = designcraft_doc::ANCHOR_MARK.len_utf8();
        sel.text = Some(TextSel { anchor: t.anchor.saturating_add(shift), focus: t.focus.saturating_add(shift), ..t });
        Ok(json!({"id": id}))
    })
}

fn rename_anchor(s: &mut Session, p: &Value) -> Result<Value> {
    let id = p.get("id").and_then(Value::as_u64).ok_or_else(|| bad("anchor.rename", "missing id"))?;
    let name = str_param(p, "name").filter(|n| !n.trim().is_empty()).ok_or_else(|| bad("anchor.rename", "missing name"))?;
    s.edit(|d, _| {
        if d.stories.values().any(|st| st.anchors.iter().any(|a| a.id != id && a.name == name)) {
            return Err(bad("anchor.rename", "a text anchor already has that name"));
        }
        let (sid, _) = d.find_anchor(id).ok_or_else(|| bad("anchor.rename", "no such text anchor"))?;
        let st = d.story_mut(sid).ok_or(designcraft_doc::DocError::NoStory(sid))?;
        let a = st.anchors.iter_mut().find(|a| a.id == id).ok_or_else(|| bad("anchor.rename", "no such text anchor"))?;
        std::sync::Arc::make_mut(a).name = name.to_string();
        st.rev = st.rev.wrapping_add(1);
        Ok(Value::Null)
    })
}

fn delete_anchor(s: &mut Session, p: &Value) -> Result<Value> {
    let id = p.get("id").and_then(Value::as_u64).ok_or_else(|| bad("anchor.delete", "missing id"))?;
    let remove_bookmarks = p.get("removeBookmarks").and_then(Value::as_bool).unwrap_or(false);
    s.edit(|d, sel| {
        if d.hyperlinks.iter().any(|h| h.dest == designcraft_doc::HyperlinkDest::Anchor(id))
            || d.stories.values().any(|st| st.xrefs.iter().any(|x| x.target == id))
            || (!remove_bookmarks && bookmark_uses(&d.bookmarks, id))
        {
            return Err(bad("anchor.delete", "remove bookmarks, hyperlinks and cross-references to this text anchor first"));
        }
        let (sid, pos) = d.find_anchor(id).ok_or_else(|| bad("anchor.delete", "no such text anchor"))?;
        let end = pos.saturating_add(designcraft_doc::ANCHOR_MARK.len_utf8());
        d.story_mut(sid).ok_or(designcraft_doc::DocError::NoStory(sid))?.delete(pos..end);
        d.adjust_text_ranges(sid, pos..end, 0);
        if remove_bookmarks {
            let mut pending = vec![&mut d.bookmarks];
            while let Some(bookmarks) = pending.pop() {
                let mut i = 0;
                while i < bookmarks.len() {
                    if bookmarks.get(i).is_some_and(|b| b.anchor == Some(id)) {
                        let removed = bookmarks.remove(i);
                        bookmarks.splice(i..i, removed.children);
                    } else {
                        i += 1;
                    }
                }
                pending.extend(bookmarks.iter_mut().map(|b| &mut b.children));
            }
        }
        if let Some(t) = sel.text.as_mut().filter(|t| t.story == sid && t.cell.is_none()) {
            let repair = |at: usize| if at > pos { at.saturating_sub(end - pos).max(pos) } else { at };
            t.anchor = repair(t.anchor);
            t.focus = repair(t.focus);
        }
        Ok(Value::Null)
    })
}

fn bookmark_uses(bookmarks: &[designcraft_doc::Bookmark], id: u64) -> bool {
    let mut pending: Vec<_> = bookmarks.iter().collect();
    while let Some(b) = pending.pop() {
        if b.anchor == Some(id) {
            return true;
        }
        pending.extend(&b.children);
    }
    false
}

fn list(s: &mut Session, _: &Value) -> Result<Value> {
    let d = s.doc()?.doc.clone();
    let index = s.cache.xref_index(&d);
    let mut out = Vec::new();
    for st in d.stories.values() {
        for ((pos, _), x) in st.text.match_indices(designcraft_doc::XREF_MARK).zip(&st.xrefs) {
            let text = designcraft_compose::xref::xref_text(&d, x, Some(&index));
            out.push(json!({"story": st.id.0, "pos": pos, "target": x.target, "format": x.format, "text": text}));
        }
    }
    Ok(Value::Array(out))
}

fn list_anchors(s: &mut Session, _: &Value) -> Result<Value> {
    let d = s.doc()?.doc.clone();
    let mut out = Vec::new();
    for st in d.stories.values().filter(|st| !st.anchors.is_empty()) {
        let cs = s.cache.get(&d, st.id, None);
        for a in &st.anchors {
            let pos = st.anchor_pos(a.id);
            let place = pos.and_then(|pos| {
                let (fi, x, baseline, ..) = designcraft_compose::caret(&cs, pos)?;
                let ft = cs.frames.get(fi).filter(|ft| ft.range.contains(&pos))?;
                let it = d.item(ft.frame)?;
                let loc = d.find(ft.frame)?;
                let designcraft_doc::SpreadRef::Doc(si) = loc.spread else { return None };
                let point = designcraft_geom::Point::new(x, baseline);
                let point = designcraft_compose::PathTextMap::new(it, ft).map_or(point, |map| map.to_path(point));
                let point = d.parent_xf(&loc) * d.text_xf(it) * point;
                let pi = d.spreads.get(si)?.page_at_x(point.x)?;
                let page = d.first_page_of_spread(si).checked_add(pi)?;
                Some((page, point))
            });
            let (page, x, y) = place.map_or((usize::MAX, f64::INFINITY, f64::INFINITY), |(page, point)| (page, point.x, point.y));
            let value = json!({"id": a.id, "name": a.name, "story": st.id.0, "pos": pos, "page": place.map(|(page, _)| d.page_name(page))});
            out.push(((page, x, y, st.id, pos, a.id), value));
        }
    }
    out.sort_by(|(a, _), (b, _)| {
        a.0.cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)).then_with(|| a.2.total_cmp(&b.2)).then_with(|| (a.3, a.4, a.5).cmp(&(b.3, b.4, b.5)))
    });
    Ok(Value::Array(out.into_iter().map(|(_, value)| value).collect()))
}

fn define_format(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("xref.defineFormat", "missing name"))?.to_string();
    let definition = str_param(p, "definition").ok_or_else(|| bad("xref.defineFormat", "missing definition"))?.to_string();
    s.edit(|d, _| {
        match d.xref_formats.iter_mut().find(|f| f.name == name) {
            Some(f) => f.definition = definition.clone(),
            None => d.xref_formats.push(XrefFormat { name: name.clone(), definition: definition.clone() }),
        }
        Ok(Value::Null)
    })
}

fn set_format(s: &mut Session, p: &Value) -> Result<Value> {
    let sid = p.get("story").and_then(Value::as_u64).map(StoryId).ok_or_else(|| bad("xref.setFormat", "missing story"))?;
    let k = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad("xref.setFormat", "missing index"))? as usize;
    let format = str_param(p, "format").ok_or_else(|| bad("xref.setFormat", "missing format"))?.to_string();
    s.edit(|d, _| {
        if d.xref_format(&format).is_none() {
            return Err(bad("xref.setFormat", format!("no cross-reference format `{format}`")));
        }
        let st = d.story_mut(sid).ok_or(designcraft_doc::DocError::NoStory(sid))?;
        let x = st.xrefs.get_mut(k).ok_or_else(|| bad("xref.setFormat", "no such cross-reference"))?;
        let x = std::sync::Arc::make_mut(x);
        x.format = format;
        // A new format generates the text again, replacing any edits (InDesign does the same).
        x.text = None;
        st.rev = st.rev.wrapping_add(1);
        Ok(Value::Null)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_are_ordered_by_page_then_x_then_y() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 2, "facingPages": false})).unwrap();
        let text = "Line\n".repeat(50);
        let f = s.execute("frame.create", &json!({"rect": [36,36,236,66], "content": "text", "text": text})).unwrap();
        s.execute("text.select", &json!({"story": f["story"], "anchor": text.len()})).unwrap();
        s.execute("anchor.create", &json!({"name": "Overset"})).unwrap();
        for (page, x, y, name) in [(1, 36, 36, "Next page"), (0, 300, 36, "Right"), (0, 36, 400, "Left lower"), (0, 36, 36, "Left upper")] {
            let spread = s.doc().unwrap().doc.page_loc(page).unwrap().0;
            let f = s.execute("frame.create", &json!({"spread": spread, "rect": [x,y,x+200,y+100], "content": "text", "text": "Heading"})).unwrap();
            s.execute("text.select", &json!({"story": f["story"], "anchor": 0, "focus": 0})).unwrap();
            s.execute("anchor.create", &json!({"name": name})).unwrap();
        }
        let list = s.execute("anchor.list", &json!({})).unwrap();
        let names: Vec<_> = list.as_array().unwrap().iter().map(|a| a["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["Left upper", "Left lower", "Right", "Next page", "Overset"]);
        assert!(list[4]["page"].is_null());
    }

    #[test]
    fn deleting_an_anchor_can_remove_its_bookmarks_atomically_and_preserve_children() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let f = s.execute("frame.create", &json!({"rect": [36,36,300,200], "content": "text", "text": "Heading"})).unwrap();
        s.execute("text.select", &json!({"story": f["story"], "anchor": 0})).unwrap();
        let id = s.execute("anchor.create", &json!({"name": "Heading"})).unwrap()["id"].clone();
        s.execute("bookmark.add", &json!({"anchor": id})).unwrap();
        s.edit(|d, _| {
            let target = d.bookmarks[0].clone();
            let child = designcraft_doc::Bookmark { name: "Child".into(), anchor: None, page: 0, children: vec![], generated_by_toc: false };
            d.bookmarks[0].children.push(child);
            d.bookmarks.push(designcraft_doc::Bookmark {
                name: "Parent".into(),
                anchor: None,
                page: 0,
                children: vec![target],
                generated_by_toc: false,
            });
            Ok(Value::Null)
        })
        .unwrap();
        let before = s.doc().unwrap().doc.bookmarks.clone();
        assert!(s.execute("anchor.delete", &json!({"id": id})).is_err());
        s.execute("hyperlink.create", &json!({"anchor": id, "ids": [f["id"]]})).unwrap();
        assert!(s.execute("anchor.delete", &json!({"id": id, "removeBookmarks": true})).is_err());
        assert_eq!(serde_json::to_value(&s.doc().unwrap().doc.bookmarks).unwrap(), serde_json::to_value(&before).unwrap());
        s.execute("edit.undo", &json!({})).unwrap();
        s.execute("anchor.delete", &json!({"id": id, "removeBookmarks": true})).unwrap();
        let d = &s.doc().unwrap().doc;
        assert_eq!(d.story(StoryId(f["story"].as_u64().unwrap())).unwrap().text, "Heading");
        assert_eq!(d.bookmarks[0].name, "Child");
        assert_eq!(d.bookmarks[1].name, "Parent");
        assert!(d.bookmarks[1].children.is_empty());
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(serde_json::to_value(&s.doc().unwrap().doc.bookmarks).unwrap(), serde_json::to_value(&before).unwrap());
        assert_eq!(s.execute("anchor.list", &json!({})).unwrap()[0]["id"], id);
    }

    #[test]
    fn manage_text_anchors_preserves_selected_text_and_references() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let f = s.execute("frame.create", &json!({"rect": [36,36,300,200], "content": "text", "text": "Résumé"})).unwrap();
        let sid = StoryId(f["story"].as_u64().unwrap());
        s.execute("text.select", &json!({"story": sid.0, "anchor": "Résumé".len(), "focus": 0})).unwrap();
        let id = s.execute("anchor.create", &json!({"name": "Heading"})).unwrap()["id"].clone();
        let st = s.doc().unwrap().doc.story(sid).unwrap();
        let t = s.doc().unwrap().selection.text.unwrap();
        assert_eq!(&st.text[t.range()], "Résumé");
        assert!(st.objects.is_empty());
        s.execute("anchor.rename", &json!({"id": id, "name": "Introduction"})).unwrap();
        assert_eq!(s.execute("anchor.list", &json!({})).unwrap()[0]["name"], "Introduction");
        s.execute("hyperlink.create", &json!({"anchor": id})).unwrap();
        assert!(s.execute("anchor.delete", &json!({"id": id})).is_err());
        s.execute("edit.undo", &json!({})).unwrap();
        s.execute("anchor.delete", &json!({"id": id})).unwrap();
        assert_eq!(s.doc().unwrap().doc.story(sid).unwrap().text, "Résumé");
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.execute("anchor.list", &json!({})).unwrap()[0]["id"], id);
        assert!(s.execute("anchor.rename", &json!({"id": id, "name": " "})).is_err());
    }

    #[test]
    fn cross_reference_follows_its_destination() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 3, "facingPages": false})).unwrap();
        // A heading on page 3 and a referring frame on page 1.
        let sp = s.doc().unwrap().doc.page_loc(2).unwrap().0;
        let r = s
            .execute("frame.create", &json!({"spread": sp, "rect": [72, 72, 400, 200], "content": "text", "text": "Results: what we found"}))
            .unwrap();
        let head = s.doc().unwrap().doc.item(designcraft_doc::ItemId(r["id"].as_u64().unwrap())).unwrap().text_frame().unwrap().story;
        let r = s.execute("frame.create", &json!({"rect": [72, 72, 400, 200], "content": "text", "text": "See ."})).unwrap();
        let src = s.doc().unwrap().doc.item(designcraft_doc::ItemId(r["id"].as_u64().unwrap())).unwrap().text_frame().unwrap().story;
        s.execute("text.select", &json!({"story": src.0, "anchor": 4, "focus": 4})).unwrap();
        s.execute("xref.insert", &json!({"paragraph": "Results"})).unwrap();
        let l = s.execute("xref.list", &json!({})).unwrap();
        assert_eq!(l[0]["text"], "\u{201C}Results: what we found\u{201D} on page 3");
        // Edit the heading: the reference follows without an update step.
        // The paragraph now starts with its (zero-width) anchor mark.
        let at = designcraft_doc::ANCHOR_MARK.len_utf8();
        s.execute("text.select", &json!({"story": head.0, "anchor": at, "focus": at})).unwrap();
        s.execute("text.insert", &json!({"text": "Key "})).unwrap();
        let l = s.execute("xref.list", &json!({})).unwrap();
        assert_eq!(l[0]["text"], "\u{201C}Key Results: what we found\u{201D} on page 3");
        // Formats.
        s.execute("xref.setFormat", &json!({"story": src.0, "index": 0, "format": "Partial Paragraph"})).unwrap();
        assert_eq!(s.execute("xref.list", &json!({})).unwrap()[0]["text"], "\u{201C}Key Results\u{201D}");
        s.execute("xref.defineFormat", &json!({"name": "Short", "definition": "(p. <pageNum />)"})).unwrap();
        s.execute("xref.setFormat", &json!({"story": src.0, "index": 0, "format": "Short"})).unwrap();
        assert_eq!(s.execute("xref.list", &json!({})).unwrap()[0]["text"], "(p. 3)");
        // The generated text is composed into the source frame.
        let d = s.doc().unwrap().doc.clone();
        let cs = s.cache.get(&d, src, None);
        let glyphs: usize = cs.frames[0].lines.iter().map(|l| l.glyphs.iter().filter(|g| g.visible && g.adv > 0.0).count()).sum();
        assert!(glyphs >= "See (p. 3).".replace(' ', "").len(), "{glyphs}");
        // Deleting the destination paragraph's anchor leaves an unresolved reference.
        let a = s.execute("anchor.list", &json!({})).unwrap();
        assert_eq!(a[0]["page"], "3");
        s.execute("text.select", &json!({"story": head.0, "anchor": 0, "focus": 3})).unwrap();
        s.execute("text.delete", &json!({})).unwrap();
        assert_eq!(s.execute("xref.list", &json!({})).unwrap()[0]["text"], "??");
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.execute("xref.list", &json!({})).unwrap()[0]["text"], "(p. 3)");
    }

    /// Two text anchors of the same name made every reference to it ambiguous: `xref.insert
    /// {"anchor": "A"}` and `hyperlink.create {"anchor": "A"}` take the first they find, and an
    /// exported file writes both destinations under one name.
    #[test]
    fn a_text_anchor_name_is_the_documents_so_it_is_not_reused() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 2, "facingPages": false})).unwrap();
        let a = s.execute("frame.create", &json!({"rect": [72, 72, 400, 200], "content": "text", "text": "Photo one here"})).unwrap();
        let select = |s: &mut Session, story: serde_json::Value, from: usize| {
            s.execute("text.select", &json!({"story": story, "anchor": from, "focus": from})).unwrap();
        };
        select(&mut s, a["story"].clone(), 0);
        s.execute("anchor.create", &json!({"name": "Photo 1"})).unwrap();

        // The same name again, in the same story and in another one.
        select(&mut s, a["story"].clone(), 10);
        let e = s.execute("anchor.create", &json!({"name": "Photo 1"})).expect_err("refused").to_string();
        assert!(e.contains("already in this document"), "{e}");
        let sp = s.doc().unwrap().doc.page_loc(1).unwrap().0;
        let b = s.execute("frame.create", &json!({"spread": sp, "rect": [72, 72, 400, 200], "content": "text", "text": "Photo two"})).unwrap();
        select(&mut s, b["story"].clone(), 0);
        assert!(s.execute("anchor.create", &json!({"name": "Photo 1"})).is_err(), "not in another story either");
        // A name of spaces is not a name.
        assert!(s.execute("anchor.create", &json!({"name": "   "})).is_err());
        assert!(s.execute("anchor.create", &json!({"name": ""})).is_err());

        // One anchor, so a reference by name can only mean that one.
        let anchors = s.execute("anchor.list", &json!({})).unwrap();
        assert_eq!(anchors.as_array().unwrap().len(), 1, "{anchors}");
        // Another name is fine.
        s.execute("anchor.create", &json!({"name": "Photo 2"})).unwrap();
        assert_eq!(s.execute("anchor.list", &json!({})).unwrap().as_array().unwrap().len(), 2);
        // The check is on the name, not on where it is: a name that differs only in case is a
        // different name, as it is everywhere else in the document.
        select(&mut s, b["story"].clone(), 0);
        s.execute("anchor.create", &json!({"name": "photo 1"})).expect("a different name");
    }

    #[test]
    fn cross_reference_into_a_story_with_a_running_header_composes() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 1, "facingPages": false})).unwrap();
        let head = s.execute("frame.create", &json!({"rect": [72, 72, 400, 120], "content": "text", "text": "Chapter"})).unwrap()["story"].clone();
        let body = s.execute("frame.create", &json!({"rect": [72, 200, 400, 300], "content": "text", "text": "See"})).unwrap()["story"].clone();
        s.execute("text.select", &json!({"story": head, "anchor": 0, "focus": 0})).unwrap();
        s.execute("variables.insert", &json!({"name": "Running Header"})).unwrap();
        s.execute("text.select", &json!({"story": body, "anchor": 3, "focus": 3})).unwrap();
        s.execute("xref.insert", &json!({"story": head, "para": 0})).unwrap();
        // The cross-reference index and the running-header index used to build each other without end.
        assert_eq!(s.execute("xref.list", &json!({})).unwrap()[0]["text"], "\u{201C}Chapter\u{201D} on page 1");
        // The header shows the body's paragraph in front of its own text.
        let d = s.doc().unwrap().doc.clone();
        let cs = s.cache.get(&d, StoryId(head.as_u64().unwrap()), None);
        let glyphs: usize = cs.frames[0].lines.iter().map(|l| l.glyphs.iter().filter(|g| g.visible && g.adv > 0.0).count()).sum();
        assert_eq!(glyphs, "SeeChapter".len());
    }

    /// Every `/Rect` in an exported PDF, as `[x0, y0, x1, y1]`: the link annotations of a
    /// document with nothing else annotated.
    fn link_rects(s: &mut Session) -> Vec<[f64; 4]> {
        let r = s.execute("file.exportPdf", &json!({})).unwrap();
        let bytes = super::super::file::base64_decode(r["base64"].as_str().unwrap());
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let mut out = Vec::new();
        for part in text.split("/Rect[").skip(1) {
            let Some(end) = part.find(']') else { continue };
            let v: Vec<f64> = part[..end].split_whitespace().filter_map(|n| n.parse().ok()).collect();
            if let [x0, y0, x1, y1] = v[..] {
                out.push([x0, y0, x1, y1]);
            }
        }
        out
    }

    /// A cross-reference's link covered its first letter only: the source range is the one marker
    /// character, and the glyphs of the text it generates all point at that character while only
    /// the first of them owns it, so measuring the glyphs that own their bytes measured one glyph
    /// — a ~7 pt link under ~40 pt of text (#1446).
    #[test]
    fn a_cross_references_link_covers_the_text_it_generated() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 2, "facingPages": false})).unwrap();
        let sp = s.doc().unwrap().doc.page_loc(1).unwrap().0;
        s.execute("frame.create", &json!({"spread": sp, "rect": [72, 72, 400, 200], "content": "text", "text": "Photo 1"})).unwrap();
        let r = s.execute("frame.create", &json!({"rect": [72, 72, 400, 200], "content": "text", "text": "See  for details."})).unwrap();
        let src = r["story"].clone();
        s.execute("text.select", &json!({"story": src, "anchor": 4, "focus": 4})).unwrap();
        s.execute("xref.defineFormat", &json!({"name": "Bare", "definition": "<paraText />"})).unwrap();
        s.execute("xref.insert", &json!({"paragraph": "Photo 1", "format": "Bare"})).unwrap();
        let generated = s.execute("xref.list", &json!({})).unwrap()[0]["text"].as_str().unwrap().to_string();
        assert_eq!(generated, "Photo 1");
        let xref_rects = link_rects(&mut s);
        assert_eq!(xref_rects.len(), 1, "one link, for the cross-reference: {xref_rects:?}");
        let xref_width = xref_rects[0][2] - xref_rects[0][0];

        // The same text, written out and linked as text: the same letters in the same font at the
        // same size, so the two links cover the same width.
        let mut t = Session::new();
        t.execute("file.new", &json!({"pages": 2, "facingPages": false})).unwrap();
        let r = t
            .execute("frame.create", &json!({"rect": [72, 72, 400, 200], "content": "text", "text": format!("See {generated} for details.")}))
            .unwrap();
        let story = r["story"].clone();
        let (start, end) = (4, 4 + generated.len());
        t.execute("text.select", &json!({"story": story, "anchor": start, "focus": end})).unwrap();
        t.execute("hyperlink.create", &json!({"page": 2})).unwrap();
        let plain = link_rects(&mut t);
        assert_eq!(plain.len(), 1, "{plain:?}");
        let plain_width = plain[0][2] - plain[0][0];

        assert!(plain_width > 20.0, "the text is wider than one glyph: {plain_width}");
        assert!((xref_width - plain_width).abs() < 1.0, "the cross-reference covers its text: {xref_width} vs {plain_width}");
        // And it starts where the text starts.
        assert!((xref_rects[0][0] - plain[0][0]).abs() < 1.0, "{:?} vs {:?}", xref_rects[0], plain[0]);
    }

    /// A link starting at the first character of a numbered paragraph took the number in: the
    /// composer gives the list label that character's offset and no length, so the rectangle
    /// reached over text outside the link (noted in review of #1446).
    #[test]
    fn a_links_rectangle_leaves_out_the_number_a_list_adds() {
        // The same linked words twice: once in a plain paragraph, once in a numbered one.
        let width = |numbered: bool| {
            let mut s = Session::new();
            s.execute("file.new", &json!({"pages": 2, "facingPages": false})).unwrap();
            let r = s.execute("frame.create", &json!({"rect": [72, 72, 400, 200], "content": "text", "text": "Linked words here"})).unwrap();
            let story = r["story"].clone();
            if numbered {
                s.execute("text.select", &json!({"story": story, "anchor": 0, "focus": 0})).unwrap();
                s.execute("type.para", &json!({"attrs": {"listType": "numbers"}})).unwrap();
            }
            // The link starts at the paragraph's first character, where the label also sits.
            s.execute("text.select", &json!({"story": story, "anchor": 0, "focus": "Linked words".len()})).unwrap();
            s.execute("hyperlink.create", &json!({"page": 2})).unwrap();
            let rects = link_rects(&mut s);
            assert_eq!(rects.len(), 1, "one link (numbered: {numbered}): {rects:?}");
            (rects[0][2] - rects[0][0], rects[0][0])
        };

        let (plain, plain_x0) = width(false);
        let (listed, listed_x0) = width(true);
        assert!(plain > 20.0, "the linked words are wider than one glyph: {plain}");
        // The same letters at the same size, so the link is the same width either way. The label
        // sits before the text and the paragraph is indented for it, so the rectangle starts
        // further right -- but it covers the words and nothing else.
        assert!((listed - plain).abs() < 1.0, "the number is not inside the link: {listed} vs {plain}");
        assert!(listed_x0 >= plain_x0, "the numbered paragraph's text starts no earlier: {listed_x0} vs {plain_x0}");
    }

    /// A list label's tab can carry a leader: the paragraph defines a stop at the Tab Position
    /// with one, so the composer does not add its own stop there and the label's tab takes that
    /// leader. Those leader glyphs carry the paragraph's first byte, as the label does, but were
    /// not flagged as part of it -- so a link starting at the first text character swept them into
    /// its rectangle and it reached back across the label (#1495 review).
    #[test]
    fn a_links_rectangle_leaves_out_the_leader_of_a_list_labels_tab() {
        let left_edge = |leader: bool| {
            let mut s = Session::new();
            s.execute("file.new", &json!({"pages": 2, "facingPages": false})).unwrap();
            let r = s.execute("frame.create", &json!({"rect": [72, 72, 460, 200], "content": "text", "text": "Chapter one here"})).unwrap();
            let story = r["story"].clone();
            s.execute("text.select", &json!({"story": story.clone(), "anchor": 0, "focus": 1})).unwrap();
            s.execute("type.para", &json!({"listType": "numbers", "listTab": 72})).unwrap();
            if leader {
                s.execute("type.tabs.add", &json!({"position": 72, "leader": "."})).unwrap();
            }
            s.execute("text.select", &json!({"story": story, "anchor": 0, "focus": "Chapter one".len()})).unwrap();
            s.execute("hyperlink.create", &json!({"page": 2})).unwrap();
            let rects = link_rects(&mut s);
            assert_eq!(rects.len(), 1, "one link (leader: {leader}): {rects:?}");
            rects[0][0]
        };

        // The same link, with and without a leader on the label's tab: the leader sits between the
        // label and the text, so taking it in would move the rectangle's left edge leftwards.
        let plain = left_edge(false);
        let dotted = left_edge(true);
        assert!((dotted - plain).abs() < 0.5, "the leader is left out of the link: {dotted} vs {plain}");
    }

    /// The rectangle is the extent of the selected glyphs, not the first and the last of them.
    /// Kenten marks and ruby are appended after a line's base glyphs, so with only the first
    /// character marked the last glyph in the range was that mark, sitting over the first
    /// character -- and the link ended there, leaving the rest of it unclickable (noted in
    /// review of #1446).
    #[test]
    fn a_links_rectangle_spans_its_glyphs_whatever_order_they_are_in() {
        let width = |marked: bool| {
            let mut s = Session::new();
            s.execute("file.new", &json!({"pages": 2, "facingPages": false})).unwrap();
            let r = s.execute("frame.create", &json!({"rect": [72, 72, 400, 200], "content": "text", "text": "Linked words here"})).unwrap();
            let story = r["story"].clone();
            if marked {
                // The first character only, so its mark is the line's last glyph.
                s.execute("text.select", &json!({"story": story, "anchor": 0, "focus": 1})).unwrap();
                s.execute("type.char", &json!({"attrs": {"kenten": true}})).unwrap();
            }
            s.execute("text.select", &json!({"story": story, "anchor": 0, "focus": "Linked words".len()})).unwrap();
            s.execute("hyperlink.create", &json!({"page": 2})).unwrap();
            let rects = link_rects(&mut s);
            assert_eq!(rects.len(), 1, "one link (marked: {marked}): {rects:?}");
            rects[0][2] - rects[0][0]
        };

        let plain = width(false);
        assert!(plain > 20.0, "the linked words are wider than one glyph: {plain}");
        // A mark over the first character does not shorten the link to that character.
        let marked = width(true);
        assert!((marked - plain).abs() < 1.0, "the link still covers its words: {marked} vs {plain}");
    }
}
