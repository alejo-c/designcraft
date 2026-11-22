# Caret repaint verification

- egui RawInput.focused reports whether the window receives keyboard input. Use this window state to suspend background caret redraws; individual widget focus describes a different scope.
- Context.repaint_causes reports the preceding pass. Run an empty follow-up pass after the measured caret pass before checking its repaint causes.
- Current API references: https://docs.rs/egui/latest/egui/struct.RawInput.html#structfield.focused and https://docs.rs/egui/latest/egui/struct.Context.html#method.repaint_causes.
