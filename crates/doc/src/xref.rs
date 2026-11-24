//! Text anchors and cross-references (Type › Hyperlinks & Cross-References).
//!
//! A text anchor is a zero-width [`ANCHOR_MARK`] character; the story's [`Story::anchors`] holds
//! one [`TextAnchor`] per mark in text order (like footnotes). A cross-reference is an
//! [`XREF_MARK`] character with one [`CrossRef`] per mark in [`Story::xrefs`]; it shows text
//! generated from its format (paragraph text, page number, …) and is resolved at composition, so
//! it never goes stale (InDesign's cross-references need Update Cross-References).
//!
//! Anchor ids are document-unique: `story id << 24 | n` when created, kept when text moves.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::Document;
use crate::ids::StoryId;
use crate::story::Story;

/// A text anchor (destination of cross-references and hyperlinks).
pub const ANCHOR_MARK: char = '\u{E00B}';
/// A cross-reference source (generated text).
pub const XREF_MARK: char = '\u{E00C}';

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextAnchor {
    pub id: u64,
    /// Shown in the Hyperlinks panel; paragraph destinations get a generated name.
    pub name: String,
    /// Optional parent text anchor id forming a hierarchy of text anchors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossRef {
    /// The destination anchor (0 = unresolved, e.g. pasted without its destination).
    pub target: u64,
    /// Name of the format in [`Document::xref_formats`].
    pub format: String,
    /// Text typed over the generated text (InDesign keeps a cross-reference's edited text until
    /// it is updated): shown as is, except that each `<pageNum />` shows the destination's page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The page last shown (for edited text whose destination is gone).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

/// The `<pageNum />` building block, as it appears in [`CrossRef::text`].
pub const PAGE_NUM_BLOCK: &str = "<pageNum />";

/// Edited cross-reference text with its page numbers filled in (quotes kept as typed).
pub fn expand_edited(text: &str, page: &str) -> String {
    text.replace(PAGE_NUM_BLOCK, page)
}

/// A cross-reference format: building blocks between literal text.
///
/// Blocks: `<fullPara />`, `<paraText />` (without the list number), `<paraNum />`,
/// `<pageNum />`, `<txtAnchrName />`, `<chapNum />`, `<fileName />`,
/// `<partialPara delim="x" includeDelim="false" />`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XrefFormat {
    pub name: String,
    pub definition: String,
}

pub fn default_formats() -> Vec<XrefFormat> {
    let f = |name: &str, definition: &str| XrefFormat { name: name.into(), definition: definition.into() };
    vec![
        f("Full Paragraph & Page Number", "\"<fullPara />\" on page <pageNum />"),
        f("Full Paragraph", "\"<fullPara />\""),
        f("Paragraph Text & Page Number", "\"<paraText />\" on page <pageNum />"),
        f("Paragraph Text", "\"<paraText />\""),
        f("Paragraph Number & Page Number", "<paraNum /> on page <pageNum />"),
        f("Paragraph Number", "<paraNum />"),
        f("Text Anchor Name & Page Number", "\"<txtAnchrName />\" on page <pageNum />"),
        f("Text Anchor Name", "\"<txtAnchrName />\""),
        f("Page Number", "page <pageNum />"),
        f("Partial Paragraph & Page Number", "\"<partialPara delim=\":\" includeDelim=\"false\" />\" on page <pageNum />"),
        f("Partial Paragraph", "\"<partialPara delim=\":\" includeDelim=\"false\" />\""),
    ]
}

/// Values a format can show for one destination.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct XrefValues {
    pub para: String,
    pub para_num: String,
    pub page: String,
    pub anchor_name: String,
    pub chapter: String,
    pub file_name: String,
}

