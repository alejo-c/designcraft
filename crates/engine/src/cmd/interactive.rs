//! Hyperlinks and bookmarks (Window → Interactive), exported to PDF as link annotations and the outline.

use designcraft_doc::{Bookmark, Hyperlink, HyperlinkDest, HyperlinkSource};
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, has_doc, str_param};
use crate::{Result, Session};

/// Register commands for editing hyperlinks and manual bookmarks.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "hyperlink.create",
            "New Hyperlink…",
            ["Type", "Hyperlinks & Cross-References"],
            None,
            "{url? | email? | page? (1-based) | anchor? (a text anchor's id or name; the link follows it as the text reflows), name?, ids?} — from the selected text, or the selected frames",
            has_doc,
            create
        ),
        cmd!("hyperlink.delete", "Delete Hyperlink", [], None, "{id}", has_doc, |s, p| {
            let id = p.get("id").and_then(Value::as_u64).ok_or_else(|| bad("hyperlink.delete", "missing id"))?;
            s.edit(|d, _| {
                d.hyperlinks.retain(|h| h.id != id);
                Ok(Value::Null)
            })
        }),
        cmd!(query "hyperlink.list", "Hyperlinks", [], None, "{}", has_doc, |s, _| Ok(serde_json::to_value(&s.doc()?.doc.hyperlinks).unwrap_or_default())),
        cmd!("hyperlink.edit", "Hyperlink Options…", [], None, "{id, name?, url? | email? | page? (1-based) | anchor?}", has_doc, |s, p| {
            let id = p.get("id").and_then(Value::as_u64).ok_or_else(|| bad("hyperlink.edit", "missing id"))?;
            let dest = dest_param("hyperlink.edit", &s.doc()?.doc, p)?;
            let name = str_param(p, "name").map(str::to_string);
            s.edit(|d, _| {
                let h = d.hyperlinks.iter_mut().find(|h| h.id == id).ok_or_else(|| bad("hyperlink.edit", format!("no hyperlink {id}")))?;
                if let Some(n) = name {
                    h.name = n;
                }
                if let Some(dst) = dest {
                    h.dest = dst;
                }
                Ok(Value::Null)
            })
        }),
        cmd!(noundo "hyperlink.goToSource", "Go To Source", [], None, "{id} — selects the hyperlink's text or frame", has_doc, |s, p| {
            let id = p.get("id").and_then(Value::as_u64).ok_or_else(|| bad("hyperlink.goToSource", "missing id"))?;
            let st = s.doc_mut()?;
            let h = st.doc.hyperlinks.iter().find(|h| h.id == id).cloned().ok_or_else(|| bad("hyperlink.goToSource", format!("no hyperlink {id}")))?;
            st.selection = match h.source {
                HyperlinkSource::Item { id } => designcraft_doc::Selection::items(vec![id]),
                HyperlinkSource::Text { story, start, end } => {
                    let frame = st.doc.story(story).and_then(|x| x.frames.first().copied());
                    designcraft_doc::Selection::text(designcraft_doc::TextSel { story, anchor: start, focus: end, frame, cell: None })
                }
            };
            st.bump_revision();
            Ok(Value::Null)
        }),
        cmd!("bookmark.rename", "Rename Bookmark", [], None, "{index? | path?: [usize], name}", has_doc, |s, p| {
            let path = bookmark_path_param("bookmark.rename", p)?;
            let name =
                str_param(p, "name").map(str::trim).filter(|n| !n.is_empty()).ok_or_else(|| bad("bookmark.rename", "missing name"))?.to_string();
            s.edit(|d, _| {
                let b = bookmark_at_mut(&mut d.bookmarks, &path).ok_or_else(|| bad("bookmark.rename", format!("no bookmark {path:?}")))?;
                b.name = name;
                Ok(Value::Null)
            })
        }),
        cmd!(
            "bookmark.add",
            "New Bookmark",
            [],
            None,
            "{name?, anchor?: text anchor id or name | page?: 1-based; default: anchor at selected text}",
            has_doc,
            |s, p| {
                let page = p.get("page").and_then(Value::as_u64).map(|v| (v as usize).saturating_sub(1)).unwrap_or(0);
                let n = s.doc()?.doc.page_count();
                if page >= n {
                    return Err(bad("bookmark.add", format!("no page {}", page + 1)));
                }
                let name = str_param(p, "name").map(str::to_string);
                let anchor = match dest_param("bookmark.add", &s.doc()?.doc, p)? {
                    Some(HyperlinkDest::Anchor(a)) => Some(a),
                    _ => None,
                };
                s.edit(|d, sel| {
                    let name = name
                        .clone()
                        .or_else(|| anchor.and_then(|a| d.anchor(a).map(|a| a.name.clone())))
                        .unwrap_or_else(|| format!("Bookmark {}", d.bookmarks.len() + 1));
                    let anchor = if anchor.is_some() || p.get("page").is_some() {
                        anchor
                    } else {
                        let t = sel.text.filter(|t| t.cell.is_none()).ok_or_else(|| bad("bookmark.add", "select story text or give an anchor"))?;
                        let st = d.story_mut(t.story).ok_or(designcraft_doc::DocError::NoStory(t.story))?;
                        let at = t.range().start.min(st.len());
                        let existing = st.anchors.iter().find(|a| st.anchor_pos(a.id) == Some(at)).map(|a| a.id);
                        let id = if let Some(id) = existing {
                            id
                        } else {
                            let id = st.insert_anchor(at, &name);
                            let shift = designcraft_doc::ANCHOR_MARK.len_utf8();
                            d.adjust_text_ranges(t.story, at..at, shift);
                            let start = if t.is_caret() { at.saturating_add(shift) } else { at };
                            sel.text = Some(designcraft_doc::TextSel { anchor: start, focus: t.range().end.saturating_add(shift), ..t });
                            id
                        };
                        Some(id)
                    };
                    if let Some((sid, pos)) = anchor.and_then(|a| d.find_anchor(a))
                        && let Some(t) = sel.text.as_mut().filter(|t| t.story == sid && t.cell.is_none() && !t.is_caret())
                        && t.range().start == pos.saturating_add(designcraft_doc::ANCHOR_MARK.len_utf8())
                    {
                        if t.anchor < t.focus {
                            t.anchor = pos;
                        } else {
                            t.focus = pos;
                        }
                    }
                    d.bookmarks.push(Bookmark { name, anchor, page, children: vec![], generated_by_toc: false });
                    if anchor.is_some() {
                        d.sync_anchor_bookmarks();
                    }
                    let index = d.bookmarks.len().saturating_sub(1);
                    Ok(json!({"index": index}))
                })
            }
        ),
        cmd!("bookmark.delete", "Delete Bookmark", [], None, "{index? | path?: [usize] | anchor?: text anchor id}", has_doc, |s, p| {
            let anchor = p.get("anchor").and_then(Value::as_u64);
            let path = if anchor.is_some() { None } else { Some(bookmark_path_param("bookmark.delete", p)?) };
            s.edit(|d, _| {
                if let Some(id) = anchor {
                    remove_bookmarks_for_anchor(&mut d.bookmarks, id);
                    d.sync_anchor_bookmarks();
                } else if let Some(path) = path
                    && let Some((parent_list, i)) = bookmark_list_and_index_mut(&mut d.bookmarks, &path)
                    && i < parent_list.len()
                {
                    let removed = parent_list.remove(i);
                    parent_list.splice(i..i, removed.children);
                    d.sync_anchor_bookmarks();
                }
                Ok(Value::Null)
            })
        }),
        cmd!(query "bookmark.list", "Bookmarks", [], None, "{} — pages are resolved from current text layout; null means unresolved", has_doc, |s, _| {
            let d = s.doc()?.doc.clone();
            let index = s.cache.xref_index(&d);
            fn resolved(b: &Bookmark, index: &designcraft_compose::xref::XrefIndex) -> Value {
                let page = match b.anchor { Some(a) => index.page_index(a), None => Some(b.page) };
                json!({"name": b.name, "anchor": b.anchor, "page": page, "generatedByToc": b.generated_by_toc,
                    "children": b.children.iter().map(|c| resolved(c, index)).collect::<Vec<_>>()})
            }
            Ok(Value::Array(d.bookmarks.iter().map(|b| resolved(b, &index)).collect()))
        }),
    ]
}

