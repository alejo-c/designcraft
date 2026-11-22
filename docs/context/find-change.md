# Find/Change interaction

- Keep the last found or replaced text selected and visible. A subsequent Find Next or Change advances past that result in the original search domain.
- Repeated Change must also advance when the replacement still matches the query, changes formatting only, or deletes the text.
- Preserve keyboard focus on dialog controls so repeated keyboard activation remains possible; moving the viewport does not transfer keyboard input to document text.
- Centering the canvas without accounting for the Find/Change dialog can hide the match behind the dialog. Use the available canvas area beside the dialog.
- Search and replacement remain engine commands, including when driven through automation.

# Verification commands

- cargo fmt --check -p designcraft-engine -p designcraft-ui-egui
- cargo check -p designcraft-engine -p designcraft-ui-egui
- cargo test -p designcraft-engine -p designcraft-ui-egui
- cargo clippy -p designcraft-engine -p designcraft-ui-egui --all-targets -- -D warnings
- cargo xtask layers
- cargo xtask assets
- cargo xtask wasm
- cargo xtask parity
