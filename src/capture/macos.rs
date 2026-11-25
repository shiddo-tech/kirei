use anyhow::{Context, Result, anyhow};
use screencapturekit::prelude::*;
use screencapturekit::screenshot_manager::{CGImageExt, SCScreenshotManager};
use screencapturekit::shareable_content::SCShareableContentInfo;
use screencapturekit::stream::configuration::SCCaptureResolutionType;

use super::{Captured, DisplayShot, PickableWindow};

/// Freezes every display, leaving Kirei's own windows out and, when asked, the Finder's desktop
/// icons (but not its windows).
pub fn freeze_displays(hide_desktop_icons: bool) -> Result<Vec<DisplayShot>> {
    let content = SCShareableContent::get()
        .map_err(|e| anyhow!("{e}"))
        .context("sem permissão de Gravação de Tela?")?;
    let me = std::process::id() as i32;
    let mut excluded: Vec<_> = content
        .applications()
        .into_iter()
        .filter(|app| app.process_id() == me)
        .collect();
    let mut keep = Vec::new();
    if hide_desktop_icons
        && let Some(finder) = content
            .applications()
            .into_iter()
            .find(|a| a.bundle_identifier() == "com.apple.finder")
    {
        let pid = finder.process_id();
        keep = content
            .windows()
            .into_iter()
            .filter(|w| {
                w.owning_application()
                    .is_some_and(|a| a.process_id() == pid)
                    && w.window_layer() >= 0
            })
            .collect();
        excluded.push(finder);
    }
    let excluded_refs: Vec<&SCRunningApplication> = excluded.iter().collect();
    let keep_refs: Vec<&SCWindow> = keep.iter().collect();

    let mut shots = Vec::new();
    for display in content.displays() {
        let filter = SCContentFilter::create()
            .with_display(&display)
            .with_excluding_applications(&excluded_refs, &keep_refs)
            .build()
            .map_err(|e| anyhow!("{e}"))?;
        let (width, height) = SCShareableContentInfo::for_filter(&filter)
            .map(|info| info.pixel_size())
            .unwrap_or((display.width(), display.height()));
        let config = SCStreamConfiguration::new()
            .with_width(width)
            .with_height(height)
            .with_shows_cursor(false);
        let image =
            SCScreenshotManager::capture_image(&filter, &config).map_err(|e| anyhow!("{e}"))?;
        shots.push(DisplayShot {
            display_id: display.display_id(),
            width: image.width() as u32,
            height: image.height() as u32,
            bgra: image.bgra_data().map_err(|e| anyhow!("{e}"))?,
        });
    }
    Ok(shots)
}

/// Normal app windows, front to back, excluding Kirei's own. Frames are in global points with the
/// origin at the top-left of the main display, like gpui's.
pub fn pickable_windows() -> Vec<PickableWindow> {
    let Ok(content) = SCShareableContent::get() else {
        return vec![];
    };
    let me = std::process::id() as i32;
    content
        .windows()
        .into_iter()
        .filter(|w| {
            let f = w.frame();
            w.window_layer() == 0
                && w.is_on_screen()
                && f.size.width > 40.
                && f.size.height > 40.
                && w.owning_application().is_some_and(|a| a.process_id() != me)
        })
        .map(|w| {
            let f = w.frame();
            PickableWindow {
                id: w.window_id(),
                x: f.origin.x,
                y: f.origin.y,
                w: f.size.width,
                h: f.size.height,
            }
        })
        .collect()
}

/// One window on its own, with its transparent rounded corners and without the shadow.
pub fn capture_window(id: u32) -> Result<Captured> {
    let content = SCShareableContent::get().map_err(|e| anyhow!("{e}"))?;
    let window = content
        .windows()
        .into_iter()
        .find(|w| w.window_id() == id)
        .context("janela sumiu")?;
    let filter = SCContentFilter::create()
        .with_window(&window)
        .build()
        .map_err(|e| anyhow!("{e}"))?;
    let scale = filter.point_pixel_scale().max(1.) as f64;
    let f = window.frame();
    let config = SCStreamConfiguration::new()
        .with_width(((f.size.width * scale).round() as u32).max(1))
        .with_height(((f.size.height * scale).round() as u32).max(1))
        .with_shows_cursor(false)
        .with_ignores_shadows_single_window(true)
        .map_err(|e| anyhow!("{e}"))?
        .with_should_be_opaque(false)
        .map_err(|e| anyhow!("{e}"))?
        .with_capture_resolution_type(SCCaptureResolutionType::Best)
        .map_err(|e| anyhow!("{e}"))?;
    let image = SCScreenshotManager::capture_image(&filter, &config).map_err(|e| anyhow!("{e}"))?;
    Ok(Captured {
        width: image.width() as u32,
        height: image.height() as u32,
        scale: scale as f32,
        bgra: image.bgra_data().map_err(|e| anyhow!("{e}"))?,
        screen_rect: Some(gpui::Bounds::new(
            gpui::point(gpui::px(f.origin.x as f32), gpui::px(f.origin.y as f32)),
            gpui::size(
                gpui::px(f.size.width as f32),
                gpui::px(f.size.height as f32),
            ),
        )),
        window_id: Some(id),
    })
}