/// Expand a format definition.
pub fn expand(definition: &str, v: &XrefValues) -> String {
    let mut out = String::new();
    let mut rest = definition;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let Some(j) = tag_end(&rest[i..]) else {
            out.push_str(&rest[i..]);
            rest = "";
            break;
        };
        let tag = rest[i + 1..i + j].trim().trim_end_matches('/').trim();
        let name = tag.split_whitespace().next().unwrap_or("");
        match name {
            "fullPara" => {
                if !v.para_num.is_empty() {
                    out.push_str(&v.para_num);
                    out.push(' ');
                }
                out.push_str(&v.para);
            }
            "paraText" => out.push_str(&v.para),
            "paraNum" => out.push_str(&v.para_num),
            "pageNum" => out.push_str(&v.page),
            "txtAnchrName" => out.push_str(&v.anchor_name),
            "chapNum" => out.push_str(&v.chapter),
            "fileName" => out.push_str(&v.file_name),
            "partialPara" => {
                let delim = attr(tag, "delim").unwrap_or_default();
                let include = attr(tag, "includeDelim").is_some_and(|x| x == "true");
                match (!delim.is_empty()).then(|| v.para.find(&delim)).flatten() {
                    Some(k) => out.push_str(&v.para[..if include { k + delim.len() } else { k }]),
                    None => out.push_str(&v.para),
                }
            }
            _ => out.push_str(&rest[i..i + j + 1]),
        }
        rest = &rest[i + j + 1..];
    }
    out.push_str(rest);
    typographic_quotes(&out)
}

/// Byte index of the `>` that ends the tag starting at `s`, skipping quoted attribute values.
/// An unterminated quote falls back to the first `>`, so only that malformed tag is affected.
fn tag_end(s: &str) -> Option<usize> {
    let mut in_quote = false;
    for (k, c) in s.char_indices() {
        match c {
            '"' => in_quote = !in_quote,
            '>' if !in_quote => return Some(k),
            _ => {}
        }
    }
    s.find('>')
}

/// Straight double quotes in format text become typographer's quotes.
fn typographic_quotes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev = ' ';
    for c in s.chars() {
        out.push(match c {
            '"' if prev.is_whitespace() || matches!(prev, '(' | '[') => '\u{201C}',
            '"' => '\u{201D}',
            c => c,
        });
        prev = c;
    }
    out
}

fn attr(tag: &str, key: &str) -> Option<String> {
    let k = tag.find(&format!("{key}=\""))? + key.len() + 2;
    let e = tag[k..].find('"')?;
    Some(tag[k..k + e].to_string())
}

impl Story {
    fn marks_before(&self, mark: char, pos: usize) -> usize {
        let pos = crate::story::floor_char_boundary(&self.text, pos);
        self.text[..pos].matches(mark).count()
    }

    /// Byte offset of an anchor's mark.
    pub fn anchor_pos(&self, id: u64) -> Option<usize> {
        let k = self.anchors.iter().position(|a| a.id == id)?;
        self.text.match_indices(ANCHOR_MARK).nth(k).map(|(i, _)| i)
    }

    /// The cross-reference whose mark is at byte `pos`.
    pub fn xref_at(&self, pos: usize) -> Option<&CrossRef> {
        if !self.text[pos.min(self.text.len())..].starts_with(XREF_MARK) {
            return None;
        }
        self.xrefs.get(self.marks_before(XREF_MARK, pos)).map(|x| &**x)
    }

    fn next_anchor_id(&self) -> u64 {
        let base = self.id.0 << 24;
        self.anchors.iter().filter(|a| a.id >> 24 == self.id.0).map(|a| a.id + 1).max().unwrap_or(base + 1).max(base + 1)
    }

    /// Insert a text anchor at `pos`. Returns its id.
    pub fn insert_anchor(&mut self, pos: usize, name: &str) -> u64 {
        let pos = crate::story::floor_char_boundary(&self.text, pos.min(self.len()));
        self.insert(pos, &ANCHOR_MARK.to_string());
        let k = self.marks_before(ANCHOR_MARK, pos);
        let a = Arc::make_mut(&mut self.anchors[k]);
        a.name = name.to_string();
        a.id
    }

