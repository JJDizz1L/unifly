//! Graphics protocol probing and optional true-pixel chart rendering.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock, mpsc};
use std::thread;

use image::DynamicImage;
use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};
use ratatui::widgets::Widget;
use ratatui_image::protocol::Protocol;
use ratatui_image::{Image, Resize, picker::Picker, picker::ProtocolType};

use crate::tui::render_caps::GraphicsProtocol;

const CHART_CACHE_CAPACITY: usize = 48;
const CHART_QUEUE_CAPACITY: usize = 8;

static PICKER: OnceLock<RwLock<Option<Picker>>> = OnceLock::new();
static CHARTS: OnceLock<ChartManager> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChartImageKey(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChartSlotKey(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachedChart {
    Rendered,
    Stale(CachedChartStatus),
    Missing,
    Pending,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachedChartStatus {
    Missing,
    Pending,
    Failed,
}

struct ChartRequest {
    slot: ChartSlotKey,
    key: ChartImageKey,
    picker: Picker,
    image: DynamicImage,
    target: Size,
}

struct ChartResponse {
    slot: ChartSlotKey,
    key: ChartImageKey,
    result: Result<Protocol, String>,
}

struct ChartManager {
    tx: mpsc::SyncSender<ChartRequest>,
    rx: Mutex<mpsc::Receiver<ChartResponse>>,
    cache: Mutex<ChartCache>,
    ready: Arc<AtomicBool>,
}

#[derive(Default)]
struct ChartCache {
    ready: HashMap<ChartImageKey, Arc<Protocol>>,
    latest_by_slot: HashMap<ChartSlotKey, ChartImageKey>,
    pending: HashSet<ChartImageKey>,
    failed: HashSet<ChartImageKey>,
    order: VecDeque<ChartImageKey>,
}

pub fn probe_stdio() -> GraphicsProtocol {
    if std::env::var_os("UNIFLY_DISABLE_GRAPHICS").is_some() {
        store_picker(None);
        return GraphicsProtocol::None;
    }

    match Picker::from_query_stdio() {
        Ok(mut picker) => {
            if let Some(protocol) = forced_protocol_from_env() {
                if let Some(protocol_type) = picker_protocol_from_graphics(protocol) {
                    picker.set_protocol_type(protocol_type);
                    store_picker(Some(picker));
                } else {
                    store_picker(None);
                }
                return protocol;
            }

            let protocol = protocol_from_picker(picker.protocol_type());
            if protocol.is_pixels() {
                store_picker(Some(picker));
            } else {
                store_picker(None);
            }
            protocol
        }
        Err(error) => {
            tracing::debug!("graphics protocol query failed: {error}");
            store_picker(None);
            GraphicsProtocol::None
        }
    }
}

pub fn current_picker() -> Option<Picker> {
    PICKER
        .get()
        .and_then(|lock| lock.read().ok().and_then(|guard| guard.clone()))
}

pub fn has_ready_chart() -> bool {
    CHARTS
        .get()
        .is_some_and(|manager| manager.ready.load(Ordering::SeqCst))
}

pub fn render_cached_chart(
    slot: ChartSlotKey,
    key: ChartImageKey,
    area: Rect,
    buf: &mut Buffer,
) -> CachedChart {
    let Some(manager) = CHARTS.get() else {
        return CachedChart::Missing;
    };
    manager.drain_responses();

    let Ok(cache) = manager.cache.lock() else {
        return CachedChart::Failed;
    };

    if let Some(protocol) = cache.ready.get(&key).cloned() {
        Image::new(protocol.as_ref())
            .allow_clipping(true)
            .render(area, buf);
        return CachedChart::Rendered;
    }

    let exact_status = if cache.pending.contains(&key) {
        CachedChartStatus::Pending
    } else if cache.failed.contains(&key) {
        CachedChartStatus::Failed
    } else {
        CachedChartStatus::Missing
    };

    if let Some(protocol) = cache
        .latest_by_slot
        .get(&slot)
        .and_then(|latest_key| cache.ready.get(latest_key))
        .cloned()
    {
        Image::new(protocol.as_ref())
            .allow_clipping(true)
            .render(area, buf);
        return CachedChart::Stale(exact_status);
    }

    match exact_status {
        CachedChartStatus::Missing => CachedChart::Missing,
        CachedChartStatus::Pending => CachedChart::Pending,
        CachedChartStatus::Failed => CachedChart::Failed,
    }
}

pub fn queue_chart(
    slot: ChartSlotKey,
    key: ChartImageKey,
    image: DynamicImage,
    target: Size,
) -> bool {
    let Some(picker) = current_picker() else {
        return false;
    };
    let manager = CHARTS.get_or_init(ChartManager::spawn);
    manager.queue(ChartRequest {
        slot,
        key,
        picker,
        image,
        target,
    })
}

fn store_picker(picker: Option<Picker>) {
    let lock = PICKER.get_or_init(|| RwLock::new(picker.clone()));
    if let Ok(mut guard) = lock.write() {
        *guard = picker;
    }
}

fn forced_protocol_from_env() -> Option<GraphicsProtocol> {
    let value = std::env::var("UNIFLY_GRAPHICS_PROTOCOL").ok()?;
    match value.trim().to_ascii_lowercase().as_str() {
        "kitty" => Some(GraphicsProtocol::Kitty),
        "sixel" | "sixels" => Some(GraphicsProtocol::Sixel),
        "iterm2" | "iterm" => Some(GraphicsProtocol::Iterm2),
        "off" | "none" | "false" | "0" => Some(GraphicsProtocol::None),
        _ => None,
    }
}

fn protocol_from_picker(protocol: ProtocolType) -> GraphicsProtocol {
    match protocol {
        ProtocolType::Kitty => GraphicsProtocol::Kitty,
        ProtocolType::Sixel => GraphicsProtocol::Sixel,
        ProtocolType::Iterm2 => GraphicsProtocol::Iterm2,
        ProtocolType::Halfblocks => GraphicsProtocol::None,
    }
}

fn picker_protocol_from_graphics(protocol: GraphicsProtocol) -> Option<ProtocolType> {
    match protocol {
        GraphicsProtocol::None => None,
        GraphicsProtocol::Kitty => Some(ProtocolType::Kitty),
        GraphicsProtocol::Sixel => Some(ProtocolType::Sixel),
        GraphicsProtocol::Iterm2 => Some(ProtocolType::Iterm2),
    }
}

impl ChartManager {
    fn spawn() -> Self {
        let (tx, rx_worker) = mpsc::sync_channel::<ChartRequest>(CHART_QUEUE_CAPACITY);
        let (tx_main, rx) = mpsc::sync_channel::<ChartResponse>(CHART_QUEUE_CAPACITY);
        let ready = Arc::new(AtomicBool::new(false));
        let ready_for_worker = Arc::clone(&ready);

        if let Err(error) = thread::Builder::new()
            .name("unifly-chart-graphics".to_string())
            .spawn(move || {
                while let Ok(request) = rx_worker.recv() {
                    let result = request
                        .picker
                        .new_protocol(request.image, request.target, Resize::Fit(None))
                        .map_err(|error| error.to_string());
                    if tx_main
                        .send(ChartResponse {
                            slot: request.slot,
                            key: request.key,
                            result,
                        })
                        .is_err()
                    {
                        break;
                    }
                    ready_for_worker.store(true, Ordering::SeqCst);
                }
            })
        {
            tracing::debug!("graphics chart worker failed to start: {error}");
        }

        Self {
            tx,
            rx: Mutex::new(rx),
            cache: Mutex::new(ChartCache::default()),
            ready,
        }
    }

    fn queue(&self, request: ChartRequest) -> bool {
        let key = request.key;
        {
            let Ok(mut cache) = self.cache.lock() else {
                return false;
            };
            if cache.ready.contains_key(&key) || cache.pending.contains(&key) {
                return true;
            }
            cache.failed.remove(&key);
        }

        match self.tx.try_send(request) {
            Ok(()) => {
                if let Ok(mut cache) = self.cache.lock() {
                    cache.pending.insert(key);
                }
                true
            }
            Err(mpsc::TrySendError::Full(_)) => false,
            Err(mpsc::TrySendError::Disconnected(_)) => {
                if let Ok(mut cache) = self.cache.lock() {
                    cache.failed.insert(key);
                }
                false
            }
        }
    }

    fn drain_responses(&self) {
        let Ok(rx) = self.rx.lock() else {
            return;
        };
        let mut saw_response = false;
        while let Ok(response) = rx.try_recv() {
            saw_response = true;
            if let Ok(mut cache) = self.cache.lock() {
                cache.pending.remove(&response.key);
                match response.result {
                    Ok(protocol) => {
                        cache.insert_ready(response.slot, response.key, Arc::new(protocol));
                    }
                    Err(error) => {
                        tracing::debug!(
                            key = response.key.0,
                            "graphics chart encode failed: {error}"
                        );
                        cache.ready.remove(&response.key);
                        cache.failed.insert(response.key);
                    }
                }
            }
        }

        if saw_response {
            self.ready.store(false, Ordering::SeqCst);
        }
    }
}

impl ChartCache {
    fn insert_ready(&mut self, slot: ChartSlotKey, key: ChartImageKey, protocol: Arc<Protocol>) {
        self.failed.remove(&key);
        if !self.ready.contains_key(&key) {
            self.order.push_back(key);
        }
        self.ready.insert(key, protocol);
        self.latest_by_slot.insert(slot, key);

        while self.ready.len() > CHART_CACHE_CAPACITY {
            let Some(expired) = self.order.pop_front() else {
                break;
            };
            if expired != key {
                self.ready.remove(&expired);
                self.latest_by_slot
                    .retain(|_, latest_key| *latest_key != expired);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{picker_protocol_from_graphics, protocol_from_picker};
    use crate::tui::render_caps::GraphicsProtocol;

    #[test]
    fn picker_protocol_maps_to_pixel_caps() {
        assert_eq!(
            protocol_from_picker(ratatui_image::picker::ProtocolType::Kitty),
            GraphicsProtocol::Kitty
        );
        assert_eq!(
            protocol_from_picker(ratatui_image::picker::ProtocolType::Sixel),
            GraphicsProtocol::Sixel
        );
        assert_eq!(
            protocol_from_picker(ratatui_image::picker::ProtocolType::Halfblocks),
            GraphicsProtocol::None
        );
    }

    #[test]
    fn graphics_protocol_maps_back_to_picker_protocol() {
        assert_eq!(
            picker_protocol_from_graphics(GraphicsProtocol::Kitty),
            Some(ratatui_image::picker::ProtocolType::Kitty)
        );
        assert_eq!(picker_protocol_from_graphics(GraphicsProtocol::None), None);
    }
}
