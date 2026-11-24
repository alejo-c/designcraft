//! Hyperlinks and Bookmarks panels (Window › Interactive).

use std::collections::{HashMap, HashSet};

use serde_json::{Value, json};

use crate::DesignApp;
use crate::app::file_dialogs::Bound;
use crate::theme::Tokens;

/// One row of the text anchor tree in display order (siblings keep `anchor.list` page/x/y order).
pub(crate) struct AnchorTreeRow {
    pub index: usize,
    pub id: u64,
    pub depth: usize,
    pub has_children: bool,
}

/// Flatten `anchor.list` into pre-order tree rows while preserving `anchor.list` order among siblings.
pub(crate) fn anchor_tree_rows(list: &[Value]) -> Vec<AnchorTreeRow> {
    let ids: HashSet<u64> = list.iter().filter_map(|a| a["id"].as_u64()).collect();
    let mut children: HashMap<Option<u64>, Vec<usize>> = HashMap::new();
    for (index, a) in list.iter().enumerate() {
        let parent = a["parent"].as_u64().filter(|p| ids.contains(p));
        children.entry(parent).or_default().push(index);
    }
    let mut rows = Vec::with_capacity(list.len());
    let mut visited = HashSet::with_capacity(list.len());
    fn walk(
        parent: Option<u64>,
        depth: usize,
        list: &[Value],
        children: &HashMap<Option<u64>, Vec<usize>>,
        visited: &mut HashSet<u64>,
        rows: &mut Vec<AnchorTreeRow>,
    ) {
        let Some(indices) = children.get(&parent) else { return };
        for &index in indices {
            let Some(id) = list[index]["id"].as_u64() else { continue };
            if !visited.insert(id) {
                continue;
            }
            let has_children = children.get(&Some(id)).is_some_and(|c| !c.is_empty());
            rows.push(AnchorTreeRow { index, id, depth, has_children });
            walk(Some(id), depth + 1, list, children, visited, rows);
        }
    }
    walk(None, 0, list, &children, &mut visited, &mut rows);
    // Defensive fallback if corrupt data ever left an unreachable cycle.
    for (index, a) in list.iter().enumerate() {
        if let Some(id) = a["id"].as_u64()
            && visited.insert(id)
        {
            rows.push(AnchorTreeRow { index, id, depth: 0, has_children: false });
        }
    }
    rows
}

/// Anchor `root_id` together with all of its descendants in `list` (invalid parent targets for `root_id`).
pub(crate) fn anchor_subtree_ids(list: &[Value], root_id: u64) -> HashSet<u64> {
    let mut children: HashMap<u64, Vec<u64>> = HashMap::new();
    for a in list {
        if let (Some(id), Some(parent)) = (a["id"].as_u64(), a["parent"].as_u64()) {
            children.entry(parent).or_default().push(id);
        }
    }
    let mut out = HashSet::from([root_id]);
    let mut stack = vec![root_id];
    while let Some(cur) = stack.pop() {
        if let Some(kids) = children.get(&cur) {
            for &k in kids {
                if out.insert(k) {
                    stack.push(k);
                }
            }
        }
    }
    out
}

fn collect_bookmarked_anchors(bookmarks: &[Value], out: &mut HashSet<u64>) {
    for b in bookmarks {
        if let Some(id) = b["anchor"].as_u64() {
            out.insert(id);
        }
        if let Some(children) = b["children"].as_array() {
            collect_bookmarked_anchors(children, out);
        }
    }
}

fn dest_text(lang: &str, d: &Value, anchors: &[Value]) -> String {
    match d["kind"].as_str() {
        Some("url") => d["value"].as_str().unwrap_or("").to_string(),
        Some("email") => format!("mailto:{}", d["value"].as_str().unwrap_or("")),
        Some("page") => format!("{} {}", crate::i18n::tr(lang, "Page"), d["value"].as_u64().unwrap_or(0).saturating_add(1)),
        Some("anchor") => {
            anchors.iter().find(|a| a["id"] == d["value"]).and_then(|a| a["name"].as_str()).unwrap_or("Missing text anchor").to_string()
        }
        _ => d.to_string(),
    }
}

pub fn hyperlinks(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let id = egui::Id::unique("new_hyperlink_url");
    let mut url: String = ui.data(|d| d.get_temp(id)).unwrap_or_else(|| "https://".into());
    let anchors = app.session.execute("anchor.list", &json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    let key = egui::Id::unique(("hyperlink_destination", app.session.active().map(|d| d.uid)));
    let (mut anchor_mode, mut anchor): (bool, Option<u64>) = ui.data(|d| d.get_temp(key)).unwrap_or_default();
    if !anchors.iter().any(|a| a["id"].as_u64() == anchor) {
        anchor = None;
    }
    let kind_label = crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Link to:"));
    craft_ui::ComboBox::from_id_salt(key.with("kind"))
        .selected_text(crate::i18n::tr(&app.ui.language, if anchor_mode { "Text Anchor" } else { "URL" }))
        .show_ui(ui, |ui| {
            if ui.selectable_label(!anchor_mode, crate::i18n::tr(&app.ui.language, "URL")).clicked() {
                anchor_mode = false;
            }
            if ui.selectable_label(anchor_mode, crate::i18n::tr(&app.ui.language, "Text Anchor")).clicked() {
                anchor_mode = true;
            }
        })
        .response
        .labelled_by(kind_label.id);
    let has_target = app.session.active().is_some_and(|d| d.selection.text.is_some_and(|t| !t.is_caret()) || !d.selection.items.is_empty());
    let destination_label = crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Destination:"));
    ui.horizontal(|ui| {
        if anchor_mode {
            let name = anchors.iter().find(|a| a["id"].as_u64() == anchor).and_then(|a| a["name"].as_str()).unwrap_or("Select text anchor");
            craft_ui::ComboBox::from_id_salt(key.with("anchor"))
                .selected_text(name)
                .width(170.0)
                .show_ui(ui, |ui| {
                    for row in anchor_tree_rows(&anchors) {
                        let a = &anchors[row.index];
                        let name = a["name"].as_str().unwrap_or("");
                        let label = if row.depth == 0 { name.to_string() } else { format!("{}{name}", "  ".repeat(row.depth)) };
                        if ui.selectable_label(a["id"].as_u64() == anchor, label).clicked() {
                            anchor = a["id"].as_u64();
                        }
                    }
                })
                .response
                .labelled_by(destination_label.id);
        } else {
            ui.add(egui::TextEdit::singleline(&mut url).desired_width(170.0))
                .labelled_by(destination_label.id)
                .on_hover_text(crate::i18n::tr(&app.ui.language, "URL"));
        }
        if ui
            .add_enabled(
                has_target && (!anchor_mode || anchor.is_some()),
                egui::Button::new(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "New"))),
            )
            .on_hover_ui(|ui| {
                crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Hyperlink from the selected text or frames"));
            })
            .clicked()
        {
            let p = if anchor_mode {
                json!({"anchor": anchor})
            } else if let Some(m) = url.strip_prefix("mailto:") {
                json!({"email": m})
            } else {
                json!({"url": url})
            };
            if let Err(e) = app.run("hyperlink.create", p) {
                app.status_error("Hyperlinks", e);
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(id, url));
    ui.data_mut(|d| d.insert_temp(key, (anchor_mode, anchor)));
    ui.separator();
    let list = app.session.execute("hyperlink.list", &json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    if list.is_empty() {
        crate::rtl::label(
            ui,
            crate::rtl::widget(
                ui,
                egui::RichText::new(crate::i18n::tr(&app.ui.language, "Select text or a frame, choose a destination and click New."))
                    .size(11.0)
                    .color(t.text_dim),
            ),
        );
    }
    for h in list {
        let hid = h["id"].clone();
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Delete"))).clicked() {
                    let _ = app.run("hyperlink.delete", json!({"id": hid}));
                }
                let name = h["name"].as_str().unwrap_or("");
                if ui
                    .add(egui::Button::selectable(false, name).truncate())
                    .on_hover_text(format!("{name}\n{}", dest_text(&app.ui.language, &h["dest"], &anchors)))
                    .clicked()
                {
                    let _ = app.run("hyperlink.goToSource", json!({"id": hid}));
                }
            });
        });
    }
}

#[cfg(test)]
mod anchor_link_tests {
    use super::*;
    use egui_kittest::{Harness, kittest::Queryable};

    #[test]
    fn panel_creates_a_link_to_the_selected_text_anchor() {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({})).unwrap();
        let f = app.run("frame.create", json!({"rect": [36,36,300,200], "content": "text", "text": "Destination source"})).unwrap();
        app.run("text.select", json!({"story": f["story"], "anchor": 0, "focus": 0})).unwrap();
        let a = app.run("anchor.create", json!({"name": "Introduction"})).unwrap();
        app.run("text.select", json!({"story": f["story"], "anchor": 15, "focus": 21})).unwrap();
        let mut h = Harness::builder().with_size(egui::vec2(500.0, 500.0)).build_ui_state(|ui, app| hyperlinks(app, ui), app);
        h.run_steps(3);
        h.get_by_value("URL").click();
        h.run_steps(3);
        h.get_by_label("Text Anchor").click();
        h.run_steps(3);
        assert!(h.state().session.doc().unwrap().doc.hyperlinks.is_empty());
        h.get_by_value("Select text anchor").click();
        h.run_steps(3);
        h.get_by_label("Introduction").click();
        h.run_steps(3);
        h.get_by_label("New").click();
        h.run_steps(3);
        let link = &h.state().session.doc().unwrap().doc.hyperlinks[0];
        assert_eq!(link.dest, designcraft_doc::HyperlinkDest::Anchor(a["id"].as_u64().unwrap()));
        assert_eq!(dest_text("en", &serde_json::to_value(&link.dest).unwrap(), &[json!({"id": a["id"], "name": "Introduction"})]), "Introduction");
        h.state_mut().run("edit.undo", json!({})).unwrap();
        assert!(h.state().session.doc().unwrap().doc.hyperlinks.is_empty());
    }
}