    /// Insert a cross-reference at `pos` (replacing nothing).
    pub fn insert_xref(&mut self, pos: usize, xref: CrossRef) {
        let pos = crate::story::floor_char_boundary(&self.text, pos.min(self.len()));
        self.insert(pos, &XREF_MARK.to_string());
        let k = self.marks_before(XREF_MARK, pos);
        self.xrefs[k] = Arc::new(xref);
    }

    /// Keep anchor/xref lists in step with an insertion of `text` at `pos` (before it happens).
    pub(crate) fn marks_inserted(&mut self, pos: usize, text: &str) {
        let n = text.matches(ANCHOR_MARK).count();
        if n > 0 {
            let k = self.marks_before(ANCHOR_MARK, pos);
            for i in 0..n {
                let id = self.next_anchor_id();
                self.anchors.insert(k + i, Arc::new(TextAnchor { id, name: format!("Anchor {}", id & 0xFF_FFFF), parent: None }));
            }
        }
        let n = text.matches(XREF_MARK).count();
        if n > 0 {
            let k = self.marks_before(XREF_MARK, pos);
            for i in 0..n {
                self.xrefs.insert(k + i, Arc::new(CrossRef::default()));
            }
        }
        let n = text.matches(crate::index::INDEX_MARK).count();
        if n > 0 {
            let k = self.marks_before(crate::index::INDEX_MARK, pos);
            for i in 0..n {
                self.index_refs.insert(k + i, Arc::new(crate::index::IndexRef::default()));
            }
        }
        let n = text.matches(crate::endnotes::NOTE_MARK).count();
        if n > 0 {
            let k = self.marks_before(crate::endnotes::NOTE_MARK, pos);
            for i in 0..n {
                let id = self.editorial.iter().map(|x| x.id).max().unwrap_or(0) + 1;
                self.editorial.insert((k + i).min(self.editorial.len()), Arc::new(crate::endnotes::EditorialNote { id, ..Default::default() }));
            }
        }
        let n = text.matches(crate::anchored::OBJECT_MARK).count();
        if n > 0 {
            let k = self.marks_before(crate::anchored::OBJECT_MARK, pos);
            for i in 0..n {
                self.objects.insert(k + i, Arc::new(crate::anchored::AnchoredObject::default()));
            }
        }
    }

    /// Insert an index page reference at `pos`.
    pub fn insert_index_ref(&mut self, pos: usize, r: crate::index::IndexRef) {
        let pos = crate::story::floor_char_boundary(&self.text, pos.min(self.len()));
        self.insert(pos, &crate::index::INDEX_MARK.to_string());
        let k = self.marks_before(crate::index::INDEX_MARK, pos);
        self.index_refs[k] = Arc::new(r);
    }

    /// Keep anchor/xref lists in step with a deletion of `a..b` (before it happens).
    pub(crate) fn marks_deleted(&mut self, a: usize, b: usize) {
        for mark in [ANCHOR_MARK, XREF_MARK, crate::index::INDEX_MARK, crate::anchored::OBJECT_MARK, crate::endnotes::NOTE_MARK] {
            let n = self.text[a..b].matches(mark).count();
            if n == 0 {
                continue;
            }
            let k = self.marks_before(mark, a);
            if mark == ANCHOR_MARK {
                self.anchors.drain(k..(k + n).min(self.anchors.len()));
            } else if mark == XREF_MARK {
                self.xrefs.drain(k..(k + n).min(self.xrefs.len()));
            } else if mark == crate::index::INDEX_MARK {
                self.index_refs.drain(k..(k + n).min(self.index_refs.len()));
            } else if mark == crate::endnotes::NOTE_MARK {
                self.editorial.drain(k..(k + n).min(self.editorial.len()));
            } else {
                self.objects.drain(k..(k + n).min(self.objects.len()));
            }
        }
    }