/// A live grab of a screen region (global points), excluding Kirei's windows.
pub struct RegionGrabber {
    filter: SCContentFilter,
    config: SCStreamConfiguration,
}

impl RegionGrabber {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Result<(Self, f64)> {
        let content = SCShareableContent::get().map_err(|e| anyhow!("{e}"))?;
        let (cx, cy) = (x + w / 2., y + h / 2.);
        let display = content
            .displays()
            .into_iter()
            .find(|d| {
                let f = d.frame();
                cx >= f.origin.x
                    && cx <= f.origin.x + f.size.width
                    && cy >= f.origin.y
                    && cy <= f.origin.y + f.size.height
            })
            .context("tela não encontrada")?;
        let me = std::process::id() as i32;
        let own: Vec<_> = content
            .applications()
            .into_iter()
            .filter(|a| a.process_id() == me)
            .collect();
        let refs: Vec<&SCRunningApplication> = own.iter().collect();
        let filter = SCContentFilter::create()
            .with_display(&display)
            .with_excluding_applications(&refs, &[])
            .build()
            .map_err(|e| anyhow!("{e}"))?;
        let scale = filter.point_pixel_scale().max(1.) as f64;
        let f = display.frame();
        let source = screencapturekit::cg::CGRect {
            origin: screencapturekit::cg::CGPoint {
                x: x - f.origin.x,
                y: y - f.origin.y,
            },
            size: screencapturekit::cg::CGSize {
                width: w,
                height: h,
            },
        };
        let config = SCStreamConfiguration::new()
            .with_width(((w * scale).round() as u32).max(1))
            .with_height(((h * scale).round() as u32).max(1))
            .with_source_rect(source)
            .with_shows_cursor(false);
        Ok((Self { filter, config }, scale))
    }

    /// Straight BGRA pixels of the region.
    pub fn grab(&self) -> Result<(u32, u32, Vec<u8>)> {
        let image = SCScreenshotManager::capture_image(&self.filter, &self.config)
            .map_err(|e| anyhow!("{e}"))?;
        Ok((
            image.width() as u32,
            image.height() as u32,
            image.bgra_data().map_err(|e| anyhow!("{e}"))?,
        ))
    }
}

// SAFETY: ScreenCaptureKit filters and configurations are immutable once built and may be used
// from any thread.
unsafe impl Send for RegionGrabber {}

/// Records a screen region or one window to an H.264 MP4 at up to 60 fps, with the cursor and
/// without Kirei's own windows. Needs macOS 15 (SCRecordingOutput).
pub struct Recording {
    stream: SCStream,
    output: screencapturekit::recording_output::SCRecordingOutput,
    path: std::path::PathBuf,
    pub failed: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// The system ended the capture on its own (stop sharing, display gone, window closed).
    pub ended: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

// SAFETY: SCStream and SCRecordingOutput are thread-safe Objective-C objects; the crate's
// blocking calls wait on their own completion handlers.
unsafe impl Send for Recording {}
unsafe impl Sync for Recording {}

fn recording_config(width: f64, height: f64) -> SCStreamConfiguration {
    // H.264 needs even dimensions.
    SCStreamConfiguration::new()
        .with_width(((width as u32) & !1).max(2))
        .with_height(((height as u32) & !1).max(2))
        .with_shows_cursor(true)
        .with_fps(60)
        .with_queue_depth(6)
}

impl Recording {
    pub fn start_area(x: f64, y: f64, w: f64, h: f64, path: std::path::PathBuf) -> Result<Self> {
        let content = SCShareableContent::get().map_err(|e| anyhow!("{e}"))?;
        let (cx, cy) = (x + w / 2., y + h / 2.);
        let display = content
            .displays()
            .into_iter()
            .find(|d| {
                let f = d.frame();
                cx >= f.origin.x
                    && cx <= f.origin.x + f.size.width
                    && cy >= f.origin.y
                    && cy <= f.origin.y + f.size.height
            })
            .context("tela não encontrada")?;
        let me = std::process::id() as i32;
        let own: Vec<_> = content
            .applications()
            .into_iter()
            .filter(|a| a.process_id() == me)
            .collect();
        let refs: Vec<&SCRunningApplication> = own.iter().collect();
        let filter = SCContentFilter::create()
            .with_display(&display)
            .with_excluding_applications(&refs, &[])
            .build()
            .map_err(|e| anyhow!("{e}"))?;
        let scale = filter.point_pixel_scale().max(1.) as f64;
        let f = display.frame();
        let (x0, x1) = ((x - f.origin.x).round(), (x + w - f.origin.x).round());
        let (y0, y1) = ((y - f.origin.y).round(), (y + h - f.origin.y).round());
        let (lw, lh) = ((x1 - x0).max(2.), (y1 - y0).max(2.));
        let config = recording_config(lw * scale, lh * scale)
            .with_source_rect(screencapturekit::cg::CGRect {
                origin: screencapturekit::cg::CGPoint { x: x0, y: y0 },
                size: screencapturekit::cg::CGSize {
                    width: lw,
                    height: lh,
                },
            })
            .with_capture_resolution_type(SCCaptureResolutionType::Best)
            .map_err(|e| anyhow!("{e}"))?;
        Self::launch(&filter, &config, path)
    }

