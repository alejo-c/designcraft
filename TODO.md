# DesignCraft TODO

## Pending

## Deferred / low priority

- Rerun the full quality gate with UI fonts covering Japanese and Arabic.
  - Source: cargo xtask ci stopped at missing-glyph panics in canvas composition-focus and Arabic panel-docking tests.
  - Hook: canvas::tests::a_later_field_taking_the_keyboard_ends_the_composition_in_that_pass and panel_docking::tests::the_dock_is_at_the_right_in_english_and_the_left_in_arabic.

## DO-NOT list

- Never resolve a missing text anchor bookmark through its stored page.

## Known commented-out code

## Test commands

- cargo test -p designcraft-engine --lib bookmark
- cargo test -p designcraft-ui-egui --lib panel_creates_a_text_destination_without_an_anchored_object
- cargo xtask ci

## External references
