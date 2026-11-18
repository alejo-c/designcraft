# Project structure

- apps/: native app, CLI and web app.
- assets/: bundled assets, fonts, hyphenation and ICC profiles.
- contributors/: contributor records.
- crates/: color, compose, doc, engine, epub, fonts, format, geom, idml, images, mcp, pdf, pptx, render, textimport, tools, ui-egui.
- crates/engine/src/cmd/: document commands, including text anchor management.
- crates/ui-egui/src/panels/: application panels, including Text Anchors and Bookmarks.
- docs/: detailed documentation; brand, images, user and map subdirectories.
- docs/map/: symbol map index and bookmark domain.
- docs/text-based-anchors-and-bookmarks.md: external integration contract for text destinations and bookmarks.
- licenses/: dependency and asset licenses.
- nix/: Nix configuration.
- packaging/: platform packaging.
- xtask/: repository quality and maintenance commands.
- TODO.md: pending work.
- PROGRESS.md: completed work.
