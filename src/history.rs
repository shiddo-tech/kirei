use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};
use uuid::Uuid;

use crate::settings;

const LIMIT: usize = 60;
const MAX_AGE: f64 = 14. * 24. * 3600.;
/// Seconds between 1970 and 2001, the reference date of Swift's default `Date` encoding.
const APPLE_EPOCH: f64 = 978_307_200.;

/// Same JSON as the Swift app's `HistoryItem`, so both read the same folder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HistoryItem {
    pub id: Uuid,
    /// Seconds since 2001-01-01, like Swift's `Date` encoding.
    pub date: f64,
    pub scale: f64,
    pub width: u32,
    pub height: u32,
}

impl HistoryItem {
    pub fn unix_time(&self) -> f64 {
        self.date + APPLE_EPOCH
    }
}

pub fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.)
        - APPLE_EPOCH
}

pub fn folder() -> PathBuf {
    settings::support_dir().join("History")
}

fn index_path() -> PathBuf {
    folder().join("index.json")
}

pub fn file_path(item: &HistoryItem) -> PathBuf {
    folder().join(format!("{}.png", item.id.to_string().to_uppercase()))
}

pub fn thumb_path(item: &HistoryItem) -> PathBuf {
    folder().join(format!("{}-thumb.jpg", item.id.to_string().to_uppercase()))
}

fn store() -> &'static Mutex<Vec<HistoryItem>> {
    static STORE: OnceLock<Mutex<Vec<HistoryItem>>> = OnceLock::new();
    STORE.get_or_init(|| {
        std::fs::create_dir_all(folder()).ok();
        let mut items: Vec<HistoryItem> = std::fs::read(index_path())
            .ok()
            .and_then(|d| serde_json::from_slice(&d).ok())
            .unwrap_or_default();
        prune(&mut items, now());
        Mutex::new(items)
    })
}

fn save_index(items: &[HistoryItem]) {
    if let Ok(d) = serde_json::to_vec(items) {
        let tmp = index_path().with_extension("json.tmp");
        if std::fs::write(&tmp, d).is_ok() {
            std::fs::rename(tmp, index_path()).ok();
        }
    }
}

fn delete_files(item: &HistoryItem) {
    std::fs::remove_file(file_path(item)).ok();
    std::fs::remove_file(thumb_path(item)).ok();
}

/// Keeps the newest `LIMIT` items from the last 14 days.
fn prune(items: &mut Vec<HistoryItem>, now: f64) {
    let cutoff = now - MAX_AGE;
    let mut kept = Vec::new();
    let mut dropped = Vec::new();
    for (i, item) in items.drain(..).enumerate() {
        if i < LIMIT && item.date > cutoff {
            kept.push(item);
        } else {
            dropped.push(item);
        }
    }
    *items = kept;
    if !dropped.is_empty() {
        dropped.iter().for_each(delete_files);
        save_index(items);
    }
}

pub fn items() -> Vec<HistoryItem> {
    store().lock().map(|s| s.clone()).unwrap_or_default()
}

/// Saves the original and a small thumbnail; call off the main thread.
pub fn add(pm: &Pixmap, scale: f64) -> Option<HistoryItem> {
    if !settings::get().keep_history {
        return None;
    }
    let item = HistoryItem {
        id: Uuid::new_v4(),
        date: now(),
        scale,
        width: pm.width(),
        height: pm.height(),
    };
    std::fs::create_dir_all(folder()).ok()?;
    crate::export::save(pm, scale, &file_path(&item), settings::Format::Png, true).ok()?;
    if let Some(t) = thumbnail(pm, 480.)
        && let Ok(jpg) = crate::export::jpeg(&t)
    {
        std::fs::write(thumb_path(&item), jpg).ok();
    }
    let mut items = store().lock().ok()?;
    items.insert(0, item.clone());
    let t = now();
    prune(&mut items, t);
    save_index(&items);
    Some(item)
}

pub fn load(item: &HistoryItem) -> Option<Pixmap> {
    crate::render::load_image_file(&file_path(item).to_string_lossy())
}

pub fn remove(id: Uuid) {
    if let Ok(mut items) = store().lock() {
        if let Some(item) = items.iter().find(|i| i.id == id) {
            delete_files(item);
        }
        items.retain(|i| i.id != id);
        save_index(&items);
    }
}

pub fn clear() {
    if let Ok(mut items) = store().lock() {
        items.iter().for_each(delete_files);
        items.clear();
        save_index(&items);
    }
}

fn thumbnail(pm: &Pixmap, max_side: f32) -> Option<Pixmap> {
    let s = (max_side / pm.width().max(pm.height()) as f32).min(1.);
    let (w, h) = (
        ((pm.width() as f32 * s) as u32).max(1),
        ((pm.height() as f32 * s) as u32).max(1),
    );
    let mut out = Pixmap::new(w, h)?;
    out.fill(tiny_skia::Color::WHITE);
    out.draw_pixmap(
        0,
        0,
        pm.as_ref(),
        &PixmapPaint {
            quality: FilterQuality::Bicubic,
            ..Default::default()
        },
        Transform::from_scale(w as f32 / pm.width() as f32, h as f32 / pm.height() as f32),
        None,
    );
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(age_days: f64) -> HistoryItem {
        HistoryItem {
            id: Uuid::new_v4(),
            date: now() - age_days * 24. * 3600.,
            scale: 2.,
            width: 10,
            height: 10,
        }
    }

    #[test]
    fn prunes_by_age_and_count() {
        let mut items: Vec<HistoryItem> = (0..70).map(|_| item(1.)).collect();
        items.insert(3, item(20.));
        prune(&mut items, now());
        // Like the Swift app, an expired item still takes one of the 60 slots before it goes.
        assert_eq!(items.len(), LIMIT - 1);
        assert!(items.iter().all(|i| now() - i.date < MAX_AGE));
    }

    #[test]
    fn reads_swift_index() {
        let json = r#"[{"id":"0B3B5A5E-2E4E-4B8F-9E59-3A1F0C2D7B11","date":785000000.5,"scale":2,"width":2880,"height":1800}]"#;
        let items: Vec<HistoryItem> = serde_json::from_str(json).unwrap();
        assert_eq!(items[0].width, 2880);
        assert!((items[0].unix_time() - 1_763_307_200.5).abs() < 1.);
    }
}