/// Parse `path: [usize, ...]` or top-level `index: usize` for bookmark commands.
fn bookmark_path_param(id: &str, p: &Value) -> Result<Vec<usize>> {
    if let Some(arr) = p.get("path").and_then(Value::as_array) {
        let path: Option<Vec<usize>> = arr.iter().map(|v| v.as_u64().and_then(|n| usize::try_from(n).ok())).collect();
        let path = path.filter(|v| !v.is_empty()).ok_or_else(|| bad(id, "invalid bookmark path"))?;
        return Ok(path);
    }
    let i = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad(id, "missing index"))? as usize;
    Ok(vec![i])
}

fn bookmark_at_mut<'a>(mut list: &'a mut [Bookmark], path: &[usize]) -> Option<&'a mut Bookmark> {
    let (&last, parents) = path.split_last()?;
    for &i in parents {
        list = &mut list.get_mut(i)?.children;
    }
    list.get_mut(last)
}

fn bookmark_list_and_index_mut<'a>(mut list: &'a mut Vec<Bookmark>, path: &[usize]) -> Option<(&'a mut Vec<Bookmark>, usize)> {
    let (&last, parents) = path.split_last()?;
    for &i in parents {
        list = &mut list.get_mut(i)?.children;
    }
    Some((list, last))
}

