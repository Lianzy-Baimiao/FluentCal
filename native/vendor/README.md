# vendor/

`i-slint-renderer-software`: Slint 1.18.1's software renderer with one change, marked
"FluentCal patch" in `fonts/vectorfont.rs`: glyph outlines are hinted (`.hint(true)` on the
swash scaler). 0.2 rendered text with DirectWrite in WebView2, which hints UI text; unhinted
glyphs look lighter and blurrier at 100–150 % scaling (measured with scripts/parity.mjs at 125 %).
Glyph advances are unchanged, so text layout is the same as with the original crate.

Used through `[patch.crates-io]` in ../Cargo.toml. When updating Slint, copy the new version of
the crate here and re-apply the patch. License: as upstream (see LICENSES/, the files keep
their notices); FluentCal uses Slint under the Royalty-free license, which needs the AboutSlint
widget in the About page.

`winit`: winit 0.30.13 (the version Slint 1.18.1 uses) with winit PR #4341 backported, as tested
in https://github.com/rust-windowing/winit/issues/4600 (marked "FluentCal patch" in
`src/platform_impl/windows/event_loop.rs`; also `util.rs` and `dark_mode.rs`). When a window is
dragged onto a screen with another scale, Windows 11 sends WM_DPICHANGED while the window is
still mostly on the old screen. 0.30.13 judged the suggested rectangle by the screen the window
was on and pushed it back there, so the scale flipped back and forth during the drag and the
size ran away (winit #4704, slint #11073). With the patch the window takes the rectangle
Windows suggests on Windows 11; Windows 10 (build < 22000) keeps the old code. The fix is in
winit 0.31: delete this copy when Slint moves to it. To check, run `cargo run --example
dpi_probe` with two side-by-side screens at different scales; it fails without the patch.
License: Apache-2.0, as upstream (LICENSE).
