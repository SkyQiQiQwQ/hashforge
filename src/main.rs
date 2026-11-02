#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use eframe::egui::{self, Color32, CornerRadius, Frame, Margin, RichText, Stroke, TextStyle};
use hashforge::{hash_bytes, hash_file, validate_hex, Algorithm, ALGORITHMS, DEFAULT_ALGORITHM};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

mod fonts;

const BACKGROUND: Color32 = Color32::from_rgb(246, 248, 247);
const SURFACE: Color32 = Color32::from_rgb(255, 255, 255);
const SURFACE_CONTAINER: Color32 = Color32::from_rgb(237, 242, 240);
const SURFACE_LOWEST: Color32 = Color32::from_rgb(250, 252, 251);
const INK: Color32 = Color32::from_rgb(25, 35, 33);
const MUTED: Color32 = Color32::from_rgb(93, 105, 101);
const OUTLINE: Color32 = Color32::from_rgb(195, 204, 200);
const PRIMARY: Color32 = Color32::from_rgb(0, 105, 92);
const PRIMARY_CONTAINER: Color32 = Color32::from_rgb(190, 239, 225);
const ON_PRIMARY_CONTAINER: Color32 = Color32::from_rgb(0, 32, 26);
const SUCCESS: Color32 = Color32::from_rgb(24, 105, 67);
const SUCCESS_CONTAINER: Color32 = Color32::from_rgb(216, 243, 225);
const ERROR: Color32 = Color32::from_rgb(179, 54, 48);
const ERROR_CONTAINER: Color32 = Color32::from_rgb(251, 233, 230);

const COMPARE_DEBOUNCE: Duration = Duration::from_millis(400);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ComparisonSource {
    OriginalText,
    ExpectedDigest,
}

#[derive(Clone, Copy)]
enum WorkTarget {
    FileHash,
    Comparison(u64),
}

enum Task {
    Hash {
        path: PathBuf,
        algorithm: Algorithm,
    },
    Compare {
        path: PathBuf,
        input: String,
        algorithm: Algorithm,
    },
}

enum Output {
    Hash(String),
    Comparison {
        file_digest: String,
        matched: bool,
        source: ComparisonSource,
    },
}

struct Completion {
    target: WorkTarget,
    result: Result<Output, String>,
}

struct HashforgeApp {
    file_path: Option<PathBuf>,
    algorithm: Algorithm,
    file_digest: Option<String>,
    compare_input: String,
    comparison: Option<(ComparisonSource, bool)>,
    input_revision: u64,
    compare_after: Option<Instant>,
    busy: bool,
    busy_target: Option<WorkTarget>,
    error: Option<String>,
    copy_notice: Option<Instant>,
    sender: Sender<Completion>,
    receiver: Receiver<Completion>,
}