fn remove_bookmarks_for_anchor(root: &mut Vec<Bookmark>, id: u64) {
    let mut pending = vec![root];
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

fn strip_mailto(s: &str) -> &str {
    let trimmed = s.trim();
    if trimmed.as_bytes().get(..7).is_some_and(|p| p.eq_ignore_ascii_case(b"mailto:")) {
        trimmed[7..].trim_start()
    } else {
        trimmed
    }
}

/// `url`, `email`, `page` (1-based) or `anchor` (a text anchor's id, or its name).
///
/// `None` is "no destination given", which `hyperlink.edit` needs for an edit that only renames.
/// An `anchor` that is given and names none is an error, not `None`: an unknown name made
/// `hyperlink.create` fall through to `page` and an unknown id was stored as it stood, so the
/// link went to an anchor nobody has and the export quietly left it out -- and on
/// `hyperlink.edit` the `None` read as "no destination given", so the command reported success
/// and kept the old destination (noted in review).
fn dest_param(id: &str, d: &designcraft_doc::Document, p: &Value) -> Result<Option<HyperlinkDest>> {
    if let Some(u) = str_param(p, "url") {
        if !designcraft_doc::is_safe_link_url(u) {
            return Err(bad(id, "`url` must start with http://, https:// or mailto:"));
        }
        return Ok(Some(HyperlinkDest::Url(u.to_string())));
    }
    if let Some(e) = str_param(p, "email") {
        let addr = strip_mailto(e);
        if !designcraft_doc::is_safe_link_url(&format!("mailto:{addr}")) {
            return Err(bad(id, "`email` must be a non-empty address with no whitespace"));
        }
        return Ok(Some(HyperlinkDest::Email(addr.to_string())));
    }
    if let Some(a) = p.get("anchor").filter(|a| !a.is_null()) {
        let anchors = || d.stories.values().flat_map(|st| st.anchors.iter());
        // An id names one anchor or none.
        if let Some(n) = a.as_u64() {
            return match anchors().any(|x| x.id == n) {
                true => Ok(Some(HyperlinkDest::Anchor(n))),
                false => Err(bad(id, format!("no text anchor {a} (anchor.list names them)"))),
            };
        }
        // A name need not. `anchor.create` refuses a name in use, but an IDML import can bring a
        // document that already has two of the same, so taking the first match stored whichever
        // anchor happened to come first, and a link on a name shared by anchors on two pages went
        // to the wrong page with nothing to say so (noted in review). The id still reaches either
        // of them.
        let Some(name) = a.as_str() else { return Err(bad(id, format!("no text anchor {a} (anchor.list names them)"))) };
        let mut named = anchors().filter(|x| x.name == name);
        return match (named.next(), named.next()) {
            (Some(x), None) => Ok(Some(HyperlinkDest::Anchor(x.id))),
            (Some(_), Some(_)) => {
                let ids: Vec<String> = anchors().filter(|x| x.name == name).map(|x| x.id.to_string()).collect();
                Err(bad(id, format!("more than one text anchor is named {name:?} ({}); give the id instead", ids.join(", "))))
            }
            (None, _) => Err(bad(id, format!("no text anchor {a} (anchor.list names them)"))),
        };
    }
    Ok(p.get("page").and_then(Value::as_u64).map(|pg| HyperlinkDest::Page((pg as usize).saturating_sub(1))))
}

/// The anchor's own name, for a link to one that was not given a name.
fn dest_label(d: &designcraft_doc::Document, dest: &HyperlinkDest) -> String {
    if let HyperlinkDest::Anchor(a) = dest
        && let Some(x) = d.stories.values().flat_map(|st| st.anchors.iter()).find(|x| x.id == *a)
    {
        return x.name.clone();
    }
    dest.label()
}

fn create(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let dest = dest_param("hyperlink.create", &st.doc, p)?.ok_or_else(|| bad("hyperlink.create", "give `url`, `email`, `page` or `anchor`"))?;
    let sources: Vec<HyperlinkSource> = if let Some(ids) = super::ids_param(p, "ids") {
        ids.into_iter().map(|id| HyperlinkSource::Item { id }).collect()
    } else if let Some(t) = st.selection.text.filter(|t| !t.is_caret()) {
        vec![HyperlinkSource::Text { story: t.story, start: t.range().start, end: t.range().end }]
    } else {
        st.selection.items.iter().map(|id| HyperlinkSource::Item { id: *id }).collect()
    };
    if sources.is_empty() {
        return Err(bad("hyperlink.create", "select text or frames first"));
    }
    let name = str_param(p, "name").map(str::to_string);
    s.edit(|d, _| {
        let mut ids = vec![];
        for src in sources {
            let id = d.alloc();
            let name = name.clone().unwrap_or_else(|| dest_label(d, &dest));
            d.hyperlinks.push(Hyperlink { id, name, source: src, dest: dest.clone() });
            ids.push(id);
        }
        Ok(json!({"ids": ids}))
    })
}

#[cfg(test)]
mod panel_tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn hyperlinks_edit_and_go_to_source_bookmarks_rename() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 2})).unwrap();
        let r = s.execute("frame.create", &json!({"rect": [72, 72, 400, 300], "content": "text", "text": "Visit us"})).unwrap();
        s.execute("text.select", &json!({"story": r["story"], "anchor": 6, "focus": 8})).unwrap();
        let h = s.execute("hyperlink.create", &json!({"url": "https://a.example"})).unwrap()["ids"][0].clone();
        s.execute("hyperlink.edit", &json!({"id": h, "url": "https://b.example", "name": "Site"})).unwrap();
        let l = s.execute("hyperlink.list", &json!({})).unwrap();
        assert_eq!(l[0]["name"], "Site");
        assert!(l[0].to_string().contains("b.example"));
        s.execute("edit.deselectAll", &json!({})).unwrap();
        s.execute("hyperlink.goToSource", &json!({"id": h})).unwrap();
        let t = s.doc().unwrap().selection.text.unwrap();
        assert_eq!((t.anchor, t.focus), (6, 8));
        s.execute("bookmark.add", &json!({"page": 2})).unwrap();
        s.execute("bookmark.rename", &json!({"index": 0, "name": "Back"})).unwrap();
        assert_eq!(s.execute("bookmark.list", &json!({})).unwrap()[0]["name"], "Back");
    }

    #[test]
    fn hyperlinks_refuse_non_web_schemes() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 1})).unwrap();
        let id = s.execute("frame.create", &json!({"rect": [72, 72, 200, 120]})).unwrap()["id"].clone();
        for bad in ["javascript:alert(1)", "file:///etc/passwd", "data:text/html,hi", "\\\\attacker\\share"] {
            let e = s.execute("hyperlink.create", &json!({"ids": [id.clone()], "url": bad})).expect_err("refused").to_string();
            assert!(e.contains("http://"), "{bad}: {e}");
        }
        s.execute("hyperlink.create", &json!({"ids": [id.clone()], "url": "mailto:info@example.com"})).unwrap();
        s.execute("hyperlink.create", &json!({"ids": [id], "url": "https://example.com"})).unwrap();
    }

    #[test]
    fn hyperlinks_normalize_and_validate_email_destinations() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 1})).unwrap();
        let id = s.execute("frame.create", &json!({"rect": [72, 72, 200, 120]})).unwrap()["id"].clone();
        for bad in ["", "   ", "mailto:", "alice bob@example.com"] {
            s.execute("hyperlink.create", &json!({"ids": [id.clone()], "email": bad})).expect_err("refused");
        }
        s.execute("hyperlink.create", &json!({"ids": [id.clone()], "email": "alice@example.com"})).unwrap();
        s.execute("hyperlink.create", &json!({"ids": [id], "email": "MAILTO:bob@example.com"})).unwrap();
        let list = s.execute("hyperlink.list", &json!({})).unwrap();
        assert_eq!(list[0]["dest"], json!({"kind": "email", "value": "alice@example.com"}));
        assert_eq!(list[1]["dest"], json!({"kind": "email", "value": "bob@example.com"}));
    }
}