    /// Make the lists match the marks after wholesale text changes.
    pub fn fix_marks(&mut self) {
        let n = self.text.matches(ANCHOR_MARK).count();
        while self.anchors.len() > n {
            self.anchors.pop();
        }
        while self.anchors.len() < n {
            let id = self.next_anchor_id();
            self.anchors.push(Arc::new(TextAnchor { id, name: format!("Anchor {}", id & 0xFF_FFFF), parent: None }));
        }
        let n = self.text.matches(XREF_MARK).count();
        self.xrefs.truncate(n);
        while self.xrefs.len() < n {
            self.xrefs.push(Arc::new(CrossRef::default()));
        }
        let n = self.text.matches(crate::index::INDEX_MARK).count();
        self.index_refs.truncate(n);
        while self.index_refs.len() < n {
            self.index_refs.push(Arc::new(crate::index::IndexRef::default()));
        }
        let n = self.text.matches(crate::endnotes::NOTE_MARK).count();
        self.editorial.truncate(n);
        while self.editorial.len() < n {
            let id = self.editorial.iter().map(|x| x.id).max().unwrap_or(0) + 1;
            self.editorial.push(Arc::new(crate::endnotes::EditorialNote { id, ..Default::default() }));
        }
        let n = self.text.matches(crate::anchored::OBJECT_MARK).count();
        self.objects.truncate(n);
        while self.objects.len() < n {
            self.objects.push(Arc::new(crate::anchored::AnchoredObject::default()));
        }
    }

    pub(crate) fn check_marks(&self) -> Result<(), String> {
        let (a, x) = (self.text.matches(ANCHOR_MARK).count(), self.text.matches(XREF_MARK).count());
        if a != self.anchors.len() || x != self.xrefs.len() {
            return Err(format!("story {}: {a} anchor / {x} xref marks for {} anchors / {} xrefs", self.id.0, self.anchors.len(), self.xrefs.len()));
        }
        let o = self.text.matches(crate::anchored::OBJECT_MARK).count();
        if o != self.objects.len() {
            return Err(format!("story {}: {o} object marks for {} anchored objects", self.id.0, self.objects.len()));
        }
        let e = self.text.matches(crate::endnotes::NOTE_MARK).count();
        if e != self.editorial.len() {
            return Err(format!("story {}: {e} note marks for {} notes", self.id.0, self.editorial.len()));
        }
        let i = self.text.matches(crate::index::INDEX_MARK).count();
        if i != self.index_refs.len() {
            return Err(format!("story {}: {i} index marks for {} references", self.id.0, self.index_refs.len()));
        }
        Ok(())
    }
}

impl Document {
    /// The text a cross-reference to `target` in `format` generates, with each page number as
    /// [`PAGE_NUM_BLOCK`] (the form saved text takes); `None` when the destination is gone.
    pub fn generated_xref_text(&self, target: u64, format: &str) -> Option<String> {
        let mut v = self.xref_values(target)?;
        v.page = PAGE_NUM_BLOCK.into();
        Some(expand(self.xref_format(format).map_or("<fullPara />", |f| f.definition.as_str()), &v))
    }

    /// Forget saved cross-reference text that is exactly what its format generates for its
    /// destination, so it is generated again (and follows edits to the destination). Any other
    /// saved text was typed over the generated text, or its destination is gone: it is kept as
    /// shown.
    pub fn drop_generated_xref_text(&mut self) {
        let generated: Vec<(StoryId, usize)> = self
            .stories
            .iter()
            .flat_map(|(sid, st)| st.xrefs.iter().enumerate().map(move |(k, x)| (*sid, k, x)))
            .filter(|(_, _, x)| x.text.as_ref().is_some_and(|t| self.generated_xref_text(x.target, &x.format).as_ref() == Some(t)))
            .map(|(sid, k, _)| (sid, k))
            .collect();
        for (sid, k) in generated {
            if let Some(x) = self.story_mut(sid).and_then(|st| st.xrefs.get_mut(k)) {
                let x = Arc::make_mut(x);
                x.text = None;
                x.page = None;
            }
        }
    }

