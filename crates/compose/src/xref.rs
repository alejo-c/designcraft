//! Cross-reference resolution: the page each text anchor lands on (from composition) and the
//! generated text of each cross-reference.

use std::collections::HashMap;
use std::sync::Arc;

use designcraft_doc::{Document, SpreadRef, Story, StoryId, XREF_MARK, xref};
use designcraft_geom::Point;

use crate::ComposedStory;

/// Where a text anchor's marker lands: its absolute page, the caret point, and the top of the
/// line holding it, both in spread coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnchorPlace {
    pub page: usize,
    pub point: Point,
    pub line_top: Point,
}

/// One text anchor resolved from the composition.
#[derive(Debug)]
struct IndexedAnchor {
    place: AnchorPlace,
    name: String,
}

/// Page name and composed place of every text anchor in the document.
#[derive(Debug, Default)]
pub struct XrefIndex {
    anchors: HashMap<u64, IndexedAnchor>,
}

pub fn has_xrefs(st: &Story) -> bool {
    !st.xrefs.is_empty()
}

impl XrefIndex {
    /// `get` composes a story (normally through the cache). Stories that hold cross-references or
    /// running headers are composed without resolving them (their anchors' pages hardly move):
    /// through the cache they would build this index again, directly or by way of the running-header index.
    pub fn build(doc: &Document, get: &dyn Fn(StoryId) -> Arc<ComposedStory>) -> XrefIndex {
        let mut anchors = HashMap::new();
        for (sid, st) in &doc.stories {
            if st.anchors.is_empty() {
                continue;
            }
            let indexed = has_xrefs(st) || crate::vars::has_running(doc, st);
            let cs = if indexed { Arc::new(crate::compose_story(doc, *sid, &Default::default())) } else { get(*sid) };
            for a in &st.anchors {
                let Some(pos) = st.anchor_pos(a.id) else { continue };
                let Some(place) = anchor_place(doc, &cs, pos) else { continue };
                anchors.insert(a.id, IndexedAnchor { place, name: doc.page_name(place.page) });
            }
        }
        XrefIndex { anchors }
    }

    /// Page name of an anchor.
    pub fn page(&self, anchor: u64) -> Option<&str> {
        self.anchors.get(&anchor).map(|a| a.name.as_str())
    }

    /// Absolute page index of an anchor.
    pub fn page_index(&self, anchor: u64) -> Option<usize> {
        self.anchors.get(&anchor).map(|a| a.place.page)
    }

    /// Where an anchor's marker is composed: its page, caret point and line top.
    pub fn place(&self, anchor: u64) -> Option<AnchorPlace> {
        self.anchors.get(&anchor).map(|a| a.place)
    }
}

/// Where story byte `pos` lands in `cs`: its absolute page and spread-space point.
pub fn anchor_place(doc: &Document, cs: &ComposedStory, pos: usize) -> Option<AnchorPlace> {
    let (fi, x, baseline, ascent, _) = crate::caret(cs, pos)?;
    let ft = cs.frames.get(fi).filter(|ft| ft.range.contains(&pos))?;
    let item = doc.item(ft.frame)?;
    let loc = doc.find(ft.frame)?;
    let SpreadRef::Doc(si) = loc.spread else { return None };
    let base = doc.parent_xf(&loc) * doc.text_xf(item);
    let map = crate::PathTextMap::new(item, ft);
    let place = |point: Point| map.as_ref().map_or(point, |map| map.to_path(point));
    let point = base * place(Point::new(x, baseline));
    let line_top = base * place(Point::new(x, baseline - ascent));
    let pi = doc.spreads.get(si)?.page_at_x(point.x)?;
    let page = doc.first_page_of_spread(si).checked_add(pi)?;
    Some(AnchorPlace { page, point, line_top })
}

/// The text a cross-reference shows ("??" when its destination is gone, like an unresolved
/// reference in typesetting, unless the text was edited).
pub fn xref_text(doc: &Document, x: &xref::CrossRef, index: Option<&XrefIndex>) -> String {
    // Edited text stays as typed; its page numbers still follow the destination, or show the
    // page last shown when the destination is gone (as in InDesign).
    if let Some(t) = &x.text {
        let live = index.filter(|_| doc.xref_values(x.target).is_some()).and_then(|i| i.page(x.target));
        return xref::expand_edited(t, live.or(x.page.as_deref()).unwrap_or("?"));
    }
    let Some(mut v) = doc.xref_values(x.target) else { return "??".into() };
    v.page = index.and_then(|i| i.page(x.target)).unwrap_or("?").to_string();
    let def = doc.xref_format(&x.format).map_or("<fullPara />", |f| f.definition.as_str());
    xref::expand(def, &v)
}

/// Generated text of the cross-references in `range` of `story`, by byte of their mark.
pub fn texts_in(doc: &Document, story: &Story, range: std::ops::Range<usize>, index: Option<&XrefIndex>) -> HashMap<usize, String> {
    if story.xrefs.is_empty() {
        return HashMap::new();
    }
    let before = story.text[..range.start].matches(XREF_MARK).count();
    texts_after(doc, story, range, before, index)
}

/// [`texts_in`] for a caller that already knows `before`, the number of cross-reference marks in
/// `story` before `range`.
pub(crate) fn texts_after(
    doc: &Document,
    story: &Story,
    range: std::ops::Range<usize>,
    before: usize,
    index: Option<&XrefIndex>,
) -> HashMap<usize, String> {
    let mut out = HashMap::new();
    if story.xrefs.is_empty() {
        return out;
    }
    for (k, (i, _)) in story.text[range.clone()].match_indices(XREF_MARK).enumerate() {
        if let Some(x) = story.xrefs.get(before + k) {
            out.insert(range.start + i, xref_text(doc, x, index));
        }
    }
    out
}