pub fn bookmarks(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let has_text = app.session.active().is_some_and(|d| d.selection.text.is_some_and(|t| t.cell.is_none()));
    if ui.add_enabled(has_text, egui::Button::new(crate::i18n::tr(&app.ui.language, "New Bookmark from Selected Text"))).clicked()
        && let Err(e) = app.run("bookmark.add", json!({}))
    {
        app.status_error("Bookmarks", e);
    }
    ui.separator();
    let list = app.session.execute("bookmark.list", &json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    if list.is_empty() {
        crate::rtl::label(
            ui,
            crate::rtl::widget(
                ui,
                egui::RichText::new(crate::i18n::tr(&app.ui.language, "No bookmarks. They become the PDF outline.")).size(11.0).color(t.text_dim),
            ),
        );
    }
    let doc_uid = app.session.active().map(|d| d.uid);
    let draft_key = egui::Id::unique(("bookmark_rename_draft", doc_uid));
    let mut active_draft: Option<(Vec<usize>, String)> = ui.data(|d| d.get_temp(draft_key));
    let has_hierarchy = list.iter().any(|b| b["children"].as_array().is_some_and(|c| !c.is_empty()));

    struct BookmarkTreeCtx {
        has_hierarchy: bool,
        doc_uid: Option<u64>,
    }

    fn render_bookmarks(
        app: &mut DesignApp,
        ui: &mut egui::Ui,
        items: &[Value],
        prefix: &[usize],
        ctx: &BookmarkTreeCtx,
        active_draft: &mut Option<(Vec<usize>, String)>,
    ) {
        let depth = prefix.len();
        let t = Tokens::get(ui.ctx());
        for (i, b) in items.iter().enumerate() {
            let mut path = prefix.to_vec();
            path.push(i);
            let page = b["page"].as_u64().and_then(|p| usize::try_from(p).ok());
            let children = b["children"].as_array().map(Vec::as_slice).unwrap_or(&[]);
            let has_children = !children.is_empty();
            let open_key = egui::Id::unique(("bookmark_tree_open", ctx.doc_uid, &path));
            let mut open: bool = ui.data(|d| d.get_temp(open_key)).unwrap_or(true);
            let name = b["name"].as_str().unwrap_or("");
            ui.push_id(&path, |ui| {
                ui.horizontal(|ui| {
                    if depth > 0 {
                        ui.add_space((depth as f32 * 14.0).min(84.0));
                    }
                    if has_children {
                        let toggle_tip = format!(
                            "{} {name}",
                            crate::i18n::tr(&app.ui.language, if open { "Collapse" } else { "Expand" })
                        );
                        let toggle = ui
                            .add(egui::Button::new("").frame(false).min_size(egui::vec2(18.0, 20.0)))
                            .accessible_name(&toggle_tip)
                            .on_hover_text(&toggle_tip);
                        crate::icons::paint(
                            ui.painter(),
                            toggle.rect.shrink(3.0),
                            if open { "chevron-down" } else { "chevron-right" },
                            t.icon,
                        );
                        if toggle.clicked() {
                            open = !open;
                            ui.data_mut(|d| d.insert_temp(open_key, open));
                        }
                    } else if ctx.has_hierarchy {
                        ui.add_space(18.0);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Delete"))).clicked() {
                            let _ = app.run("bookmark.delete", json!({"path": path}));
                            *active_draft = None;
                        }
                        if ui.small_button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Rename"))).clicked() {
                            *active_draft = Some((path.clone(), name.to_string()));
                        }
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            if ui
                                .add_enabled(page.is_some(), egui::Button::selectable(false, name).truncate())
                                .on_hover_text(format!(
                                    "{name}\n{}",
                                    page.map(|p| format!("{} {}", crate::i18n::tr(&app.ui.language, "Page"), app.session.page_label(p)))
                                        .unwrap_or_else(|| "Unresolved text anchor".into())
                                ))
                                .clicked()
                                && let Some(page) = page
                            {
                                if let Some(anchor) = b["anchor"].as_u64() {
                                    go_to_anchor(app, anchor);
                                } else {
                                    crate::canvas::go_to_page(app, page);
                                }
                            }
                        });
                    });
                });
                if active_draft.as_ref().is_some_and(|(p, _)| *p == path) {
                    let mut draft_name = active_draft.as_ref().map(|(_, n)| n.clone()).unwrap_or_default();
                    let mut close = false;
                    let label = crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Name"));
                    ui.add(egui::TextEdit::singleline(&mut draft_name).desired_width(170.0)).labelled_by(label.id);
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!draft_name.trim().is_empty(), egui::Button::new(crate::i18n::tr(&app.ui.language, "Save"))).clicked() {
                            match app.run("bookmark.rename", json!({"path": path, "name": draft_name})) {
                                Ok(_) => close = true,
                                Err(e) => app.status_error("Bookmarks", e),
                            }
                        }
                        close |= ui.button(crate::i18n::tr(&app.ui.language, "Cancel")).clicked();
                    });
                    if close {
                        *active_draft = None;
                    } else {
                        *active_draft = Some((path.clone(), draft_name));
                    }
                }
            });
            if has_children && open {
                render_bookmarks(app, ui, children, &path, ctx, active_draft);
            }
        }
    }

    let ctx = BookmarkTreeCtx { has_hierarchy, doc_uid };
    render_bookmarks(app, ui, &list, &[], &ctx, &mut active_draft);
    ui.data_mut(|d| match active_draft {
        Some(draft) => {
            d.insert_temp(draft_key, draft);
        }
        None => {
            d.remove::<(Vec<usize>, String)>(draft_key);
        }
    });
}

#[cfg(test)]
mod bookmark_name_tests {
    use super::*;
    use egui_kittest::{Harness, kittest::Queryable};

    #[test]
    fn panel_renames_bookmarks_with_undo() {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({})).unwrap();
        let f = app.run("frame.create", json!({"rect": [36,36,300,200], "content": "text", "text": "Original"})).unwrap();
        app.run("text.select", json!({"story": f["story"], "anchor": 0, "focus": 8})).unwrap();
        app.run("bookmark.add", json!({"name": "Original"})).unwrap();
        let mut h = Harness::builder().with_size(egui::vec2(500.0, 500.0)).build_ui_state(|ui, app| bookmarks(app, ui), app);
        h.run_steps(3);
        h.get_by_label("Rename").click();
        h.run_steps(3);
        h.get_by_role(egui::accesskit::Role::TextInput).click();
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        h.event(egui::Event::Text("Introduction".into()));
        h.run_steps(3);
        h.get_by_label("Save").click();
        h.run_steps(3);
        assert_eq!(h.state().session.doc().unwrap().doc.bookmarks[0].name, "Introduction");
        h.state_mut().run("edit.undo", json!({})).unwrap();
        assert_eq!(h.state().session.doc().unwrap().doc.bookmarks[0].name, "Original");
    }
}

fn go_to_anchor(app: &mut DesignApp, id: u64) {
    let destination = (|| {
        let doc = &app.session.active()?.doc;
        let (story, pos) = doc.find_anchor(id)?;
        let cs = app.session.cache.get(doc, story, None);
        let (fi, x, baseline, asc, desc) = designcraft_compose::caret(&cs, pos)?;
        let ft = cs.frames.get(fi).filter(|ft| ft.range.contains(&pos))?;
        let it = doc.item(ft.frame)?;
        let loc = doc.find(ft.frame)?;
        let designcraft_doc::SpreadRef::Doc(_) = loc.spread else { return None };
        let xf = doc.parent_xf(&loc) * doc.text_xf(it);
        let on = |p| designcraft_compose::PathTextMap::new(it, ft).map_or(p, |map| map.to_path(p));
        let p0 = xf * on(designcraft_geom::Point::new(x, baseline - asc));
        let p1 = xf * on(designcraft_geom::Point::new(x, baseline + desc));
        let rect = designcraft_geom::Rect::new(p0.x.min(p1.x), p0.y.min(p1.y), p0.x.max(p1.x), p0.y.max(p1.y));
        Some((story, pos, loc.spread, rect))
    })();
    let Some((story, pos, spread, rect)) = destination else { return };
    if app.session.active().is_some_and(|st| st.editing_parents)
        && let Err(e) = app.run("layout.parents.edit", json!({"on": false}))
    {
        app.status_error("Text Anchors", e);
        return;
    }
    if let Err(e) = app.run("text.select", json!({"story": story.0, "anchor": pos, "focus": pos})) {
        app.status_error("Text Anchors", e);
        return;
    }
    if let Some(view) = app.view_mut() {
        view.fitted = true;
    }
    crate::canvas::go_to_spread_rect(app, spread, rect);
}

fn text_anchor_target(app: &mut DesignApp) -> Option<(designcraft_doc::TextSel, String)> {
    let st = app.session.active()?;
    if let Some(text) = st.selection.text {
        if text.cell.is_some() {
            return None;
        }
        let name = st.doc.story(text.story)?.text.get(text.range())?.to_string();
        return Some((text, name));
    }
    let (frame, story) = st.selection.items.iter().find_map(|id| Some((*id, st.doc.item(*id)?.text_frame()?.story)))?;
    let composed = app.session.cache.get(&st.doc, story, None);
    let pos = composed.frames.iter().find(|ft| ft.frame == frame)?.range.start;
    Some((designcraft_doc::TextSel { frame: Some(frame), ..designcraft_doc::TextSel::caret(story, pos) }, String::new()))
}