    /// (story, byte) of anchor `id`.
    pub fn find_anchor(&self, id: u64) -> Option<(StoryId, usize)> {
        if id == 0 {
            return None;
        }
        // Fast path: the story the id was created in.
        let home = StoryId(id >> 24);
        if let Some(p) = self.story(home).and_then(|s| s.anchor_pos(id)) {
            return Some((home, p));
        }
        self.stories.values().find_map(|s| s.anchor_pos(id).map(|p| (s.id, p)))
    }

    pub fn anchor(&self, id: u64) -> Option<&TextAnchor> {
        self.stories.values().find_map(|s| s.anchors.iter().find(|a| a.id == id).map(|a| &**a))
    }

    /// Whether making `candidate_parent` the parent of anchor `id` would introduce a cycle.
    pub fn anchor_parent_creates_cycle(&self, id: u64, candidate_parent: u64) -> bool {
        let mut visited = std::collections::HashSet::new();
        let mut cur = Some(candidate_parent);
        while let Some(curr) = cur {
            if curr == id {
                return true;
            }
            if !visited.insert(curr) {
                return false;
            }
            cur = self.anchor(curr).and_then(|a| a.parent);
        }
        false
    }

    /// Valid parent anchor id for `id`, ignoring dangling or cyclic parent links.
    pub fn effective_anchor_parent(&self, id: u64) -> Option<u64> {
        let parent = self.anchor(id)?.parent?;
        if self.find_anchor(parent).is_none() || self.anchor_parent_creates_cycle(id, parent) {
            return None;
        }
        Some(parent)
    }

    /// Reconcile anchored bookmark nesting in `self.bookmarks` with [`TextAnchor::parent`].
    ///
    /// Anchored bookmarks whose anchor has a nearest bookmarked ancestor in the anchor tree move
    /// under that ancestor's bookmark. Anchored bookmarks with no bookmarked ancestor in the
    /// anchor tree that sit inside another anchored bookmark move back to the top level. Legacy
    /// page bookmarks, TOC bookmarks, and bookmarks nested under page bookmarks stay in place.
    pub fn sync_anchor_bookmarks(&mut self) {
        let mut bookmarks = std::mem::take(&mut self.bookmarks);
        self.reconcile_anchor_bookmarks(&mut bookmarks, true);
        self.bookmarks = bookmarks;
    }

    /// A copy of `self.bookmarks` with [`TextAnchor::parent`] hierarchy applied without mutating
    /// the document or flattening legacy manually-nested bookmarks whose anchors have no parent.
    pub fn bookmarks_for_export(&self) -> Vec<crate::Bookmark> {
        let mut bookmarks = self.bookmarks.clone();
        self.reconcile_anchor_bookmarks(&mut bookmarks, false);
        bookmarks
    }