#[cfg(test)]
mod anchor_dest_tests {
    use serde_json::json;

    use crate::Session;

    fn outline_pages(s: &mut Session) -> Vec<usize> {
        let r = s.execute("file.exportPdf", &json!({})).unwrap();
        let bytes = super::super::file::base64_decode(r["base64"].as_str().unwrap());
        let pdf = String::from_utf8_lossy(&bytes);
        let kids: Vec<_> =
            pdf.split("/Kids[").nth(1).unwrap().split(']').next().unwrap().split(" 0 R").map(str::trim).filter(|s| !s.is_empty()).collect();
        pdf.split("/Title")
            .skip(1)
            .filter_map(|part| {
                let dest = part.split("/Dest ").nth(1)?.split(" 0 R").next()?.trim();
                let object = pdf.split(&format!("\n{dest} 0 obj\n")).nth(1)?.split("\nendobj").next()?;
                let page = object.trim_start_matches('[').split(" 0 R").next()?.trim();
                kids.iter().position(|k| *k == page)
            })
            .collect()
    }

    #[test]
    fn text_bookmark_follows_reflow_in_panel_and_pdf() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 300, "height": 300, "pages": 2, "facingPages": false, "margins": 0})).unwrap();
        let a = s.execute("frame.create", &json!({"rect": [20,20,280,65], "content": "text", "text": "Chapter"})).unwrap();
        let spread = s.doc().unwrap().doc.page_loc(1).unwrap().0;
        let b = s.execute("frame.create", &json!({"spread": spread, "rect": [20,20,280,280], "content": "text"})).unwrap();
        s.execute("text.thread", &json!({"from": a["id"], "to": b["id"]})).unwrap();
        s.execute("text.select", &json!({"story": a["story"], "anchor": 0, "focus": 7})).unwrap();
        s.execute("bookmark.add", &json!({"name": "Chapter"})).unwrap();
        let anchor = s.execute("bookmark.list", &json!({})).unwrap()[0]["anchor"].clone();
        assert!(anchor.is_u64());
        assert_eq!(s.execute("bookmark.list", &json!({})).unwrap()[0]["page"], 0);
        assert_eq!(outline_pages(&mut s), vec![0]);
        s.execute("text.select", &json!({"story": a["story"], "anchor": 0, "focus": 0})).unwrap();
        s.execute("text.insert", &json!({"text": "Before\nBefore\nBefore\nBefore\n"})).unwrap();
        assert_eq!(s.execute("bookmark.list", &json!({})).unwrap()[0]["page"], 1);
        assert_eq!(outline_pages(&mut s), vec![1]);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(outline_pages(&mut s), vec![0]);
        s.execute("edit.redo", &json!({})).unwrap();
        assert_eq!(outline_pages(&mut s), vec![1]);
        let d = &s.doc().unwrap().doc;
        let bytes = designcraft_format::save(d).unwrap();
        let back = designcraft_format::load(&bytes).unwrap();
        assert_eq!(back.bookmarks[0].anchor, anchor.as_u64());
        assert!(s.execute("anchor.delete", &json!({"id": anchor})).is_err());
        s.execute("text.select", &json!({"story": a["story"], "anchor": 0, "focus": 1000})).unwrap();
        s.execute("text.delete", &json!({})).unwrap();
        assert!(s.execute("bookmark.list", &json!({})).unwrap()[0]["page"].is_null());
        assert!(outline_pages(&mut s).is_empty());
    }

    #[test]
    fn bookmark_can_use_an_existing_anchor_and_old_page_bookmarks_still_load() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let f = s.execute("frame.create", &json!({"rect": [36,36,300,200], "content": "text", "text": "Heading"})).unwrap();
        s.execute("text.select", &json!({"story": f["story"], "anchor": 0})).unwrap();
        let a = s.execute("anchor.create", &json!({"name": "Heading"})).unwrap()["id"].clone();
        s.execute("bookmark.add", &json!({"anchor": "Heading"})).unwrap();
        let bookmarks = s.execute("bookmark.list", &json!({})).unwrap();
        assert_eq!(bookmarks[0]["anchor"], a);
        assert_eq!(bookmarks[0]["name"], "Heading");
        assert!(s.execute("bookmark.add", &json!({"anchor": "Missing"})).is_err());
        let old: designcraft_doc::Bookmark = serde_json::from_value(json!({"name": "Old", "page": 0})).unwrap();
        assert_eq!(old.anchor, None);
    }

    #[test]
    fn bookmark_follows_cut_text_but_copy_keeps_the_original_destination() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 2, "facingPages": false})).unwrap();
        let a = s.execute("frame.create", &json!({"rect": [36,36,300,200], "content": "text", "text": "Heading"})).unwrap();
        let spread = s.doc().unwrap().doc.page_loc(1).unwrap().0;
        let b = s.execute("frame.create", &json!({"spread": spread, "rect": [36,36,300,200], "content": "text"})).unwrap();
        s.execute("text.select", &json!({"story": a["story"], "anchor": 0, "focus": 7})).unwrap();
        s.execute("bookmark.add", &json!({"name": "Heading"})).unwrap();
        let anchor = s.execute("bookmark.list", &json!({})).unwrap()[0]["anchor"].clone();
        s.execute("edit.cut", &json!({})).unwrap();
        s.execute("text.select", &json!({"story": b["story"], "anchor": 0})).unwrap();
        s.execute("edit.paste", &json!({})).unwrap();
        assert_eq!(s.execute("bookmark.list", &json!({})).unwrap()[0]["page"], 1);
        assert_eq!(outline_pages(&mut s), vec![1]);
        let len = s.doc().unwrap().doc.story(designcraft_doc::StoryId(b["story"].as_u64().unwrap())).unwrap().len();
        s.execute("text.select", &json!({"story": b["story"], "anchor": 0, "focus": len})).unwrap();
        s.execute("edit.copy", &json!({})).unwrap();
        s.execute("text.select", &json!({"story": a["story"], "anchor": 0})).unwrap();
        s.execute("edit.paste", &json!({})).unwrap();
        assert_eq!(s.execute("bookmark.list", &json!({})).unwrap()[0]["anchor"], anchor);
        assert_eq!(outline_pages(&mut s), vec![1]);
        let anchors = s.execute("anchor.list", &json!({})).unwrap();
        assert_eq!(anchors.as_array().unwrap().len(), 2);
        assert_ne!(anchors[0]["id"], anchors[1]["id"]);
    }

    /// The page object numbers of a PDF, in order, and the page each link annotation points at
    /// (as an index into that list).
    fn link_pages(s: &mut Session) -> Vec<usize> {
        let r = s.execute("file.exportPdf", &json!({})).unwrap();
        let bytes = super::super::file::base64_decode(r["base64"].as_str().unwrap());
        let pdf = String::from_utf8_lossy(&bytes).into_owned();
        let kids: Vec<&str> = pdf
            .split("/Kids[")
            .nth(1)
            .and_then(|k| k.split(']').next())
            .map(|k| k.split(" 0 R").map(str::trim).filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();
        let object =
            |n: &str| -> String { pdf.split(&format!("\n{n} 0 obj\n")).nth(1).and_then(|o| o.split("\nendobj").next()).unwrap_or("").to_string() };
        let mut out = Vec::new();
        for part in pdf.split("/Subtype/Link").skip(1) {
            let Some(dest) = part.split("/Dest ").nth(1).and_then(|d| d.split(" 0 R").next()) else { continue };
            // `[<page> 0 R/XYZ …]`
            let page = object(dest.trim());
            let Some(page) = page.trim_start_matches('[').split(" 0 R").next() else { continue };
            if let Some(i) = kids.iter().position(|k| *k == page.trim()) {
                out.push(i);
            }
        }
        out
    }

    /// `anchor.create` does not require a name to be unique -- and an IDML import can bring a
    /// document that already has two of the same -- so the lookup taking the first match meant a
    /// link made on a shared name went to whichever anchor came first, a different page, with
    /// nothing to say so (noted in review). The id still reaches either of them.
    #[test]
    fn a_name_two_anchors_share_cannot_pick_one_of_them() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 300, "height": 300, "pages": 3, "facingPages": false, "margins": 0})).unwrap();

        // Two anchors with one name, in text on different pages. `anchor.create` refuses a name
        // in use, but a document opened from a file can still hold two, so the second is renamed
        // in the document directly.
        let mut made = Vec::new();
        for (page, name) in [(1usize, "Photo 1"), (2, "Photo 1 (second)")] {
            let sp = s.doc().unwrap().doc.page_loc(page).unwrap().0;
            let f = s
                .execute("frame.create", &json!({"spread": sp, "rect": [20, 20, 280, 100], "content": "text", "text": "Photo 1 full size"}))
                .unwrap();
            s.execute("text.select", &json!({"story": f["story"].clone(), "anchor": 0, "focus": 7})).unwrap();
            made.push(s.execute("anchor.create", &json!({"name": name})).unwrap());
        }
        s.edit(|d, _| {
            for sid in d.stories.keys().copied().collect::<Vec<_>>() {
                for a in &mut d.story_mut(sid).unwrap().anchors {
                    if a.name == "Photo 1 (second)" {
                        std::sync::Arc::make_mut(a).name = "Photo 1".into();
                    }
                }
            }
            Ok(())
        })
        .unwrap();
        assert_ne!(made[0]["id"], made[1]["id"], "two anchors, one name");
        let thumb = s.execute("frame.create", &json!({"rect": [20, 20, 120, 80], "content": "unassigned"})).unwrap();

        // Creating on the shared name is refused, and says what to do instead.
        let e = s.execute("hyperlink.create", &json!({"anchor": "Photo 1", "ids": [thumb["id"].clone()]})).expect_err("ambiguous").to_string();
        assert!(e.contains("more than one text anchor"), "{e}");
        assert!(e.contains("give the id instead"), "it says what to do instead: {e}");
        assert!(s.execute("hyperlink.list", &json!({})).unwrap().as_array().is_some_and(|l| l.is_empty()), "and no link was made");

        // By id it works, and the id picks the one that was asked for.
        s.execute("hyperlink.create", &json!({"anchor": made[1]["id"].clone(), "ids": [thumb["id"].clone()]})).unwrap();
        let live = s.execute("hyperlink.list", &json!({})).unwrap()[0]["id"].clone();
        let dest = |s: &mut Session| s.execute("hyperlink.list", &json!({})).unwrap()[0]["dest"].clone();
        assert_eq!(dest(&mut s), json!({"kind": "anchor", "value": made[1]["id"].clone()}));

        // An edit on the shared name is refused and leaves the destination as it was.
        let e = s.execute("hyperlink.edit", &json!({"id": live.clone(), "anchor": "Photo 1"})).expect_err("ambiguous").to_string();
        assert!(e.contains("more than one text anchor"), "{e}");
        assert_eq!(dest(&mut s), json!({"kind": "anchor", "value": made[1]["id"].clone()}), "the refused edit changed nothing");

        // And the other anchor is still reachable by its id.
        s.execute("hyperlink.edit", &json!({"id": live, "anchor": made[0]["id"].clone()})).unwrap();
        assert_eq!(dest(&mut s), json!({"kind": "anchor", "value": made[0]["id"].clone()}));
    }

    /// A frame can link to a text anchor, as a cross-reference refers to one, and the link lands
    /// on whichever page the anchor's text is on when the PDF is written — IDML files link photo
    /// thumbnails to their full-size pictures that way (#1446).
    #[test]
    fn a_frame_can_link_to_a_text_anchor_and_follows_it() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 300, "height": 300, "pages": 3, "facingPages": false, "margins": 0})).unwrap();
        // The destination: an anchor in text on page 3.
        let sp = s.doc().unwrap().doc.page_loc(2).unwrap().0;
        let dest =
            s.execute("frame.create", &json!({"spread": sp, "rect": [20, 20, 280, 100], "content": "text", "text": "Photo 1 full size"})).unwrap();
        s.execute("text.select", &json!({"story": dest["story"].clone(), "anchor": 0, "focus": 7})).unwrap();
        let anchor = s.execute("anchor.create", &json!({"name": "Photo 1"})).unwrap();
        // The source: a thumbnail frame on page 1, linked by the anchor's name.
        let thumb = s.execute("frame.create", &json!({"rect": [20, 20, 120, 80], "content": "unassigned"})).unwrap();
        s.execute("hyperlink.create", &json!({"anchor": "Photo 1", "ids": [thumb["id"].clone()]})).unwrap();
        let links = s.execute("hyperlink.list", &json!({})).unwrap();
        assert_eq!(links[0]["dest"], json!({"kind": "anchor", "value": anchor["id"].clone()}), "{links}");
        // Named after the anchor, since no name was given.
        assert_eq!(links[0]["name"], "Photo 1");
        assert_eq!(link_pages(&mut s), vec![2], "the link lands on the anchor's page (the third)");

        // A page inserted in front moves the anchor's text to page 4, and the link follows it
        // without being edited.
        s.execute("layout.pages.insert", &json!({"count": 1, "after": 0})).unwrap();
        assert_eq!(s.execute("anchor.list", &json!({})).unwrap()[0]["page"], "4");
        assert_eq!(link_pages(&mut s), vec![3], "the link follows the text");

        // An anchor's id works as well as its name, and an unknown name is refused.
        s.execute("hyperlink.delete", &json!({"id": links[0]["id"].clone()})).unwrap();
        s.execute("hyperlink.create", &json!({"anchor": anchor["id"].clone(), "ids": [thumb["id"].clone()], "name": "Thumbnail"})).unwrap();
        assert_eq!(s.execute("hyperlink.list", &json!({})).unwrap()[0]["name"], "Thumbnail");
        assert_eq!(link_pages(&mut s), vec![3]);
        let e = s.execute("hyperlink.create", &json!({"anchor": "No Such Anchor", "ids": [thumb["id"].clone()]})).expect_err("refused").to_string();
        assert!(e.contains("no text anchor"), "and it says why, rather than that no destination was given: {e}");

        // An unknown anchor *id* is refused as well. `as_u64` answered for any number, so a link
        // was stored to an anchor nobody has and the export quietly left it out (noted in review).
        let live = s.execute("hyperlink.list", &json!({})).unwrap()[0]["id"].clone();
        let missing = anchor["id"].as_u64().expect("the anchor's id") + 9_999;
        let e = s.execute("hyperlink.create", &json!({"anchor": missing, "ids": [thumb["id"].clone()]})).expect_err("refused").to_string();
        assert!(e.contains("no text anchor"), "an unknown id is refused: {e}");

        // And `hyperlink.edit` reports it rather than reading it as "no destination given", which
        // made the command succeed and keep the old destination (noted in review).
        for unknown in [json!(missing), json!("No Such Anchor")] {
            let e = s.execute("hyperlink.edit", &json!({"id": live.clone(), "anchor": unknown.clone()})).expect_err("refused").to_string();
            assert!(e.contains("no text anchor"), "{unknown}: {e}");
        }
        let after = s.execute("hyperlink.list", &json!({})).unwrap();
        assert_eq!(after[0]["dest"], json!({"kind": "anchor", "value": anchor["id"].clone()}), "the destination is unchanged: {after}");
        assert_eq!(after[0]["name"], "Thumbnail", "and so is the name");
        assert_eq!(link_pages(&mut s), vec![3], "and the export still lands on the anchor's page");

        // A rename that names no destination still works: that is what `None` is for, and it must
        // not have become an error on the way.
        s.execute("hyperlink.edit", &json!({"id": live.clone(), "name": "Thumb"})).unwrap();
        let renamed = s.execute("hyperlink.list", &json!({})).unwrap();
        assert_eq!(renamed[0]["name"], "Thumb");
        assert_eq!(renamed[0]["dest"], json!({"kind": "anchor", "value": anchor["id"].clone()}), "the destination survives a rename");

        // A link to a deleted anchor is not exported, rather than pointing at the wrong page.
        s.execute("text.select", &json!({"story": dest["story"].clone(), "anchor": 0, "focus": 8})).unwrap();
        s.execute("text.delete", &json!({})).unwrap();
        assert!(s.execute("anchor.list", &json!({})).unwrap().as_array().unwrap().is_empty());
        assert_eq!(link_pages(&mut s), Vec::<usize>::new(), "no link rather than a wrong one");
    }
}