pub fn text_anchors(app: &mut DesignApp, ui: &mut egui::Ui) {
    let list = app.session.execute("anchor.list", &json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    let bookmarks = app.session.execute("bookmark.list", &json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    let mut bookmarked = HashSet::new();
    collect_bookmarked_anchors(&bookmarks, &mut bookmarked);
    let doc_uid = app.session.active().map(|st| st.uid);
    let target = text_anchor_target(app);
    let tree_rows = anchor_tree_rows(&list);
    let has_hierarchy = tree_rows.iter().any(|r| r.has_children);
    let ids: HashSet<u64> = list.iter().filter_map(|a| a["id"].as_u64()).collect();
    let mut children_map: HashMap<Option<u64>, Vec<usize>> = HashMap::new();
    for (index, a) in list.iter().enumerate() {
        let parent = a["parent"].as_u64().filter(|p| ids.contains(p));
        children_map.entry(parent).or_default().push(index);
    }

    struct AnchorTreeCtx<'a> {
        list: &'a [Value],
        tree_rows: &'a [AnchorTreeRow],
        children_map: &'a HashMap<Option<u64>, Vec<usize>>,
        bookmarked: &'a HashSet<u64>,
        target: &'a Option<(designcraft_doc::TextSel, String)>,
        doc_uid: Option<u64>,
        has_hierarchy: bool,
    }

    fn render_anchors(app: &mut DesignApp, ui: &mut egui::Ui, parent: Option<u64>, depth: usize, ctx: &AnchorTreeCtx<'_>, visited: &mut HashSet<u64>) {
        let Some(indices) = ctx.children_map.get(&parent) else { return };
        let t = Tokens::get(ui.ctx());
        for &index in indices {
            let a = &ctx.list[index];
            let Some(id) = a["id"].as_u64() else { continue };
            if !visited.insert(id) {
                continue;
            }
            let name = a["name"].as_str().unwrap_or("");
            let has_children = ctx.children_map.get(&Some(id)).is_some_and(|c| !c.is_empty());
            let open_key = egui::Id::unique(("anchor_tree_open", ctx.doc_uid, id));
            let mut open: bool = ui.data(|d| d.get_temp(open_key)).unwrap_or(true);
            let is_bookmarked = ctx.bookmarked.contains(&id);
            ui.push_id(id, |ui| {
                ui.horizontal(|ui| {
                    if depth > 0 {
                        ui.add_space((depth as f32 * 14.0).min(84.0));
                    }
                    if has_children {
                        let toggle_tip = format!(
                            "{} {name}",
                            crate::i18n::tr(&app.ui.language, if open { "Collapse" } else { "Expand" })
                        );
                        let toggle = ui
                            .add(egui::Button::new("").frame(false).min_size(egui::vec2(18.0, 24.0)))
                            .accessible_name(&toggle_tip)
                            .on_hover_text(&toggle_tip);
                        crate::icons::paint(
                            ui.painter(),
                            toggle.rect.shrink(3.0),
                            if open { "chevron-down" } else { "chevron-right" },
                            t.icon,
                        );
                        if toggle.clicked() {
                            open = !open;
                            ui.data_mut(|d| d.insert_temp(open_key, open));
                        }
                    } else if ctx.has_hierarchy {
                        ui.add_space(18.0);
                    }
                    let width = (ui.available_width() - 3.0 * (24.0 + ui.spacing().item_spacing.x)).max(0.0);
                    let resp = ui
                        .add_enabled_ui(!a["page"].is_null(), |ui| ui.add_sized([width, 24.0], egui::Button::selectable(false, name).truncate()))
                        .inner
                        .on_hover_text(name);
                    if resp.clicked() {
                        go_to_anchor(app, id);
                    }
                    resp.context_menu(|ui| {
                        let lang = app.ui.language.clone();
                        crate::menus::menu_button(ui, crate::rtl::widget(ui, crate::i18n::tr(&lang, "Move to Parent")), |ui| {
                            let forbidden = anchor_subtree_ids(ctx.list, id);
                            let mut any = false;
                            if parent.is_some() {
                                any = true;
                                if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&lang, "[No Parent]"))).clicked() {
                                    if let Err(e) = app.run("anchor.setParent", json!({"id": id, "parent": Value::Null})) {
                                        app.status_error("Text Anchors", e);
                                    }
                                    ui.close();
                                }
                            }
                            for cand in ctx.tree_rows {
                                if forbidden.contains(&cand.id) || parent == Some(cand.id) {
                                    continue;
                                }
                                any = true;
                                let cand_name = ctx.list[cand.index]["name"].as_str().unwrap_or("");
                                let label = if cand.depth == 0 {
                                    cand_name.to_string()
                                } else {
                                    format!("{}{cand_name}", "  ".repeat(cand.depth))
                                };
                                if ui.button(label).clicked() {
                                    if let Err(e) = app.run("anchor.setParent", json!({"id": id, "parent": cand.id})) {
                                        app.status_error("Text Anchors", e);
                                    }
                                    ui.close();
                                }
                            }
                            if !any {
                                crate::rtl::label(
                                    ui,
                                    egui::RichText::new(crate::i18n::tr(&lang, "No other text anchors")).size(11.0).color(t.text_dim),
                                );
                            }
                        });
                        if ui
                            .add_enabled(
                                ctx.target.is_some(),
                                egui::Button::new(crate::rtl::widget(ui, crate::i18n::tr(&lang, "New Child Text Anchor…"))),
                            )
                            .clicked()
                            && let Some((selection, default_name)) = ctx.target.clone()
                        {
                            app.ui.dialog = Some(crate::dialogs::Dialog::new(
                                "textAnchor",
                                json!({
                                    "name": default_name,
                                    "parent": id,
                                    "selection": selection,
                                    "uid": ctx.doc_uid,
                                    "focusName": true
                                }),
                            ));
                            ui.close();
                        }
                        ui.separator();
                        let bm_label = if is_bookmarked { "Remove Bookmark" } else { "Add Bookmark" };
                        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&lang, bm_label))).clicked() {
                            let result = if is_bookmarked {
                                app.run("bookmark.delete", json!({"anchor": id}))
                            } else {
                                app.run("bookmark.add", json!({"anchor": id}))
                            };
                            if let Err(e) = result {
                                app.status_error("Text Anchors", e);
                            }
                            ui.close();
                        }
                        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&lang, "Rename…"))).clicked() {
                            app.ui.dialog = Some(crate::dialogs::Dialog::new(
                                "textAnchor",
                                json!({
                                    "id": id,
                                    "name": name,
                                    "parent": parent,
                                    "uid": ctx.doc_uid,
                                    "focusName": true
                                }),
                            ));
                            ui.close();
                        }
                        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&lang, "Delete"))).clicked() {
                            if let Err(e) = app.run("anchor.delete", json!({"id": id, "removeBookmarks": true})) {
                                app.status_error("Text Anchors", e);
                            }
                            ui.close();
                        }
                    });
                    if crate::icons::button(ui, "panel-bookmarks", 24.0, is_bookmarked, crate::i18n::tr(&app.ui.language, "Bookmark")).clicked() {
                        let result = if is_bookmarked {
                            app.run("bookmark.delete", json!({"anchor": id}))
                        } else {
                            app.run("bookmark.add", json!({"anchor": id}))
                        };
                        if let Err(e) = result {
                            app.status_error("Text Anchors", e);
                        }
                    }
                    if crate::icons::button(ui, "tool-pencil", 24.0, false, crate::i18n::tr(&app.ui.language, "Rename")).clicked() {
                        app.ui.dialog = Some(crate::dialogs::Dialog::new(
                            "textAnchor",
                            json!({
                                "id": id, "name": name, "parent": parent, "uid": ctx.doc_uid, "focusName": true
                            }),
                        ));
                    }
                    if crate::icons::button(ui, "trash", 24.0, false, crate::i18n::tr(&app.ui.language, "Delete")).clicked()
                        && let Err(e) = app.run("anchor.delete", json!({"id": id, "removeBookmarks": true}))
                    {
                        app.status_error("Text Anchors", e);
                    }
                });
            });
            if has_children && open {
                render_anchors(app, ui, Some(id), depth + 1, ctx, visited);
            }
        }
    }

    let list_height = (ui.available_height() - 24.0 - ui.spacing().item_spacing.y).max(0.0);
    egui::ScrollArea::vertical().id_salt("text_anchors_list").max_height(list_height).min_scrolled_height(0.0).auto_shrink([false, false]).show(
        ui,
        |ui| {
            let ctx = AnchorTreeCtx {
                list: &list,
                tree_rows: &tree_rows,
                children_map: &children_map,
                bookmarked: &bookmarked,
                target: &target,
                doc_uid,
                has_hierarchy,
            };
            let mut visited = HashSet::with_capacity(list.len());
            render_anchors(app, ui, None, 0, &ctx, &mut visited);
        },
    );
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let label = crate::i18n::tr(&app.ui.language, "New Text Anchor");
        let add =
            ui.add_enabled(target.is_some(), egui::Button::new("").min_size(egui::vec2(24.0, 24.0))).accessible_name(label).on_hover_text(label);
        let t = Tokens::get(ui.ctx());
        crate::icons::paint(ui.painter(), add.rect.shrink(4.0), "plus", if add.enabled() { t.icon } else { t.text_disabled });
        if add.clicked()
            && let Some((selection, name)) = target
        {
            app.ui.dialog = Some(crate::dialogs::Dialog::new(
                "textAnchor",
                json!({
                    "name": name, "parent": Value::Null, "selection": selection, "uid": doc_uid, "focusName": true
                }),
            ));
        }
    });
}