    pub fn start_window(id: u32, path: std::path::PathBuf) -> Result<Self> {
        let content = SCShareableContent::get().map_err(|e| anyhow!("{e}"))?;
        let window = content
            .windows()
            .into_iter()
            .find(|w| w.window_id() == id)
            .context("janela sumiu")?;
        let filter = SCContentFilter::create()
            .with_window(&window)
            .build()
            .map_err(|e| anyhow!("{e}"))?;
        let scale = filter.point_pixel_scale().max(1.) as f64;
        let f = window.frame();
        let config = recording_config(f.size.width * scale, f.size.height * scale)
            .with_ignores_shadows_single_window(true)
            .map_err(|e| anyhow!("{e}"))?
            .with_capture_resolution_type(SCCaptureResolutionType::Best)
            .map_err(|e| anyhow!("{e}"))?;
        Self::launch(&filter, &config, path)
    }

    fn launch(
        filter: &SCContentFilter,
        config: &SCStreamConfiguration,
        path: std::path::PathBuf,
    ) -> Result<Self> {
        use screencapturekit::recording_output::{
            RecordingCallbacks, SCRecordingOutput, SCRecordingOutputCodec,
            SCRecordingOutputConfiguration, SCRecordingOutputFileType,
        };
        use std::sync::atomic::{AtomicBool, Ordering};
        anyhow::ensure!(
            SCRecordingOutput::is_available(),
            "gravação precisa do macOS 15"
        );
        let rec_config = SCRecordingOutputConfiguration::new()
            .map_err(|e| anyhow!("{e}"))?
            .with_output_url(&path)
            .map_err(|e| anyhow!("{e:?}"))?
            .with_video_codec(SCRecordingOutputCodec::H264)
            .with_output_file_type(SCRecordingOutputFileType::MP4);
        let failed = std::sync::Arc::new(AtomicBool::new(false));
        let ended = std::sync::Arc::new(AtomicBool::new(false));
        let (f, e1, e2) = (failed.clone(), ended.clone(), ended.clone());
        let callbacks = RecordingCallbacks::new()
            .on_fail(move |e| {
                eprintln!("kirei: gravação falhou: {e}");
                f.store(true, Ordering::Relaxed);
            })
            .on_finish(move || e1.store(true, Ordering::Relaxed));
        let output = SCRecordingOutput::new_with_delegate(&rec_config, callbacks)
            .context("SCRecordingOutput")?;
        let delegate = screencapturekit::stream::ErrorHandler::new(move |e| {
            eprintln!("kirei: captura encerrada pelo sistema: {e}");
            e2.store(true, Ordering::Relaxed);
        });
        let stream =
            SCStream::new_with_delegate(filter, config, delegate).map_err(|e| anyhow!("{e}"))?;
        stream
            .add_recording_output(&output)
            .map_err(|e| anyhow!("{e}"))?;
        stream.start_capture().map_err(|e| anyhow!("{e}"))?;
        Ok(Self {
            stream,
            output,
            path,
            failed,
            ended,
        })
    }

    /// Stops and waits until the file is finalized. `None` when nothing usable was written.
    pub fn stop(self) -> Option<std::path::PathBuf> {
        if let Err(e) = self.stream.remove_recording_output(&self.output) {
            eprintln!("kirei: parar gravação: {e}");
        }
        self.stream.stop_capture().ok();
        let size = std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        (size > 0 && !self.failed.load(std::sync::atomic::Ordering::Relaxed)).then_some(self.path)
    }
}