    fn reconcile_anchor_bookmarks(&self, bookmarks: &mut Vec<crate::Bookmark>, unnest_roots: bool) {
        fn collect_anchors(list: &[crate::Bookmark], out: &mut std::collections::HashSet<u64>) {
            for b in list {
                if let Some(a) = b.anchor {
                    out.insert(a);
                }
                collect_anchors(&b.children, out);
            }
        }
        let mut bookmarked = std::collections::HashSet::new();
        collect_anchors(bookmarks, &mut bookmarked);
        if bookmarked.is_empty() {
            return;
        }
        let mut desired_parent: std::collections::HashMap<u64, Option<u64>> = std::collections::HashMap::new();
        for &a in &bookmarked {
            let mut cur = self.effective_anchor_parent(a);
            let mut seen = std::collections::HashSet::from([a]);
            let mut found = None;
            while let Some(p) = cur {
                if !seen.insert(p) {
                    break;
                }
                if bookmarked.contains(&p) {
                    found = Some(p);
                    break;
                }
                cur = self.effective_anchor_parent(p);
            }
            desired_parent.insert(a, found);
        }

        let mut to_root: Vec<crate::Bookmark> = Vec::new();
        let mut to_anchor: std::collections::HashMap<u64, Vec<crate::Bookmark>> = std::collections::HashMap::new();

        fn detach(
            list: &mut Vec<crate::Bookmark>,
            parent_anchor: Option<u64>,
            unnest_roots: bool,
            desired_parent: &std::collections::HashMap<u64, Option<u64>>,
            to_root: &mut Vec<crate::Bookmark>,
            to_anchor: &mut std::collections::HashMap<u64, Vec<crate::Bookmark>>,
        ) {
            let mut i = 0;
            while i < list.len() {
                let my_anchor = list[i].anchor;
                detach(&mut list[i].children, my_anchor, unnest_roots, desired_parent, to_root, to_anchor);
                let move_to = match my_anchor {
                    Some(a) => match desired_parent.get(&a).copied().flatten() {
                        Some(target) if parent_anchor != Some(target) => Some(Some(target)),
                        None if unnest_roots && parent_anchor.is_some() => Some(None),
                        _ => None,
                    },
                    None => None,
                };
                if let Some(dest) = move_to {
                    let b = list.remove(i);
                    match dest {
                        Some(target) => to_anchor.entry(target).or_default().push(b),
                        None => to_root.push(b),
                    }
                } else {
                    i += 1;
                }
            }
        }

        detach(bookmarks, None, unnest_roots, &desired_parent, &mut to_root, &mut to_anchor);
        bookmarks.extend(to_root);

        fn attach_pass(list: &mut [crate::Bookmark], to_anchor: &mut std::collections::HashMap<u64, Vec<crate::Bookmark>>) -> bool {
            let mut progress = false;
            for b in list.iter_mut() {
                if let Some(a) = b.anchor
                    && let Some(children) = to_anchor.remove(&a)
                {
                    b.children.extend(children);
                    progress = true;
                }
                if attach_pass(&mut b.children, to_anchor) {
                    progress = true;
                }
            }
            progress
        }
        while !to_anchor.is_empty() {
            if !attach_pass(bookmarks, &mut to_anchor) {
                for (_, leftover) in to_anchor.drain() {
                    bookmarks.extend(leftover);
                }
                break;
            }
        }
    }

    /// The anchor at the start of paragraph containing `pos` in `sid`, creating one if needed
    /// (cross-references to paragraphs point at such an anchor).
    pub fn paragraph_anchor(&mut self, sid: StoryId, pos: usize) -> Option<u64> {
        let st = self.story(sid)?;
        let pi = st.para_at(pos);
        let r = st.para_ranges()[pi].clone();
        if st.text[r.clone()].starts_with(ANCHOR_MARK) {
            let k = st.marks_before(ANCHOR_MARK, r.start);
            return st.anchors.get(k).map(|a| a.id);
        }
        let name: String = clean(&st.text[r.clone()]).chars().take(40).collect();
        let st = self.story_mut(sid)?;
        Some(st.insert_anchor(r.start, &name))
    }

    /// Values for the paragraph holding anchor `id` (page left empty: it needs composition).
    pub fn xref_values(&self, id: u64) -> Option<XrefValues> {
        let (sid, pos) = self.find_anchor(id)?;
        let st = self.story(sid)?;
        let pi = st.para_at(pos);
        let ranges = st.para_ranges();
        let r = ranges[pi].clone();
        let (pp, _) = self.styles.resolve_para(&st.paras[pi]);
        let para_num = match pp.list_type {
            crate::ListType::Numbers if !r.is_empty() => {
                // Count the run of numbered paragraphs before this one (empty ones have no number).
                let n = (0..=pi)
                    .rev()
                    .take_while(|&k| self.styles.resolve_para(&st.paras[k]).0.list_type == crate::ListType::Numbers)
                    .filter(|&k| ranges.get(k).is_some_and(|r| !r.is_empty()))
                    .count();
                pp.number_label(u32::try_from(n).unwrap_or(u32::MAX)).trim_end().to_string()
            }
            _ => String::new(),
        };
        Some(XrefValues {
            para: clean(&st.text[r]),
            para_num,
            page: String::new(),
            anchor_name: self.anchor(id).map(|a| a.name.clone()).unwrap_or_default(),
            chapter: self.chapter_label(),
            file_name: self.title.clone(),
        })
    }

