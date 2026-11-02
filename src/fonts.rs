use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

const BUNDLED_FONT_NAME: &str = "hashforge-source-han-sans";
const BUNDLED_FONT: &[u8] = include_bytes!("../assets/fonts/SourceHanSansSC-Regular.otf");

fn bundled_definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        BUNDLED_FONT_NAME.to_owned(),
        Arc::new(FontData::from_static(BUNDLED_FONT)),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, BUNDLED_FONT_NAME.to_owned());
    // Keep ASCII hashes monospaced while providing CJK glyphs for text input.
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .push(BUNDLED_FONT_NAME.to_owned());
    fonts
}

pub fn install(ctx: &egui::Context) {
    // Always install the embedded font, even if no system font is available.
    #[allow(unused_mut)]
    let mut fonts = bundled_definitions();
    #[cfg(target_os = "windows")]
    {
        // Debug builds can preview the same embedded-only font setup as macOS/Linux.
        let bundled_preview =
            cfg!(debug_assertions) && std::env::var_os("HASHFORGE_PREVIEW_BUNDLED_FONT").is_some();
        if !bundled_preview {
            if let Some(font) = microsoft_yahei() {
                let name = "hashforge-microsoft-yahei".to_owned();
                fonts.font_data.insert(name.clone(), Arc::new(font));
                fonts
                    .families
                    .entry(FontFamily::Proportional)
                    .or_default()
                    .insert(0, name.clone());
                fonts
                    .families
                    .entry(FontFamily::Monospace)
                    .or_default()
                    .insert(1, name);
            }
        }
    }
    ctx.set_fonts(fonts);
}

#[cfg(target_os = "windows")]
fn microsoft_yahei() -> Option<FontData> {
    let windows = std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"));
    let bytes = std::fs::read(windows.join("Fonts").join("msyh.ttc")).ok()?;
    Some(FontData::from_blob(Arc::new(bytes), 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_font_renders_ui_and_cjk_text_without_system_fonts() {
        let ctx = egui::Context::default();
        ctx.set_fonts(bundled_definitions());
        let ui_text = include_str!("main.rs");
        let cjk: String = ui_text
            .chars()
            .filter(|ch| ('\u{4e00}'..='\u{9fff}').contains(ch))
            .collect();
        let sample = format!(
            "{cjk}繁體中文檔案雜湊驗證简体中文日本語한국어文件名．（）0123456789ABCDEFabcdef"
        );
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            for family in [FontFamily::Proportional, FontFamily::Monospace] {
                for size in [12.0, 16.0, 20.0] {
                    let id = egui::FontId::new(size, family.clone());
                    ui.fonts_mut(|fonts| {
                        for ch in sample.chars() {
                            assert!(fonts.has_glyph(&id, ch), "missing glyph: {ch} ({id:?})");
                        }
                    });
                    ui.label(egui::RichText::new(&sample).font(id));
                }
            }
        });
        assert!(!ctx
            .tessellate(output.shapes, output.pixels_per_point)
            .is_empty());
        assert!(
            !output.textures_delta.set.is_empty(),
            "glyph atlas should be rasterized"
        );
        output.textures_delta.clear();
    }

    #[test]
    fn embedded_font_keeps_ascii_hashes_monospaced() {
        let ctx = egui::Context::default();
        ctx.set_fonts(bundled_definitions());
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.fonts_mut(|fonts| {
                let id = egui::FontId::monospace(14.0);
                let width = fonts.glyph_width(&id, '0');
                for ch in "0123456789abcdefABCDEF".chars() {
                    assert!((fonts.glyph_width(&id, ch) - width).abs() < 0.01);
                }
            });
        });
        output.textures_delta.clear();
    }
}