#[cfg(test)]
mod text_anchor_panel_tests {
    use super::*;
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };

    #[test]
    fn long_anchor_names_leave_delete_visible_and_deletion_is_undoable() {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({})).unwrap();
        let f = app.run("frame.create", json!({"rect": [36,36,300,200], "content": "text", "text": "Heading"})).unwrap();
        app.run("text.select", json!({"story": f["story"], "anchor": 0, "focus": 0})).unwrap();
        let name = "An anchor name that is much too long to fit inside the narrow Text Anchors panel";
        let anchor = app.run("anchor.create", json!({"name": name})).unwrap();
        app.run("bookmark.add", json!({"anchor": anchor["id"]})).unwrap();
        let mut h = Harness::builder().with_size(egui::vec2(280.0, 500.0)).build_ui_state(|ui, app| text_anchors(app, ui), app);
        h.run_steps(3);
        let delete = h.get_by_label("Delete");
        assert!(delete.rect().right() <= 280.0, "{:?}", delete.rect());
        assert!(h.output().shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(text) if text.galley.job.text == name && text.galley.elided)));
        if let Some(path) = std::env::var_os("DESIGNCRAFT_ANCHOR_NARROW_SHOT") {
            h.render().unwrap().save(path).unwrap();
        }
        let delete = h.get_by_label("Delete");
        delete.click();
        h.run_steps(3);
        assert!(h.state().session.doc().unwrap().doc.stories.values().all(|st| st.anchors.is_empty()));
        assert!(h.state().session.doc().unwrap().doc.bookmarks.is_empty());
        h.state_mut().run("edit.undo", json!({})).unwrap();
        assert_eq!(h.state().session.doc().unwrap().doc.stories.values().next().unwrap().anchors[0].name, name);
        assert_eq!(h.state().session.doc().unwrap().doc.bookmarks[0].anchor, anchor["id"].as_u64());
    }

    #[test]
    fn anchor_and_bookmark_clicks_select_and_centre_the_anchor() {
        for (panel_name, panel) in [("Bookmarks", bookmarks as fn(&mut DesignApp, &mut egui::Ui)), ("Text Anchors", text_anchors)] {
            let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
            app.run("file.new", json!({"pages": 2, "facingPages": false})).unwrap();
            let spread = app.session.doc().unwrap().doc.page_loc(1).unwrap().0;
            let f = app
                .run("frame.create", json!({"spread": spread, "rect": [72,500,372,700], "content": "text", "text": "First line\nDestination"}))
                .unwrap();
            let pos = "First line\n".len();
            app.run("text.select", json!({"story": f["story"], "anchor": pos, "focus": pos})).unwrap();
            let anchor = app.run("anchor.create", json!({"name": "Destination"})).unwrap();
            app.run("bookmark.add", json!({"anchor": anchor["id"]})).unwrap();
            app.run("layout.parents.edit", json!({"on": true})).unwrap();
            assert_eq!(app.session.execute("bookmark.list", &json!({})).unwrap()[0]["page"], 1);
            let d = &app.session.doc().unwrap().doc;
            let (sid, at) = d.find_anchor(anchor["id"].as_u64().unwrap()).unwrap();
            let composed = app.session.cache.get(d, sid, None);
            let (fi, ..) = designcraft_compose::caret(&composed, at).unwrap();
            assert!(composed.frames[fi].range.contains(&at), "{panel_name}: anchor {at}, frame range {:?}", composed.frames[fi].range);
            app.canvas_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 400.0)));
            let view = app.view_mut().unwrap();
            view.zoom = 2.0;
            view.fitted = true;
            let mut h = Harness::builder().with_size(egui::vec2(280.0, 500.0)).build_ui_state(|ui, app| panel(app, ui), app);
            h.run_steps(3);
            assert!(!h.get_by_label("Destination").accesskit_node().is_disabled(), "{panel_name}");
            h.get_by_label("Destination").click();
            h.run_steps(3);
            let app = h.state();
            assert!(!app.session.doc().unwrap().editing_parents, "{panel_name}");
            let ts = app.session.doc().unwrap().selection.text.expect("bookmark selects its text destination");
            assert_eq!(ts.story.0, f["story"].as_u64().unwrap());
            assert_eq!((ts.anchor, ts.focus), (pos, pos));
            let doc = &app.session.doc().unwrap().doc;
            let cs = app.session.cache.get(doc, ts.story, None);
            let (fi, x, baseline, asc, desc) = designcraft_compose::caret(&cs, pos).unwrap();
            let it = doc.item(cs.frames[fi].frame).unwrap();
            let p = doc.text_xf(it) * designcraft_geom::Point::new(x, baseline + (desc - asc) / 2.0);
            let layout = designcraft_tools::CanvasLayout::new(doc, false);
            let p = layout.slot(designcraft_doc::SpreadRef::Doc(spread)).unwrap().xf * p;
            let view = app.view().unwrap();
            assert_eq!(view.zoom, 2.0);
            assert!((view.origin.y + 100.0 - p.y).abs() < 1e-6);
        }
    }

    #[test]
    fn long_page_bookmark_names_leave_delete_visible_and_keep_page_navigation() {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({"pages": 2, "facingPages": false})).unwrap();
        let name = "A page bookmark with a name that extends beyond the narrow Bookmarks panel";
        app.run("bookmark.add", json!({"name": name, "page": 2})).unwrap();
        app.canvas_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 400.0)));
        app.view_mut().unwrap().fitted = true;
        let mut h = Harness::builder().with_size(egui::vec2(280.0, 500.0)).build_ui_state(|ui, app| bookmarks(app, ui), app);
        h.run_steps(3);
        assert!(h.get_by_label("Delete").rect().right() <= 280.0);
        h.get_by_label(name).click();
        h.run_steps(3);
        let app = h.state();
        assert!(app.session.doc().unwrap().selection.text.is_none());
        let doc = &app.session.doc().unwrap().doc;
        let layout = designcraft_tools::CanvasLayout::new(doc, false);
        let centre = layout.page_rect(doc, 1).unwrap().center();
        let view = app.view().unwrap();
        assert!((view.origin.y + 200.0 / view.zoom - centre.y).abs() < 1e-6);
        h.get_by_label("Delete").click();
        h.run_steps(3);
        assert!(h.state().session.doc().unwrap().doc.bookmarks.is_empty());
    }

    #[test]
    fn panel_creates_a_text_destination_without_an_anchored_object() {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({})).unwrap();
        let f = app.run("frame.create", json!({"rect": [36,36,300,200], "content": "text", "text": "Heading"})).unwrap();
        app.run("text.select", json!({"story": f["story"], "anchor": 0, "focus": 7})).unwrap();
        let mut h = Harness::builder()
            .with_size(egui::vec2(500.0, 500.0))
            .with_context_setup(|ctx| {
                crate::theme::install_fonts(ctx, "en");
            })
            .build_ui_state(
                |ui, app| {
                    text_anchors(app, ui);
                    crate::dialogs::show(app, ui.ctx());
                },
                app,
            );
        h.run_steps(3);
        assert!(h.query_by_role(egui::accesskit::Role::TextInput).is_none());
        h.get_by_label("New Text Anchor").click();
        h.run_steps(3);
        assert_eq!(h.get_by_role(egui::accesskit::Role::TextInput).value().as_deref(), Some("Heading"));
        assert!(h.get_by_role(egui::accesskit::Role::TextInput).is_focused());
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        h.event(egui::Event::Text("Introduction".into()));
        h.key_press(egui::Key::Enter);
        h.run_steps(3);
        let st = h.state().session.doc().unwrap().doc.story(designcraft_doc::StoryId(f["story"].as_u64().unwrap())).unwrap();
        assert_eq!(st.anchors[0].name, "Introduction");
        assert!(st.objects.is_empty());
        assert_eq!(st.text.replace(designcraft_doc::ANCHOR_MARK, ""), "Heading");
        h.get_by_label("Bookmark").click();
        h.run_steps(3);
        let d = &h.state().session.doc().unwrap().doc;
        assert_eq!(d.bookmarks[0].anchor, Some(d.stories.values().next().unwrap().anchors[0].id));
        if let Some(path) = std::env::var_os("DESIGNCRAFT_ANCHOR_PANEL_SHOT") {
            h.render().unwrap().save(path).unwrap();
        }
        h.get_by_label("Bookmark").click();
        h.run_steps(3);
        assert!(h.state().session.doc().unwrap().doc.bookmarks.is_empty());
    }

    #[test]
    fn frame_selection_opens_an_empty_name_dialog_and_other_selections_disable_add() {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({})).unwrap();
        let frame = app.run("frame.create", json!({"rect": [36,36,300,200], "content": "text", "text": "Heading"})).unwrap();
        let shape = app.run("frame.create", json!({"rect": [36,250,300,350]})).unwrap();
        let mut h = Harness::builder()
            .with_size(egui::vec2(500.0, 500.0))
            .with_context_setup(|ctx| {
                crate::theme::install_fonts(ctx, "en");
            })
            .build_ui_state(
                |ui, app| {
                    text_anchors(app, ui);
                    crate::dialogs::show(app, ui.ctx());
                },
                app,
            );
        for ids in [json!([]), json!([shape["id"]])] {
            h.state_mut().run("selection.set", json!({"ids": ids})).unwrap();
            h.run_steps(3);
            assert!(h.get_by_label("New Text Anchor").accesskit_node().is_disabled());
        }
        h.state_mut().run("selection.set", json!({"ids": [frame["id"]]})).unwrap();
        h.run_steps(3);
        assert!(!h.get_by_label("New Text Anchor").accesskit_node().is_disabled());
        h.get_by_label("New Text Anchor").focus();
        h.run_steps(3);
        h.key_press(egui::Key::Space);
        h.run_steps(3);
        assert_eq!(h.state().ui.dialog.as_ref().unwrap().s("name"), "");
        assert!(h.get_by_role(egui::accesskit::Role::TextInput).is_focused());
        h.key_press(egui::Key::Escape);
        h.run_steps(3);
        assert!(h.state().ui.dialog.is_none());
        assert!(h.state().session.doc().unwrap().doc.stories.values().all(|story| story.anchors.is_empty()));
        h.get_by_label("New Text Anchor").click();
        h.run_steps(3);
        h.event(egui::Event::Text("123".into()));
        h.key_press(egui::Key::Enter);
        h.run_steps(3);
        let doc = &h.state().session.doc().unwrap().doc;
        let story = doc.story(designcraft_doc::StoryId(frame["story"].as_u64().unwrap())).unwrap();
        assert_eq!(story.anchors[0].name, "123");
        assert_eq!(doc.find_anchor(story.anchors[0].id).unwrap().1, 0);
    }

    #[test]
    fn rename_uses_a_dialog_and_keeps_bookmark_identity() {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({})).unwrap();
        let frame = app.run("frame.create", json!({"rect": [36,36,300,200], "content": "text", "text": "Heading"})).unwrap();
        app.run("text.select", json!({"story": frame["story"], "anchor": 0, "focus": 0})).unwrap();
        let anchor = app.run("anchor.create", json!({"name": "Heading"})).unwrap();
        app.run("bookmark.add", json!({"anchor": anchor["id"]})).unwrap();
        let mut h = Harness::builder()
            .with_size(egui::vec2(500.0, 500.0))
            .with_context_setup(|ctx| {
                crate::theme::install_fonts(ctx, "en");
            })
            .build_ui_state(
                |ui, app| {
                    text_anchors(app, ui);
                    crate::dialogs::show(app, ui.ctx());
                },
                app,
            );
        h.run_steps(3);
        let name = h.get_by_label("Heading").rect();
        let bookmark = h.get_by_label("Bookmark").rect();
        let rename = h.get_by_label("Rename").rect();
        let delete = h.get_by_label("Delete").rect();
        assert!(name.right() <= bookmark.left() && bookmark.right() <= rename.left() && rename.right() <= delete.left());
        assert!(name.y_range().contains(bookmark.center().y) && name.y_range().contains(rename.center().y));
        h.get_by_label("Rename").click();
        h.run_steps(3);
        assert_eq!(h.get_by_role(egui::accesskit::Role::TextInput).value().as_deref(), Some("Heading"));
        assert!(h.get_by_role(egui::accesskit::Role::TextInput).is_focused());
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        h.event(egui::Event::Text("Renamed".into()));
        h.key_press(egui::Key::Enter);
        h.run_steps(3);
        assert!(h.state().ui.dialog.is_none());
        let doc = &h.state().session.doc().unwrap().doc;
        assert_eq!(doc.story(designcraft_doc::StoryId(frame["story"].as_u64().unwrap())).unwrap().anchors[0].name, "Renamed");
        assert_eq!(doc.bookmarks[0].anchor, anchor["id"].as_u64());
    }

    #[test]
    fn add_stays_at_the_bottom_outside_the_scrolling_anchor_list() {
        for height in [80.0, 200.0] {
            let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
            app.run("file.new", json!({})).unwrap();
            let frame = app.run("frame.create", json!({"rect": [36,36,300,200], "content": "text", "text": "Heading"})).unwrap();
            app.run("text.select", json!({"story": frame["story"], "anchor": 0, "focus": 0})).unwrap();
            let mut h = Harness::builder().with_size(egui::vec2(280.0, height)).build_ui_state(|ui, app| text_anchors(app, ui), app);
            h.run_steps(3);
            let footer = h.get_by_label("New Text Anchor").rect();
            assert!(footer.bottom() <= height);
            for index in 0..30 {
                h.state_mut().run("anchor.create", json!({"name": format!("Anchor {index}")})).unwrap();
            }
            h.run_steps(3);
            assert_eq!(h.get_by_label("New Text Anchor").rect(), footer);
            let first_top = h.get_by_label("Anchor 0").rect().top();
            let scroll = h.get_by_role(egui::accesskit::Role::ScrollBar).rect();
            assert!(scroll.bottom() <= footer.top());
            h.hover_at(egui::pos2(100.0, scroll.center().y));
            h.event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -120.0),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
                source: egui::MouseWheelSource::Unknown,
            });
            h.run_steps(3);
            assert_eq!(h.get_by_label("New Text Anchor").rect(), footer);
            assert!(h.query_by_label("Anchor 0").is_none_or(|node| node.rect().top() < first_top));
            assert!(h.query_by_role(egui::accesskit::Role::TextInput).is_none());
        }
    }

    #[test]
    fn anchor_tree_renders_indented_collapsible_rows_and_toggles_nested_bookmarks() {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({})).unwrap();
        let frame = app.run("frame.create", json!({"rect": [36, 36, 300, 300], "content": "text", "text": "Part\nChapter\nSection"})).unwrap();
        app.run("text.select", json!({"story": frame["story"], "anchor": 0, "focus": 0})).unwrap();
        let part = app.run("anchor.create", json!({"name": "Part"})).unwrap()["id"].clone();
        app.run("text.select", json!({"story": frame["story"], "anchor": 8, "focus": 8})).unwrap();
        let chapter = app.run("anchor.create", json!({"name": "Chapter", "parent": part})).unwrap()["id"].clone();
        app.run("text.select", json!({"story": frame["story"], "anchor": 19, "focus": 19})).unwrap();
        let section = app.run("anchor.create", json!({"name": "Section", "parent": chapter})).unwrap()["id"].clone();

        let mut h = Harness::builder()
            .with_size(egui::vec2(320.0, 500.0))
            .with_context_setup(|ctx| {
                crate::theme::install_fonts(ctx, "en");
            })
            .build_ui_state(
                |ui, app| {
                    text_anchors(app, ui);
                    crate::dialogs::show(app, ui.ctx());
                },
                app,
            );
        h.run_steps(3);

        // Child rows are indented further right than their parent.
        let part_rect = h.get_by_label("Part").rect();
        let chapter_rect = h.get_by_label("Chapter").rect();
        let section_rect = h.get_by_label("Section").rect();
        assert!(part_rect.left() < chapter_rect.left() && chapter_rect.left() < section_rect.left());

        // Collapse Part hides Chapter and Section; Expand brings them back.
        h.get_by_label("Collapse Part").click();
        h.run_steps(3);
        assert!(h.query_by_label("Chapter").is_none());
        assert!(h.query_by_label("Section").is_none());
        h.get_by_label("Expand Part").click();
        h.run_steps(3);
        assert!(h.query_by_label("Section").is_some());

        // Toggle bookmark on Section (leaf) and Part (root), skipping Chapter (intermediate):
        // Section nests under Part in Document.bookmarks, and toggling Part off promotes Section.
        assert_eq!(h.get_all_by_label("Bookmark").count(), 3);
        h.get_all_by_label("Bookmark").nth(2).unwrap().click();
        h.run_steps(3);
        h.get_all_by_label("Bookmark").next().unwrap().click();
        h.run_steps(3);
        let d = &h.state().session.doc().unwrap().doc;
        assert_eq!(d.bookmarks.len(), 1);
        assert_eq!(d.bookmarks[0].anchor, part.as_u64());
        assert_eq!(d.bookmarks[0].children.len(), 1);
        assert_eq!(d.bookmarks[0].children[0].anchor, section.as_u64());

        // Toggle Part off: Section remains bookmarked at root.
        h.get_all_by_label("Bookmark").next().unwrap().click();
        h.run_steps(3);
        let d = &h.state().session.doc().unwrap().doc;
        assert_eq!(d.bookmarks.len(), 1);
        assert_eq!(d.bookmarks[0].anchor, section.as_u64());

        // Right-click Section -> Move to Parent -> [No Parent] moves Section to root with undo.
        let sec_center = h.get_by_label("Section").rect().center();
        h.event(egui::Event::PointerMoved(sec_center));
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: sec_center, button: egui::PointerButton::Secondary, pressed, modifiers: Default::default() });
            h.run_steps(2);
        }
        h.get_by_label("Move to Parent").hover();
        h.run_steps(3);
        h.get_by_label("[No Parent]").click();
        h.run_steps(3);
        assert_eq!(h.state().session.doc().unwrap().doc.anchor(section.as_u64().unwrap()).unwrap().parent, None);
        h.state_mut().run("edit.undo", json!({})).unwrap();
        assert_eq!(h.state().session.doc().unwrap().doc.anchor(section.as_u64().unwrap()).unwrap().parent, chapter.as_u64());
    }

    #[test]
    fn bookmarks_panel_displays_nested_bookmarks_and_edits_by_path() {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({})).unwrap();
        let frame = app.run("frame.create", json!({"rect": [36, 36, 300, 300], "content": "text", "text": "Part\nSection"})).unwrap();
        app.run("text.select", json!({"story": frame["story"], "anchor": 0, "focus": 0})).unwrap();
        let part = app.run("anchor.create", json!({"name": "Part"})).unwrap()["id"].clone();
        app.run("text.select", json!({"story": frame["story"], "anchor": 8, "focus": 8})).unwrap();
        let section = app.run("anchor.create", json!({"name": "Section", "parent": part})).unwrap()["id"].clone();
        app.run("bookmark.add", json!({"anchor": part})).unwrap();
        app.run("bookmark.add", json!({"anchor": section})).unwrap();

        let mut h = Harness::builder()
            .with_size(egui::vec2(320.0, 500.0))
            .with_context_setup(|ctx| {
                crate::theme::install_fonts(ctx, "en");
            })
            .build_ui_state(|ui, app| bookmarks(app, ui), app);
        h.run_steps(3);
        let part_rect = h.get_by_label("Part").rect();
        let section_rect = h.get_by_label("Section").rect();
        assert!(part_rect.left() < section_rect.left());
        // Collapse hides child bookmark; Expand shows it again.
        h.get_by_label("Collapse Part").click();
        h.run_steps(3);
        assert!(h.query_by_label("Section").is_none());
        h.get_by_label("Expand Part").click();
        h.run_steps(3);
        assert!(h.query_by_label("Section").is_some());
        // Deleting parent bookmark promotes child bookmark to top level.
        h.get_all_by_label("Delete").next().unwrap().click();
        h.run_steps(3);
        let d = &h.state().session.doc().unwrap().doc;
        assert_eq!(d.bookmarks.len(), 1);
        assert_eq!(d.bookmarks[0].anchor, section.as_u64());
    }
}

