use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use log::{Level, LevelFilter, Metadata, Record};
use pomolice_lib::{log_info_event, BackendLogEvent};

#[path = "../tray_icon_renderer.rs"]
mod tray_icon_renderer;

use pomolice_core::types::TimerStatus;
use tray_icon_renderer::{IdleGlyphVariant, IDLE_GLYPH_VARIANTS};

/// Explicit export instruction for one tray icon fixture.
///
/// # Examples
/// ```ignore
/// let instruction = IconExportInstruction {
///     output_name: "idle-hollow-circle",
///     render_action: IconRenderAction::Idle(IdleGlyphVariant::HollowCircle),
/// };
/// ```
#[derive(Debug, Clone, Copy)]
struct IconExportInstruction {
    output_name: &'static str,
    render_action: IconRenderAction,
}

#[derive(Debug, Clone, Copy)]
enum IconRenderAction {
    ActiveIdle,
    Idle(IdleGlyphVariant),
    RemainingMinutes { status: TimerStatus, minutes: u32 },
}

fn main() -> Result<(), String> {
    initialize_logging();

    let output_directory = project_root()?
        .join("docs")
        .join("tray-icon-visual-review")
        .join("fixtures");
    fs::create_dir_all(&output_directory).map_err(|error| {
        format!(
            "Failed to create output directory {}: {error}",
            output_directory.display()
        )
    })?;

    let export_instructions: &[IconExportInstruction] = &[
        IconExportInstruction {
            output_name: "idle-active",
            render_action: IconRenderAction::ActiveIdle,
        },
        IconExportInstruction {
            output_name: idle_variant_file_label(IDLE_GLYPH_VARIANTS[0]),
            render_action: IconRenderAction::Idle(IDLE_GLYPH_VARIANTS[0]),
        },
        IconExportInstruction {
            output_name: idle_variant_file_label(IDLE_GLYPH_VARIANTS[1]),
            render_action: IconRenderAction::Idle(IDLE_GLYPH_VARIANTS[1]),
        },
        IconExportInstruction {
            output_name: idle_variant_file_label(IDLE_GLYPH_VARIANTS[2]),
            render_action: IconRenderAction::Idle(IDLE_GLYPH_VARIANTS[2]),
        },
        IconExportInstruction {
            output_name: "working-25m",
            render_action: IconRenderAction::RemainingMinutes {
                status: TimerStatus::Working,
                minutes: 25,
            },
        },
        IconExportInstruction {
            output_name: "break-5m",
            render_action: IconRenderAction::RemainingMinutes {
                status: TimerStatus::OnBreak,
                minutes: 5,
            },
        },
        IconExportInstruction {
            output_name: "working-2m",
            render_action: IconRenderAction::RemainingMinutes {
                status: TimerStatus::Working,
                minutes: 2,
            },
        },
    ];

    for instruction in export_instructions {
        let rgba = render_rgba(instruction.render_action);
        let size = tray_icon_renderer::icon_size();
        let output_path = output_directory.join(format!("{}.png", instruction.output_name));
        write_png_rgba(&output_path, size, size, &rgba)?;
        log_info_event(
            BackendLogEvent::ToolingTrayIconFixtureExported,
            &[("path", output_path.display().to_string())],
        );
    }

    log_info_event(
        BackendLogEvent::ToolingTrayIconFixtureExportCompleted,
        &[("output_directory", output_directory.display().to_string())],
    );
    Ok(())
}

fn initialize_logging() {
    static LOGGER: FixtureExportLogger = FixtureExportLogger;

    let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(LevelFilter::Info));
}

struct FixtureExportLogger;

impl log::Log for FixtureExportLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            let mut stderr = std::io::stderr().lock();
            let _ = writeln!(stderr, "{}", record.args());
        }
    }

    fn flush(&self) {}
}

fn render_rgba(render_action: IconRenderAction) -> Vec<u8> {
    match render_action {
        IconRenderAction::ActiveIdle => tray_icon_renderer::render_idle_icon(),
        IconRenderAction::Idle(variant) => {
            tray_icon_renderer::render_idle_icon_for_variant(variant)
        }
        IconRenderAction::RemainingMinutes { status, minutes } => {
            tray_icon_renderer::render_tray_icon(minutes, status)
        }
    }
}

fn idle_variant_file_label(variant: IdleGlyphVariant) -> &'static str {
    match variant {
        IdleGlyphVariant::HollowCircle => "idle-hollow-circle",
        IdleGlyphVariant::PowerStandby => "idle-power-standby",
        IdleGlyphVariant::PauseBars => "idle-pause-bars",
    }
}

fn write_png_rgba(output_path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let file = File::create(output_path)
        .map_err(|error| format!("Failed to create {}: {error}", output_path.display()))?;
    let writer = BufWriter::new(file);

    let mut encoder = png::Encoder::new(writer, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut png_writer = encoder.write_header().map_err(|error| {
        format!(
            "Failed to write PNG header {}: {error}",
            output_path.display()
        )
    })?;
    png_writer.write_image_data(rgba).map_err(|error| {
        format!(
            "Failed to write PNG pixels {}: {error}",
            output_path.display()
        )
    })?;

    Ok(())
}

fn project_root() -> Result<PathBuf, String> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(std::path::Path::to_path_buf)
        .ok_or_else(|| "Expected src-tauri to have a parent directory".to_string())
}