    pub fn xref_format(&self, name: &str) -> Option<&XrefFormat> {
        self.xref_formats.iter().find(|f| f.name == name)
    }
}

/// Paragraph text without markers, tabs and breaks.
pub fn clean(s: &str) -> String {
    let t: String =
        s.chars().filter(|c| !('\u{E000}'..='\u{E1FF}').contains(c)).map(|c| if c == '\t' || c == '\u{2028}' { ' ' } else { c }).collect();
    t.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gt_inside_quoted_attribute_does_not_end_tag() {
        let v = XrefValues { para: "Intro > the start".into(), ..Default::default() };
        assert_eq!(expand("<partialPara delim=\">\" includeDelim=\"false\" />|", &v), "Intro |");
    }

    #[test]
    fn unterminated_lt_still_gets_typographic_quotes() {
        let v = XrefValues { para: "Intro".into(), ..Default::default() };
        assert_eq!(expand("\"<paraText />\" <", &v), "\u{201C}Intro\u{201D} <");
    }

    #[test]
    fn unterminated_quote_only_affects_its_own_tag() {
        let v = XrefValues { para: "Intro > the start".into(), ..Default::default() };
        let out = expand("<x a=\"b> <fullPara />", &v);
        assert!(out.contains("Intro > the start"), "{out}");
    }

    #[test]
    fn edited_text_fills_in_its_page_numbers() {
        assert_eq!(expand_edited("p. <pageNum /> and <pageNum />", "4"), "p. 4 and 4");
    }

    #[test]
    fn formats_expand() {
        let v =
            XrefValues { para: "Intro: the start".into(), page: "7".into(), para_num: "2.".into(), anchor_name: "A".into(), ..Default::default() };
        let fs = default_formats();
        let get = |n: &str| expand(&fs.iter().find(|f| f.name == n).unwrap().definition, &v);
        assert_eq!(get("Full Paragraph & Page Number"), "\u{201C}2. Intro: the start\u{201D} on page 7");
        assert_eq!(get("Paragraph Text"), "\u{201C}Intro: the start\u{201D}");
        assert_eq!(get("Partial Paragraph"), "\u{201C}Intro\u{201D}");
        assert_eq!(get("Page Number"), "page 7");
        assert_eq!(get("Text Anchor Name"), "\u{201C}A\u{201D}");
        assert_eq!(expand("see <bogus /> x", &v), "see <bogus /> x");
    }

    #[test]
    fn marks_follow_edits() {
        let mut s = Story::with_text(StoryId(3), "Heading\nBody text.", Default::default());
        let a = s.insert_anchor(0, "Heading");
        assert_eq!(a >> 24, 3);
        s.insert_xref(13, CrossRef { target: a, format: "Page Number".into(), ..Default::default() });
        s.check().unwrap();
        assert_eq!(s.anchor_pos(a), Some(0));
        s.insert(0, "New ");
        assert_eq!(s.anchor_pos(a), Some(4));
        let x = s.text.find(XREF_MARK).unwrap();
        assert_eq!(s.xref_at(x).unwrap().target, a);
        s.delete(x..x + XREF_MARK.len_utf8());
        s.check().unwrap();
        assert!(s.xrefs.is_empty());
        s.delete(0..8);
        s.check().unwrap();
        assert!(s.anchors.is_empty());
    }
}