#[cfg(test)]
mod interactive_panel_tests {
    use super::*;
    use egui_kittest::{Harness, kittest::Queryable};

    #[test]
    fn long_names_leave_delete_visible_in_bookmarks_and_hyperlinks() {
        for is_bookmark in [true, false] {
            let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
            app.run("file.new", json!({})).unwrap();
            let name = "A name that is much too long to fit inside a narrow panel without hiding its action buttons";
            let list = if is_bookmark {
                app.run("bookmark.add", json!({"name": name, "page": 0})).unwrap();
                "bookmark.list"
            } else {
                let frame = app.run("frame.create", json!({"rect": [36, 36, 300, 200]})).unwrap();
                app.run("hyperlink.create", json!({"ids": [frame["id"]], "url": "https://example.com", "name": name})).unwrap();
                "hyperlink.list"
            };
            let mut h = Harness::builder().with_size(egui::vec2(280.0, 400.0)).build_ui_state(
                move |ui, app| {
                    if is_bookmark {
                        bookmarks(app, ui);
                    } else {
                        hyperlinks(app, ui);
                    }
                },
                app,
            );
            h.run_steps(3);
            let delete = h.get_by_label("Delete");
            let name_button = h.get_by_label(name);
            assert!(delete.rect().right() <= h.ctx.content_rect().right(), "{list}: {:?}", delete.rect());
            assert!(name_button.rect().right() <= delete.rect().left(), "{list}: {:?}", name_button.rect());
            assert!(
                h.output().shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(text) if text.galley.job.text == name && text.galley.elided))
            );
            delete.click();
            h.run_steps(3);
            assert!(h.state_mut().session.execute(list, &json!({})).unwrap().as_array().unwrap().is_empty());
        }
    }
}

