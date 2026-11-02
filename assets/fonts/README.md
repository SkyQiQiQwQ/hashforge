# Bundled Chinese font

Hashforge embeds the unmodified **Source Han Sans SC Regular 2.005** font from
[adobe-fonts/source-han-sans](https://github.com/adobe-fonts/source-han-sans).
It provides CJK glyphs without requiring any system font, font installation,
network access, or additional runtime files.

- Upstream revision: `a4f7cf94edfb9d7ffbdfc4841de276358bd7e0f2` (release branch).
- Font: `OTF/SimplifiedChinese/SourceHanSansSC-Regular.otf`.
- SHA-256: `f1d8611151880c6c336aabeac4640ef434fa13cbfbf1ffe82d0a71b2a5637256`.
- License: SIL Open Font License 1.1, reproduced unchanged in [LICENSE.txt](LICENSE.txt).
- Copyright 2014-2025 Adobe, with Reserved Font Name 'Source'.

The file is embedded with `include_bytes!`, without a heap copy of the entire
font. Only the Regular face is included (16,529,832 bytes); the renderer builds
its glyph atlas as text is used. ASCII digests retain the existing monospace font.

macOS and Linux use this font as the primary UI font. Windows retains Microsoft
YaHei when available, with the embedded font always present as a fallback.
Do not redistribute proprietary system fonts with Hashforge.

The CI packaging step includes the full font license in each archive and as a
separate Release attachment for users who download standalone Windows binaries.

For a local debug preview of the macOS/Linux font setup on Windows:

```powershell
$env:HASHFORGE_PREVIEW_BUNDLED_FONT = '1'
cargo run --bin hashforge
```

This override is ignored in Release builds. Font regression tests explicitly
disable system fonts and check UI glyph coverage, CJK input, glyph rasterization,
and monospace digest layout on all three CI operating systems.