impl HashforgeApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut visuals = egui::Visuals::light();
        visuals.panel_fill = BACKGROUND;
        visuals.window_fill = SURFACE;
        visuals.faint_bg_color = SURFACE_CONTAINER;
        visuals.extreme_bg_color = SURFACE_LOWEST;
        visuals.selection.bg_fill = PRIMARY_CONTAINER;
        visuals.selection.stroke = Stroke::new(1.0, PRIMARY);
        visuals.widgets.noninteractive.bg_fill = SURFACE;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, INK);
        visuals.widgets.inactive.bg_fill = SURFACE_CONTAINER;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, OUTLINE);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, INK);
        visuals.widgets.hovered.bg_fill = PRIMARY_CONTAINER;
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, ON_PRIMARY_CONTAINER);
        visuals.widgets.active.bg_fill = PRIMARY;
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
        cc.egui_ctx.set_visuals(visuals);
        fonts::install(&cc.egui_ctx);

        let mut style = (*cc.egui_ctx.style_of(egui::Theme::Light)).clone();
        style.spacing.item_spacing = egui::vec2(12.0, 14.0);
        style.spacing.button_padding = egui::vec2(16.0, 10.0);
        cc.egui_ctx.set_style_of(egui::Theme::Light, style);

        let (sender, receiver) = mpsc::channel();
        Self {
            file_path: None,
            algorithm: DEFAULT_ALGORITHM,
            file_digest: None,
            compare_input: String::new(),
            comparison: None,
            input_revision: 0,
            compare_after: None,
            busy: false,
            busy_target: None,
            error: None,
            copy_notice: None,
            sender,
            receiver,
        }
    }

    fn set_file(&mut self, path: PathBuf, ctx: &egui::Context) {
        if self.file_path.as_ref() == Some(&path) {
            return;
        }
        self.file_path = Some(path);
        self.file_digest = None;
        self.comparison = None;
        self.error = None;
        self.input_revision = self.input_revision.wrapping_add(1);
        self.compare_after = (!self.compare_input.is_empty()).then(Instant::now);
        self.start_file_hash(ctx);
    }

    fn select_file(&mut self, ctx: &egui::Context) {
        if self.busy {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .set_title("选择要校验的文件")
            .pick_file()
        {
            self.set_file(path, ctx);
        }
    }

    fn start_file_hash(&mut self, ctx: &egui::Context) {
        let Some(path) = self.file_path.clone() else {
            return;
        };
        self.file_digest = None;
        self.comparison = None;
        self.error = None;
        self.copy_notice = None;
        self.spawn_task(
            Task::Hash {
                path,
                algorithm: self.algorithm,
            },
            WorkTarget::FileHash,
            ctx,
        );
    }

    fn spawn_task(&mut self, task: Task, target: WorkTarget, ctx: &egui::Context) {
        let sender = self.sender.clone();
        let repaint = ctx.clone();
        match thread::Builder::new()
            .name("hashforge-worker".to_owned())
            .spawn(move || {
                let result = run_task(task);
                let _ = sender.send(Completion { target, result });
                repaint.request_repaint();
            }) {
            Ok(_) => {
                self.busy = true;
                self.busy_target = Some(target);
            }
            Err(error) => {
                self.busy = false;
                self.busy_target = None;
                self.error = Some(format!("无法启动计算线程：{error}"));
            }
        }
    }

    fn poll_worker(&mut self) {
        while let Ok(completion) = self.receiver.try_recv() {
            self.busy = false;
            self.busy_target = None;
            match completion.target {
                WorkTarget::FileHash => match completion.result {
                    Ok(Output::Hash(digest)) => self.file_digest = Some(digest),
                    Ok(Output::Comparison { .. }) => {
                        self.error = Some("文件摘要任务返回了无效结果".to_owned())
                    }
                    Err(error) => self.error = Some(error),
                },
                WorkTarget::Comparison(revision) if revision == self.input_revision => {
                    match completion.result {
                        Ok(Output::Comparison {
                            file_digest,
                            matched,
                            source,
                        }) => {
                            self.file_digest = Some(file_digest);
                            self.comparison = Some((source, matched));
                        }
                        Ok(Output::Hash(_)) => {
                            self.error = Some("比对任务返回了无效结果".to_owned())
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
                WorkTarget::Comparison(_) => {
                    if let Ok(Output::Comparison { file_digest, .. }) = completion.result {
                        self.file_digest = Some(file_digest);
                    }
                }
            }
        }
    }

    fn input_changed(&mut self) {
        self.input_revision = self.input_revision.wrapping_add(1);
        self.comparison = None;
        self.error = None;
        self.copy_notice = None;
        self.compare_after = if self.compare_input.is_empty() {
            None
        } else {
            Some(Instant::now())
        };
    }

    fn maybe_start_comparison(&mut self, ctx: &egui::Context) {
        let Some(since) = self.compare_after else {
            return;
        };
        let remaining = COMPARE_DEBOUNCE.saturating_sub(since.elapsed());
        if !remaining.is_zero() {
            ctx.request_repaint_after(remaining);
            return;
        }
        if self.busy {
            return;
        }
        let Some(path) = self.file_path.clone() else {
            return;
        };
        if self.compare_input.is_empty() {
            self.compare_after = None;
            return;
        }

        self.compare_after = None;
        self.comparison = None;
        self.spawn_task(
            Task::Compare {
                path,
                input: self.compare_input.clone(),
                algorithm: self.algorithm,
            },
            WorkTarget::Comparison(self.input_revision),
            ctx,
        );
    }

    fn show_file_area(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let width = ui.available_width();
        Frame::new()
            .fill(SURFACE_CONTAINER)
            .stroke(Stroke::new(1.0, OUTLINE))
            .corner_radius(CornerRadius::same(16))
            .inner_margin(Margin::same(20))
            .show(ui, |ui| {
                ui.set_width(width - 40.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        if let Some(path) = &self.file_path {
                            let name = path
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or("所选文件");
                            ui.label(RichText::new(name).size(16.0).strong().color(INK));
                            let details = match std::fs::metadata(path) {
                                Ok(metadata) => format!(
                                    "{}  ·  {}",
                                    path.display(),
                                    format_bytes(metadata.len())
                                ),
                                Err(_) => path.display().to_string(),
                            };
                            ui.label(RichText::new(details).size(12.0).color(MUTED));
                        } else {
                            ui.label(
                                RichText::new("拖入文件开始校验")
                                    .size(16.0)
                                    .strong()
                                    .color(INK),
                            );
                            ui.label(RichText::new("或选择本地文件").size(13.0).color(MUTED));
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let button = egui::Button::new(
                            RichText::new(if self.file_path.is_some() {
                                "更换文件"
                            } else {
                                "选择文件"
                            })
                            .strong()
                            .color(Color32::WHITE),
                        )
                        .fill(PRIMARY)
                        .corner_radius(CornerRadius::same(20));
                        if ui.add_enabled(!self.busy, button).clicked() {
                            self.select_file(ctx);
                        }
                    });
                });
            });
    }

    fn show_hash_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            section_heading(ui, "文件摘要");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let previous = self.algorithm;
                ui.add_enabled_ui(!self.busy, |ui| {
                    egui::ComboBox::from_id_salt("hashforge-algorithm")
                        .selected_text(self.algorithm.display_name())
                        .width(132.0)
                        .show_ui(ui, |ui| {
                            for algorithm in ALGORITHMS {
                                ui.selectable_value(
                                    &mut self.algorithm,
                                    algorithm,
                                    algorithm.display_name(),
                                );
                            }
                        });
                });
                if self.algorithm != previous {
                    self.comparison = None;
                    self.error = None;
                    self.input_revision = self.input_revision.wrapping_add(1);
                    self.compare_after = (!self.compare_input.is_empty()).then(Instant::now);
                    self.start_file_hash(ctx);
                }
            });
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let mut digest = self.file_digest.clone().unwrap_or_default();
            let field = egui::TextEdit::singleline(&mut digest)
                .font(TextStyle::Monospace)
                .hint_text(if self.file_path.is_some() {
                    "正在计算摘要..."
                } else {
                    "选择文件后自动计算"
                })
                .interactive(false);
            ui.add_sized([ui.available_width() - 100.0, 44.0], field);
            if self
                .copy_notice
                .is_some_and(|time| time.elapsed() >= Duration::from_secs(2))
            {
                self.copy_notice = None;
            }
            let copied = self.copy_notice.is_some();
            let button = egui::Button::new(if copied { "已复制" } else { "复制" })
                .corner_radius(CornerRadius::same(20));
            if ui.add_enabled(self.file_digest.is_some(), button).clicked() {
                if let Some(digest) = &self.file_digest {
                    ctx.copy_text(digest.clone());
                    self.copy_notice = Some(Instant::now());
                    ctx.request_repaint_after(Duration::from_secs(2));
                }
            }
        });
    }

    fn show_compare_section(&mut self, ui: &mut egui::Ui) {
        section_heading(ui, "自动比对");
        ui.add_space(8.0);
        let hint = format!(
            "输入原文，或粘贴完整的 {} 摘要（不区分大小写）",
            self.algorithm.display_name()
        );
        let response = ui.add_sized(
            [ui.available_width(), 112.0],
            egui::TextEdit::multiline(&mut self.compare_input)
                .font(TextStyle::Monospace)
                .hint_text(hint),
        );
        if response.changed() {
            self.input_changed();
        }
    }

    fn show_status(&self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if let Some(target) = self.busy_target {
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(
                    RichText::new(match target {
                        WorkTarget::FileHash => "正在计算文件摘要...",
                        WorkTarget::Comparison(_) => "正在自动比对...",
                    })
                    .color(MUTED),
                );
            });
        } else if let Some(error) = &self.error {
            ui.add_space(14.0);
            let width = ui.available_width();
            Frame::new()
                .fill(ERROR_CONTAINER)
                .corner_radius(CornerRadius::same(12))
                .inner_margin(Margin::same(14))
                .show(ui, |ui| {
                    ui.set_width(width - 28.0);
                    ui.label(RichText::new(error).color(ERROR));
                });
        } else if let Some((source, matched)) = self.comparison {
            ui.add_space(14.0);
            let (label, fill, color) = match (source, matched) {
                (ComparisonSource::OriginalText, true) => {
                    ("原文与文件一致", SUCCESS_CONTAINER, SUCCESS)
                }
                (ComparisonSource::OriginalText, false) => {
                    ("原文与文件不一致", ERROR_CONTAINER, ERROR)
                }
                (ComparisonSource::ExpectedDigest, true) => {
                    ("摘要匹配", SUCCESS_CONTAINER, SUCCESS)
                }
                (ComparisonSource::ExpectedDigest, false) => ("摘要不匹配", ERROR_CONTAINER, ERROR),
            };
            let width = ui.available_width();
            Frame::new()
                .fill(fill)
                .corner_radius(CornerRadius::same(12))
                .inner_margin(Margin::same(14))
                .show(ui, |ui| {
                    ui.set_width(width - 28.0);
                    ui.horizontal(|ui| {
                        let center = ui
                            .allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover())
                            .0
                            .center();
                        ui.painter().circle_filled(center, 5.0, color);
                        ui.label(RichText::new(label).strong().color(color));
                    });
                });
        } else if !self.compare_input.is_empty() && self.file_path.is_none() {
            ui.add_space(10.0);
            ui.label(RichText::new("选择文件后自动比对").color(MUTED));
        }
        if self.copy_notice.is_some() {
            ctx.request_repaint_after(Duration::from_millis(150));
        }
    }
}