/// Articles panel: reading-order lists for EPUB / HTML export.
pub fn articles(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let has_sel = app.session.active().is_some_and(|d| !d.selection.items.is_empty());
    if ui.add_enabled(has_sel, egui::Button::new(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "New Article from Selection")))).clicked() {
        let _ = app.run("article.new", json!({}));
    }
    ui.separator();
    let list = app.session.execute("article.list", &json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    if list.is_empty() {
        crate::rtl::label(
            ui,
            crate::rtl::widget(
                ui,
                egui::RichText::new(crate::i18n::tr(&app.ui.language, "No articles: exports follow page order.")).size(11.0).color(t.text_dim),
            ),
        );
    }
    for a in list {
        let name = a["name"].as_str().unwrap_or("").to_string();
        let mut export = a["export"].as_bool().unwrap_or(true);
        ui.horizontal(|ui| {
            if ui
                .checkbox(&mut export, "")
                .on_hover_ui(|ui| {
                    crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Include when exporting"));
                })
                .changed()
            {
                let _ = app.run("article.options", json!({"name": name, "export": export}));
            }
            crate::rtl::label(ui, egui::RichText::new(&name).strong());
            crate::rtl::label(
                ui,
                egui::RichText::new(crate::i18n::count_label(&app.ui.language, "objects", a["items"].as_array().map_or(0, Vec::len)))
                    .size(10.5)
                    .color(t.text_dim),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Delete"))).clicked() {
                    let _ = app.run("article.delete", json!({"name": name}));
                }
                if ui
                    .add_enabled(has_sel, egui::Button::new(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Add"))).small())
                    .on_hover_ui(|ui| {
                        crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Add the selection"));
                    })
                    .clicked()
                {
                    let _ = app.run("article.add", json!({"name": name}));
                }
            });
        });
    }
}

/// Tags panel and structure: tags (click to tag the selection), the structure in reading order,
/// XML export/import.
pub fn tags(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let Ok(info) = app.session.execute("xml.tags", &json!({})) else { return };
    let has_sel = app.session.active().is_some_and(|d| !d.selection.items.is_empty());
    // Selected text is tagged inline instead of its frame.
    let text_sel = app.session.active().and_then(|d| d.selection.text).is_some_and(|t| !t.range().is_empty());
    let id = egui::Id::unique("new_tag_name");
    let mut name: String = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut name)
                .hint_text(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "New tag")))
                .desired_width(140.0),
        );
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "New"))).clicked() && !name.trim().is_empty() {
            match app.run("xml.newTag", json!({"name": name.trim()})) {
                Ok(_) => name.clear(),
                Err(e) => app.status_error("Tags", e),
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(id, name));
    for tag in info["tags"].as_array().cloned().unwrap_or_default() {
        let n = tag["name"].as_str().unwrap_or("").to_string();
        let c = tag["color"].as_array().map(|a| a.iter().map(|v| v.as_u64().unwrap_or(0) as u8).collect::<Vec<_>>()).unwrap_or_default();
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
            if c.len() == 3 {
                ui.painter().rect_filled(r, 2.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
            }
            if ui
                .add_enabled(has_sel || text_sel, egui::Button::new(&n).frame(false))
                .on_hover_ui(|ui| {
                    crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Tag the selection"));
                })
                .clicked()
            {
                let _ = if text_sel { app.run("xml.tagText", json!({"tag": n})) } else { app.run("xml.tag", json!({"tag": n})) };
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Delete"))).clicked() {
                    let _ = app.run("xml.deleteTag", json!({"name": n}));
                }
            });
        });
    }
    if (has_sel || text_sel) && ui.small_button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Untag Selection"))).clicked() {
        let _ = if text_sel { app.run("xml.tagText", json!({"tag": null})) } else { app.run("xml.tag", json!({"tag": null})) };
    }
    let mut markers = app.ui.tag_markers;
    if ui.checkbox(&mut markers, crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Show Tag Markers"))).changed() {
        let _ = app.run("view.tagMarkers", json!({"on": markers}));
    }
    ui.separator();
    crate::rtl::label(ui, egui::RichText::new(crate::i18n::tr(&app.ui.language, "Structure")).strong());
    let st = app.session.execute("xml.structure", &json!({})).unwrap_or_default();
    crate::rtl::label(ui, egui::RichText::new(st["root"].as_str().unwrap_or("Root")).size(11.0).color(t.text_dim));
    for el in st["elements"].as_array().cloned().unwrap_or_default() {
        let text = el["text"].as_str().unwrap_or("").chars().take(40).collect::<String>();
        let label = format!("  <{}> {}", el["tag"].as_str().unwrap_or(""), text);
        if ui.selectable_label(false, egui::RichText::new(label).size(11.0)).clicked() {
            let _ = app.run("selection.set", json!({"ids": [el["id"]]}));
        }
    }
    // DTD: load, validate (problems listed until the next validation), delete.
    let vid = egui::Id::unique("dtd_problems");
    let mut problems: Option<Vec<String>> = ui.data(|d| d.get_temp(vid));
    ui.horizontal(|ui| {
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Load DTD…"))).clicked() {
            // A new DTD's problems show at the next validation.
            problems = None;
            ask_then_run(app, "dtd", "Load DTD", |path| ("xml.loadDtd", json!({"path": path})));
        }
        let has_dtd = info["dtd"].as_bool().unwrap_or(false);
        if ui.add_enabled(has_dtd, egui::Button::new(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Validate")))).clicked()
            && let Ok(v) = app.session.execute("xml.validate", &json!({}))
        {
            problems = Some(
                v["problems"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|p| format!("{}: {}", p["path"].as_str().unwrap_or(""), p["message"].as_str().unwrap_or("")))
                    .collect(),
            );
        }
        if has_dtd && ui.small_button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Delete DTD"))).clicked() {
            let _ = app.run("xml.deleteDtd", json!({}));
            problems = None;
        }
    });
    match &problems {
        Some(list) if list.is_empty() => {
            crate::rtl::label(ui, egui::RichText::new(crate::i18n::tr(&app.ui.language, "The structure is valid.")).size(11.0).color(t.text_dim));
        }
        Some(list) => {
            for p in list {
                crate::rtl::label(ui, egui::RichText::new(format!("⚠ {p}")).size(11.0));
            }
        }
        None => {}
    }
    ui.data_mut(|d| match problems {
        Some(p) => {
            d.insert_temp(vid, p);
        }
        None => {
            d.remove::<Vec<String>>(vid);
        }
    });
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Export XML…"))).clicked() {
            let _ = app.run("app.exportXml", json!({}));
        }
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Import XML…"))).clicked() {
            ask_then_run(app, "xml", "Import XML", |path| ("file.importXml", json!({"path": path})));
        }
    });
}

/// Asks for a file for `purpose`, then runs the command `command(path)` names; a failure shows
/// in the status line as `feature`'s.
#[track_caller]
fn ask_then_run(app: &mut DesignApp, purpose: &str, feature: &'static str, command: impl FnOnce(String) -> (&'static str, Value) + 'static) {
    let asked = app.ask_open(purpose, Bound::Document, move |app, path| {
        let (id, params) = command(path);
        if let Err(e) = app.run(id, params) {
            app.status_error(feature, e);
        }
        Ok(Value::Null)
    });
    if let Err(e) = asked {
        app.status_error(feature, e);
    }
}

/// Object States panel: the selected multi-state object's states (click to show one).
pub fn states(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let n = app.session.active().map_or(0, |d| d.selection.items.len());
    let info = app.session.execute("states.list", &json!({})).ok();
    let list = info.as_ref().and_then(|i| i["states"].as_array().cloned()).unwrap_or_default();
    if list.is_empty() {
        crate::rtl::label(
            ui,
            crate::rtl::widget(
                ui,
                egui::RichText::new(crate::i18n::tr(&app.ui.language, "Select two or more objects (or a group) to make a multi-state object."))
                    .size(11.0)
                    .color(t.text_dim),
            ),
        );
        if ui
            .add_enabled(
                n > 0,
                egui::Button::new(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Convert Selection to Multi-State Object"))),
            )
            .clicked()
            && let Err(e) = app.run("states.create", json!({}))
        {
            app.status_error("Object States", e);
        }
        return;
    }
    let active = info.as_ref().and_then(|i| i["active"].as_u64()).unwrap_or(0) as usize;
    for (i, st) in list.iter().enumerate() {
        if ui.selectable_label(i == active, st.as_str().unwrap_or("")).clicked() {
            let _ = app.run("states.show", json!({"index": i}));
        }
    }
    ui.separator();
    if ui.small_button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Release to Objects"))).clicked() {
        let _ = app.run("states.release", json!({}));
    }
}

