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