impl eframe::App for HashforgeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_worker();
        let ctx = ui.ctx().clone();
        if self.busy {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        if !self.busy {
            let dropped = ctx.input(|input| {
                input
                    .raw
                    .dropped_files
                    .first()
                    .map(|file| file.path().to_owned())
            });
            if let Some(path) = dropped {
                self.set_file(path, &ctx);
            }
        }

        egui::Panel::top("hashforge-header")
            .exact_size(64.0)
            .frame(Frame::new().fill(SURFACE))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.label(RichText::new("Hashforge").size(20.0).strong().color(INK));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(20.0);
                        ui.label(RichText::new("本地计算").size(12.0).color(MUTED));
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(Frame::new().fill(BACKGROUND))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let available_width = ui.available_width();
                    let content_width = available_width.min(760.0);
                    let side_margin = ((available_width - content_width) * 0.5).max(0.0);
                    ui.add_space(28.0);
                    ui.horizontal(|ui| {
                        ui.add_space(side_margin);
                        ui.vertical(|ui| {
                            ui.set_width(content_width);
                            self.show_file_area(ui, &ctx);
                            ui.add_space(24.0);
                            self.show_hash_section(ui, &ctx);
                            ui.add_space(26.0);
                            self.show_compare_section(ui);
                            self.show_status(ui, &ctx);
                            ui.add_space(28.0);
                        });
                    });
                });
            });

        self.maybe_start_comparison(&ctx);
    }
}