/// Buttons and Forms panel: the selected objects' On Release action.
pub fn buttons(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let Some(id) = app.session.active().and_then(|d| d.selection.items.first().copied()) else {
        crate::rtl::label(
            ui,
            crate::rtl::widget(
                ui,
                egui::RichText::new(crate::i18n::tr(&app.ui.language, "Select an object to make it a button.")).size(11.0).color(t.text_dim),
            ),
        );
        return;
    };
    let cur = app.session.active().and_then(|d| d.doc.item(id)).and_then(|it| it.button.clone());
    use designcraft_doc::ButtonAction as B;
    let (kind, page, url) = match &cur {
        None => ("none", 0, String::new()),
        Some(B::GoToPage { page }) => ("page", *page, String::new()),
        Some(B::GoToFirstPage) => ("firstPage", 0, String::new()),
        Some(B::GoToLastPage) => ("lastPage", 0, String::new()),
        Some(B::GoToNextPage) => ("nextPage", 0, String::new()),
        Some(B::GoToPreviousPage) => ("previousPage", 0, String::new()),
        Some(B::GoToUrl { url }) => ("url", 0, url.clone()),
    };
    let labels = [
        ("none", "Not a button"),
        ("nextPage", "Go To Next Page"),
        ("previousPage", "Go To Previous Page"),
        ("firstPage", "Go To First Page"),
        ("lastPage", "Go To Last Page"),
        ("page", "Go To Page"),
        ("url", "Go To URL"),
    ];
    let mut pick = None;
    ui.horizontal(|ui| {
        crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "On Release"));
        let combo = craft_ui::ComboBox::from_id_salt("button_action")
            .selected_text(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, labels.iter().find(|l| l.0 == kind).map_or("", |l| l.1))))
            .show_ui(ui, |ui| {
                for (k, l) in labels {
                    if ui.selectable_label(k == kind, crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, l))).clicked() {
                        pick = Some(k);
                    }
                }
            });
        let _ = crate::widgets::name_combo(combo, crate::i18n::tr(&app.ui.language, "On Release Action"));
    });
    let mut params = None;
    if let Some(k) = pick {
        params = Some(json!({"action": k, "page": page, "url": if url.is_empty() { "https://" } else { &url }}));
    }
    if kind == "page" {
        let total = app.session.active().map_or(1, |d| d.doc.page_count());
        let mut n = page + 1;
        ui.horizontal(|ui| {
            crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Page"));
            if ui.add(egui::DragValue::new(&mut n).range(1..=total)).changed() {
                params = Some(json!({"action": "page", "page": n - 1}));
            }
        });
    }
    if kind == "url" {
        let key = egui::Id::unique(("button_url", id.0));
        let mut u: String = ui.data(|d| d.get_temp(key)).unwrap_or(url.clone());
        let r = ui.add(egui::TextEdit::singleline(&mut u).desired_width(f32::INFINITY));
        if r.lost_focus() && u != url {
            params = Some(json!({"action": "url", "url": u}));
        }
        ui.data_mut(|d| d.insert_temp(key, u));
    }
    if let Some(p) = params
        && let Err(e) = app.run("button.set", p)
    {
        app.status_error("Buttons", e);
    }
    crate::rtl::label(
        ui,
        crate::rtl::widget(
            ui,
            egui::RichText::new(crate::i18n::tr(&app.ui.language, "Buttons act in interactive PDF export.")).size(10.5).color(t.text_dim),
        ),
    );
    // Form field.
    ui.separator();
    let ff = app.session.active().and_then(|d| d.doc.item(id)).and_then(|it| it.form_field.clone());
    use designcraft_doc::FieldKind as K;
    let kinds = [
        (None, "Not a form field"),
        (Some(K::TextField), "Text Field"),
        (Some(K::CheckBox), "Check Box"),
        (Some(K::ComboBox), "Combo Box"),
        (Some(K::ListBox), "List Box"),
        (Some(K::Signature), "Signature Field"),
    ];
    let cur = ff.as_ref().map(|f| f.kind);
    ui.horizontal(|ui| {
        crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Form Field"));
        let combo = craft_ui::ComboBox::from_id_salt("form_kind")
            .selected_text(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, kinds.iter().find(|k| k.0 == cur).map_or("", |k| k.1))))
            .show_ui(ui, |ui| {
                for (k, l) in kinds {
                    if ui.selectable_label(k == cur, crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, l))).clicked() {
                        let _ = match k {
                            Some(k) => app.run("form.set", json!({"kind": k})),
                            None => app.run("form.clear", json!({})),
                        };
                    }
                }
            });
        let _ = crate::widgets::name_combo(combo, crate::i18n::tr(&app.ui.language, "Form Field Type"));
    });
    if let Some(f) = ff {
        let key = egui::Id::unique(("form_edit", id.0));
        let (mut name, mut value, mut opts): (String, String, String) =
            ui.data(|d| d.get_temp(key)).unwrap_or((f.name.clone(), f.value.clone(), f.options.join(", ")));
        let mut commit = false;
        ui.horizontal(|ui| {
            crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Name"));
            commit |= ui.text_edit_singleline(&mut name).lost_focus();
        });
        if f.kind == K::CheckBox {
            let mut on = !f.value.is_empty() && f.value != "Off";
            if ui.checkbox(&mut on, crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Checked by default"))).changed() {
                let _ = app.run("form.set", json!({"kind": f.kind, "value": if on { "On" } else { "" }}));
            }
        } else if f.kind != K::Signature {
            ui.horizontal(|ui| {
                crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Default"));
                commit |= ui.text_edit_singleline(&mut value).lost_focus();
            });
        }
        if matches!(f.kind, K::ComboBox | K::ListBox) {
            ui.horizontal(|ui| {
                crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Choices"));
                commit |= ui
                    .text_edit_singleline(&mut opts)
                    .on_hover_ui(|ui| {
                        crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Separated by commas"));
                    })
                    .lost_focus();
            });
        }
        let mut req = f.required;
        if ui.checkbox(&mut req, crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Required"))).changed() {
            let _ = app.run("form.set", json!({"kind": f.kind, "required": req}));
        }
        if f.kind == K::TextField {
            let mut ml = f.multiline;
            if ui.checkbox(&mut ml, crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Multiline"))).changed() {
                let _ = app.run("form.set", json!({"kind": f.kind, "multiline": ml}));
            }
        }
        if commit {
            let options: Vec<String> = opts.split(',').map(|o| o.trim().to_string()).filter(|o| !o.is_empty()).collect();
            let _ = app.run("form.set", json!({"kind": f.kind, "name": name, "value": value, "options": options}));
            ui.data_mut(|d| d.remove::<(String, String, String)>(key));
        } else {
            ui.data_mut(|d| d.insert_temp(key, (name, value, opts)));
        }
    }
}

/// Liquid page rules in InDesign's order, with their menu labels.
pub const LIQUID_RULES: [(designcraft_doc::LiquidRule, &str); 5] = [
    (designcraft_doc::LiquidRule::Off, "Off"),
    (designcraft_doc::LiquidRule::Scale, "Scale"),
    (designcraft_doc::LiquidRule::ReCenter, "Re-center"),
    (designcraft_doc::LiquidRule::ObjectBased, "Object-based"),
    (designcraft_doc::LiquidRule::GuideBased, "Guide-based"),
];

/// A liquid page rule's menu label.
pub fn liquid_rule_label(rule: designcraft_doc::LiquidRule) -> &'static str {
    LIQUID_RULES.iter().find(|(r, _)| *r == rule).map_or("Off", |(_, l)| l)
}

/// Whether page `abs` follows the object-based rule (its own, or its parent page's).
fn object_based(d: &designcraft_doc::Document, abs: usize) -> bool {
    use designcraft_doc::LiquidRule as L;
    let Some((si, pi)) = d.page_loc(abs) else { return false };
    match d.spreads[si].pages[pi].liquid {
        L::ObjectBased => true,
        L::ControlledByParent => {
            d.parent_page_for(abs).and_then(|(ppi, ppg)| d.parents.get(ppi)?.pages.get(ppg).map(|p| p.liquid == L::ObjectBased)).unwrap_or(false)
        }
        _ => false,
    }
}

/// Liquid Layout panel, laid out as InDesign's: the page's Liquid Page Rule (with "Page: n"),
/// Content › Auto-Fit, and Object Constraints › Resize with Page (Height, Width) and Pin (Top,
/// Left, Bottom, Right) for the selected object on an object-based page. The page's liquid
/// guides follow (Create Alternate Layout is in the Layout menu).
pub fn liquid(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let lang = app.ui.language.clone();
    // The Page tool's page, the page in view, or (before the canvas lays out) the first.
    let Some(page) = app.session.active().map(|s| s.selection.page.or_else(|| crate::canvas::current_page(app)).unwrap_or(0)) else {
        crate::rtl::label(ui, egui::RichText::new(crate::i18n::tr(&lang, "No document.")).size(11.0).color(t.text_dim));
        return;
    };
    let Some(d) = app.session.active().map(|d| d.doc.clone()) else { return };
    let Some((si, pi)) = d.page_loc(page) else { return };
    let pg = d.spreads[si].pages[pi].clone();
    ui.horizontal(|ui| {
        crate::widgets::caption(ui, crate::i18n::tr(&lang, "Liquid Page Rule:"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            crate::widgets::caption(ui, &format!("{} {}", crate::i18n::tr(&lang, "Page:"), d.page_name(page)));
        });
    });
    let combo = craft_ui::ComboBox::from_id_salt("liquid_rule")
        .selected_text(crate::rtl::widget(ui, crate::i18n::tr(&lang, liquid_rule_label(pg.liquid))))
        .width(190.0)
        .show_ui(ui, |ui| {
            for (r, label) in LIQUID_RULES {
                if ui.selectable_label(pg.liquid == r, crate::rtl::widget(ui, crate::i18n::tr(&lang, label))).clicked() && r != pg.liquid {
                    let _ = app.run("liquid.pageRule", json!({"rule": r, "pages": [page + 1]}));
                }
            }
        });
    let _ = crate::widgets::name_combo(combo, crate::i18n::tr(&lang, "Liquid Page Rule"));
    ui.separator();
    // The selected object (one on this page).
    let item = app
        .session
        .active()
        .and_then(|s| s.selection.items.first().copied())
        .and_then(|id| d.item(id))
        .filter(|it| d.page_of_item(it.id) == Some(page));
    // Content: Auto-Fit (graphic frames).
    crate::widgets::caption(ui, crate::i18n::tr(&lang, "Content:"));
    let graphic = item.and_then(|it| if let designcraft_doc::Content::Graphic(g) = &it.content { Some(g.auto_fit) } else { None });
    let mut auto = graphic.is_some_and(|f| f != designcraft_doc::Fitting::None);
    ui.indent("liquid_content", |ui| {
        let r = ui.add_enabled(graphic.is_some(), egui::Checkbox::new(&mut auto, crate::rtl::widget(ui, crate::i18n::tr(&lang, "Auto-Fit"))));
        if r.changed() {
            let _ = app.run("object.fittingOptions", json!({"autoFit": auto}));
        }
    });
    // Object Constraints: for an object on an object-based page.
    crate::widgets::caption(ui, crate::i18n::tr(&lang, "Object Constraints:"));
    let on = item.is_some() && object_based(&d, page);
    let l = item
        .filter(|_| on)
        .map(|it| it.liquid.unwrap_or(designcraft_doc::ObjectLiquid { resize_width: true, resize_height: true, ..Default::default() }))
        .unwrap_or_default();
    ui.indent("liquid_constraints", |ui| {
        ui.add_enabled_ui(on, |ui| {
            let rows: [(&str, [(&str, &str, bool); 2]); 2] = [
                ("Resize with Page:", [("Height", "resizeHeight", l.resize_height), ("Width", "resizeWidth", l.resize_width)]),
                ("Pin:", [("Top", "pinTop", l.pin_top), ("Left", "pinLeft", l.pin_left)]),
            ];
            for (title, boxes) in rows {
                crate::widgets::caption(ui, crate::i18n::tr(&lang, title));
                let mut grid_boxes = vec![boxes.to_vec()];
                if title == "Pin:" {
                    grid_boxes.push(vec![("Bottom", "pinBottom", l.pin_bottom), ("Right", "pinRight", l.pin_right)]);
                }
                egui::Grid::new(("liquid_grid", title)).num_columns(2).min_col_width(90.0).show(ui, |ui| {
                    for row in grid_boxes {
                        for (label, key, value) in row {
                            let mut v = value;
                            if ui.checkbox(&mut v, crate::rtl::widget(ui, crate::i18n::tr(&lang, label))).changed() {
                                let _ = app.run("liquid.object", json!({key: v}));
                            }
                        }
                        ui.end_row();
                    }
                });
            }
        });
    });
    if !pg.guides.is_empty() {
        ui.separator();
        crate::rtl::label(ui, egui::RichText::new(crate::i18n::tr(&lang, "Liquid guides")).size(11.0).color(t.text_dim));
        for (gi, g) in pg.guides.iter().enumerate() {
            let mut on = g.liquid;
            let label = format!("{:?} at {:.1}", g.orientation, g.position);
            if ui.checkbox(&mut on, label).changed() {
                let _ = app.run("guide.liquid", json!({"spread": si, "page": pi, "index": gi, "on": on}));
            }
        }
    }
}

/// Media panel: the selected video or sound's playback options and poster.
pub fn media(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let Ok(m) = app.session.execute("media.get", &json!({})) else {
        crate::rtl::label(
            ui,
            crate::rtl::widget(
                ui,
                egui::RichText::new(crate::i18n::tr(&app.ui.language, "Select a placed video or sound (File › Place a .mp4, .mov, .mp3, .wav …)."))
                    .size(11.0)
                    .color(t.text_dim),
            ),
        );
        return;
    };
    crate::rtl::label(ui, egui::RichText::new(format!("{} — {}", m["kind"].as_str().unwrap_or(""), m["name"].as_str().unwrap_or(""))).strong());
    for (key, label) in [("playOnPageLoad", "Play on Page Load"), ("loop", "Loop"), ("controls", "Show Controls")] {
        let mut on = m[key].as_bool().unwrap_or(false);
        if ui.checkbox(&mut on, crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, label))).changed() {
            let _ = app.run("media.options", json!({key: on}));
        }
    }
    ui.horizontal(|ui| {
        crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Poster"));
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Choose Image…"))).clicked() {
            ask_then_run(app, "place", "Media", |path| ("media.options", json!({"poster": {"path": path}})));
        }
        if !m["poster"].is_null() && ui.small_button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "None"))).clicked() {
            let _ = app.run("media.options", json!({"poster": null}));
        }
    });
    crate::rtl::label(
        ui,
        crate::rtl::widget(
            ui,
            egui::RichText::new(crate::i18n::tr(&app.ui.language, "Plays in EPUB export; print and PDF show the poster."))
                .size(10.5)
                .color(t.text_dim),
        ),
    );
}

/// Page Transitions panel: the current spread's transition for interactive PDF.
pub fn transitions(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let Some(page) = crate::canvas::current_page(app) else { return };
    let Some(d) = app.session.active().map(|d| d.doc.clone()) else { return };
    let Some((si, _)) = d.page_loc(page) else { return };
    let cur = d.spreads[si].pages.first().and_then(|p| p.transition);
    use designcraft_doc::TransitionKind as K;
    crate::rtl::label(ui, egui::RichText::new(format!("{} {}", crate::i18n::tr(&app.ui.language, "Spread"), si + 1)).strong());
    let mut kind = cur.map(|c| c.kind);
    let combo = craft_ui::ComboBox::from_id_salt("transition_kind")
        .selected_text(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, kind.map_or("None", K::label))))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut kind, None, crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "None")));
            for k in K::ALL {
                ui.selectable_value(&mut kind, Some(k), crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, k.label())));
            }
        });
    let _ = crate::widgets::name_combo(combo, crate::i18n::tr(&app.ui.language, "Transition"));
    let mut duration = cur.map_or(1.0, |c| c.duration);
    let mut horizontal = cur.is_some_and(|c| c.horizontal);
    let mut changed = kind != cur.map(|c| c.kind);
    if kind.is_some() {
        ui.horizontal(|ui| {
            crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Speed"));
            changed |= ui.add(egui::DragValue::new(&mut duration).range(0.1..=10.0).speed(0.1).suffix(" s")).changed();
        });
        if matches!(kind, Some(K::Blinds | K::Box | K::Split)) {
            changed |= ui
                .checkbox(
                    &mut horizontal,
                    crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, if kind == Some(K::Box) { "Inward" } else { "Horizontal" })),
                )
                .changed();
        }
    }
    let params = |all: bool| {
        let k = kind.map_or(json!("none"), |k| json!(k));
        if all {
            json!({"all": true, "kind": k, "duration": duration, "horizontal": horizontal})
        } else {
            json!({"spread": si, "kind": k, "duration": duration, "horizontal": horizontal})
        }
    };
    if changed {
        let _ = app.run("page.transition", params(false));
    }
    if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Apply to All Spreads"))).clicked() {
        let _ = app.run("page.transition", params(true));
    }
    crate::rtl::label(
        ui,
        crate::rtl::widget(
            ui,
            egui::RichText::new(crate::i18n::tr(&app.ui.language, "Transitions play in interactive PDF (full-screen mode)."))
                .size(10.5)
                .color(t.text_dim),
        ),
    );
}

/// Track Changes panel: tracking on/off and each change with Accept / Reject.
pub fn track_changes(app: &mut DesignApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let Some(on) = app.session.active().map(|d| d.doc.settings.track_changes) else { return };
    let mut track = on;
    if ui.checkbox(&mut track, crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Track Changes in All Stories"))).changed() {
        let _ = app.run("changes.track", json!({"on": track}));
    }
    let list = app.session.execute("changes.list", &json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    ui.separator();
    if list.is_empty() {
        crate::rtl::label(ui, egui::RichText::new(crate::i18n::tr(&app.ui.language, "No changes.")).size(11.0).color(t.text_dim));
        return;
    }
    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
        for c in &list {
            let (story, start) = (c["story"].clone(), c["start"].clone());
            let kind = c["kind"].as_str().unwrap_or("");
            let text: String = c["text"].as_str().unwrap_or("").chars().take(40).collect();
            ui.horizontal(|ui| {
                let label = egui::RichText::new(format!(
                    "{} “{}”",
                    crate::i18n::tr(&app.ui.language, if kind == "inserted" { "Added" } else { "Deleted" }),
                    text.replace('\n', "¶")
                ))
                .size(11.0);
                let label = if kind == "deleted" { label.strikethrough() } else { label };
                if ui
                    .selectable_label(false, label)
                    .on_hover_ui(|ui| {
                        crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Show in the text"));
                    })
                    .clicked()
                {
                    let end = c["end"].clone();
                    let _ = app.run("text.select", json!({"story": story, "anchor": start, "focus": end}));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Reject"))).clicked() {
                        let _ = app.run("changes.reject", json!({"story": story, "start": start}));
                    }
                    if ui.small_button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Accept"))).clicked() {
                        let _ = app.run("changes.accept", json!({"story": story, "start": start}));
                    }
                });
            });
        }
    });
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Accept All"))).clicked() {
            let _ = app.run("changes.acceptAll", json!({}));
        }
        if ui.button(crate::rtl::widget(ui, crate::i18n::tr(&app.ui.language, "Reject All"))).clicked() {
            let _ = app.run("changes.rejectAll", json!({}));
        }
    });
}

#[cfg(test)]
mod liquid_panel_tests {
    use egui_kittest::{Harness, kittest::Queryable};
    use serde_json::json;

    use super::*;

    #[test]
    fn object_constraints_set_the_selected_objects_pins_and_resizing() {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({})).unwrap();
        let id = app.run("frame.create", json!({"rect": [100, 100, 200, 200]})).unwrap()["id"].as_u64().unwrap();
        let mut h = Harness::builder().with_size(egui::vec2(260.0, 420.0)).build_ui_state(
            |ui, app: &mut DesignApp| {
                let key = egui::Id::unique("test_fonts");
                if ui.data(|d| d.get_temp::<bool>(key)).is_none() {
                    crate::theme::install_fonts(ui.ctx(), "");
                    ui.data_mut(|d| d.insert_temp(key, true));
                    return;
                }
                liquid(app, ui);
            },
            app,
        );
        h.run_steps(3);
        let l = |h: &Harness<'_, DesignApp>| h.state().session.doc().unwrap().doc.item(designcraft_doc::ItemId(id)).unwrap().liquid;
        // With the rule off the constraints don't apply.
        h.get_by_label("Right").click();
        h.run_steps(2);
        assert_eq!(l(&h), None);
        h.state_mut().run("liquid.pageRule", json!({"rule": "objectBased"})).unwrap();
        h.run_steps(2);
        h.get_by_label("Right").click();
        h.run_steps(2);
        h.get_by_label("Width").click();
        h.run_steps(2);
        let v = l(&h).unwrap();
        assert!(v.pin_right && !v.resize_width && v.resize_height, "{v:?}");
        let _ = h.get_by_label("Auto-Fit");
    }
}