fn run_task(task: Task) -> Result<Output, String> {
    match task {
        Task::Hash { path, algorithm } => {
            let digest = hash_file(&path, &[algorithm])?.remove(0).1;
            Ok(Output::Hash(digest))
        }
        Task::Compare {
            path,
            input,
            algorithm,
        } => {
            let trimmed = input.trim();
            let (input_digest, source) = if validate_hex(trimmed, algorithm).is_ok() {
                (trimmed.to_owned(), ComparisonSource::ExpectedDigest)
            } else {
                (
                    hash_bytes(input.as_bytes(), &[algorithm]).remove(0).1,
                    ComparisonSource::OriginalText,
                )
            };
            let file_digest = hash_file(&path, &[algorithm])?.remove(0).1;
            let matched = input_digest.eq_ignore_ascii_case(&file_digest);
            Ok(Output::Comparison {
                file_digest,
                matched,
                source,
            })
        }
    }
}

fn section_heading(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(16.0).strong().color(INK));
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 720.0])
            .with_min_inner_size([680.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Hashforge",
        options,
        Box::new(|cc| Ok(Box::new(HashforgeApp::new(cc)))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct FileFixture(PathBuf);

    impl FileFixture {
        fn new(contents: &[u8]) -> Self {
            static NEXT_ID: AtomicU64 = AtomicU64::new(0);
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "hashforge-gui-{}-{nonce}-{}.txt",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::write(&path, contents).unwrap();
            Self(path)
        }

        fn compare(&self, input: String, algorithm: Algorithm) -> (ComparisonSource, bool) {
            match run_task(Task::Compare {
                path: self.0.clone(),
                input,
                algorithm,
            })
            .unwrap()
            {
                Output::Comparison {
                    source, matched, ..
                } => (source, matched),
                Output::Hash(_) => panic!("comparison task returned a hash-only result"),
            }
        }
    }

    impl Drop for FileFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn auto_comparison_recognizes_all_digest_cases_and_surrounding_whitespace() {
        let file = FileFixture::new(b"abc");
        for (algorithm, lower) in hash_bytes(b"abc", &ALGORITHMS) {
            let upper = lower.to_ascii_uppercase();
            let mixed: String = lower
                .chars()
                .enumerate()
                .map(|(index, ch)| {
                    if index % 2 == 0 {
                        ch.to_ascii_uppercase()
                    } else {
                        ch
                    }
                })
                .collect();
            for value in [lower, upper, mixed] {
                for input in [value.clone(), format!(" \t{value}\r\n")] {
                    assert_eq!(
                        file.compare(input, algorithm),
                        (ComparisonSource::ExpectedDigest, true),
                        "{}",
                        algorithm.display_name()
                    );
                }
            }
        }
    }

    #[test]
    fn auto_comparison_rejects_changed_digests_in_every_case() {
        let file = FileFixture::new(b"abc");
        for (algorithm, digest) in hash_bytes(b"abc", &ALGORITHMS) {
            let replacement = if digest.starts_with('0') { "1" } else { "0" };
            let changed = format!("{replacement}{}", &digest[1..]);
            for input in [changed.clone(), changed.to_ascii_uppercase()] {
                assert_eq!(
                    file.compare(input, algorithm),
                    (ComparisonSource::ExpectedDigest, false),
                    "{}",
                    algorithm.display_name()
                );
            }
        }
    }

    #[test]
    fn original_text_retains_case_and_whitespace() {
        let text = "  AbC 原文\n";
        let file = FileFixture::new(text.as_bytes());
        assert_eq!(
            file.compare(text.to_owned(), DEFAULT_ALGORITHM),
            (ComparisonSource::OriginalText, true)
        );
        for input in [text.to_lowercase(), text.trim().to_owned()] {
            assert_eq!(
                file.compare(input, DEFAULT_ALGORITHM),
                (ComparisonSource::OriginalText, false)
            );
        }
    }
}
