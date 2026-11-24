#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Seoran: a fast, private browser. Rust shell, Chromium engine (WebView2 for
//! now, CEF later), built-in ad and tracker blocking.

mod engine;
mod filters;
mod layout;
mod nav;
mod pages;
mod store;
mod tabs;

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, Weak},
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use shield::Shield;
use tao::{
    dpi::{LogicalPosition, LogicalSize},
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget},
    window::{Icon, ResizeDirection, Window, WindowBuilder},
};
use wry::{Rect, WebContext, WebView, WebViewBuilder, http::Request};

use crate::{
    engine::ContentSpec,
    layout::Layout,
    store::{SessionTab, Store},
    tabs::{Tab, Tabs},
};

const MAINTENANCE_EVERY: Duration = Duration::from_secs(30);
const FLUSH_EVERY: Duration = Duration::from_millis(80);

pub enum UserEvent {
    Chrome(String),
    Content { tab: u32, origin: String, body: String },
    Title(u32, String),
    Load { tab: u32, started: bool, url: String },
    OpenTab { url: String, background: bool },
    Blocked(u32),
    NavStarting { tab: u32, url: String },
    Url { tab: u32, url: String },
    History(u32),
    Favicon { tab: u32, url: String },
    Filters(filters::Compiled),
    /// A browser command (from a key the engine caught, for example).
    Action(String),
    /// A message from the DevTools panel.
    DevTools(String),
    CdpResult { id: u64, ok: bool, json: String },
    CdpEvent { tab: u32, name: String, json: String },
    /// Right-click > Inspect at this point of the page.
    Inspect { tab: u32, x: i32, y: i32 },
}

/// Page callbacks may already be on the event queue when their WebView is dropped.
struct QueuedEvent {
    source: Option<Weak<()>>,
    event: UserEvent,
}

fn source_is_live(source: &Option<Weak<()>>) -> bool {
    source.as_ref().is_none_or(|owner| owner.strong_count() > 0)
}

impl QueuedEvent {
    fn into_live(self) -> Option<UserEvent> {
        source_is_live(&self.source).then_some(self.event)
    }
}

#[derive(Clone)]
struct EventSender {
    queue: EventLoopProxy<QueuedEvent>,
    source: Option<Weak<()>>,
}

impl EventSender {
    fn for_page(&self, lifetime: &Arc<()>) -> Self {
        Self { queue: self.queue.clone(), source: Some(Arc::downgrade(lifetime)) }
    }

    fn send(&self, event: UserEvent) {
        let _ = self.queue.send_event(QueuedEvent { source: self.source.clone(), event });
    }
}

const DEVTOOLS_MIN: f64 = 140.0;

/// Native resources for one resident tab. Dropping this entry puts its page to sleep.
struct LivePage {
    lifetime: Arc<()>,
    view: WebView,
    cdp_subs: HashSet<String>,
}

fn register_subscription(
    registered: &mut HashSet<String>,
    name: String,
    register: impl FnOnce(&str) -> windows::core::Result<()>,
) {
    if !registered.contains(&name) && register(&name).is_ok() {
        registered.insert(name);
    }
}

struct Browser {
    window: Window,
    chrome: WebView,
    ctx: WebContext,
    proxy: EventSender,
    shield: Rc<RefCell<Shield>>,
    store: Store,
    tab_layout: Layout,
    tabs: Tabs,
    pages: HashMap<u32, LivePage>,
    next_id: u32,
    closed: Vec<SessionTab>,
    /// Extra areas (menus, popovers, toasts) the UI view currently shows
    /// over the page, in logical pixels: x, y, w, h, corner radius.
    panels: Vec<[f64; 5]>,
    devtools: DevTools,
    dirty: bool,
    last_flush: Instant,
    chrome_ready: bool,
    last_maintenance: Instant,
}

/// The docked DevTools panel: its own view, attached to one tab at a time.
#[derive(Default)]
struct DevTools {
    view: Option<WebView>,
    open: bool,
    ready: bool,
    height: f64,
    /// The tab the panel is inspecting.
    tab: Option<u32>,
    /// Calls to make once the panel page has loaded.
    queued: Vec<(String, Value)>,
}

/// Window background for the current theme, so nothing ever flashes a
/// different colour while a view paints.
fn theme_bg(theme: &str, window: &Window) -> (u8, u8, u8, u8) {
    let dark = match theme {
        "dark" => true,
        "light" => false,
        _ => window.theme() == tao::window::Theme::Dark,
    };
    if dark { (20, 19, 23, 255) } else { (243, 240, 234, 255) }
}

/// The Seoran mark (grape square, white S, lime dot), drawn in code at
/// 64x64 so the window icon needs no image file.
fn app_icon() -> Option<Icon> {
    let size = 64u32;
    let s = size as f32 / 32.0; // drawn on the logo's 32-unit grid
    // Distance from p to the S: two half-circle arcs joined by straight bars.
    let dist_s = |x: f32, y: f32| -> f32 {
        let seg = |ax: f32, ay: f32, bx: f32, by: f32| {
            let (dx, dy) = (bx - ax, by - ay);
            let t = (((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            (x - ax - t * dx).hypot(y - ay - t * dy)
        };
        let arc = |cx: f32, cy: f32, left: bool| {
            let d = ((x - cx).hypot(y - cy) - 3.25).abs();
            if (x <= cx) == left { d } else { f32::MAX }
        };
        seg(21.5, 9.5, 13.25, 9.5)
            .min(arc(13.25, 12.75, true))
            .min(seg(13.25, 16.0, 18.75, 16.0))
            .min(arc(18.75, 19.25, false))
            .min(seg(18.75, 22.5, 10.5, 22.5))
    };
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let (x, y) = ((px as f32 + 0.5) / s, (py as f32 + 0.5) / s);
            let (fx, fy) = (x - 16.0, y - 16.0);
            let corner = (fx.abs() - 7.0).max(0.0).hypot((fy.abs() - 7.0).max(0.0));
            let pixel = if corner > 9.0 {
                [0, 0, 0, 0]
            } else if (x - 24.5).hypot(y - 22.5) <= 2.4 {
                [166, 226, 46, 255]
            } else if dist_s(x, y) <= 1.8 {
                [255, 255, 255, 255]
            } else {
                [109, 53, 217, 255]
            };
            rgba.extend_from_slice(&pixel);
        }
    }
    Icon::from_rgba(rgba, size, size).ok()
}

impl Browser {
    fn new(target: &EventLoopWindowTarget<QueuedEvent>, proxy: EventSender) -> Self {
        let base = dirs::data_local_dir().unwrap_or_else(std::env::temp_dir);
        let profile = base.join("Seoran");
        // The browser used to be called Kite: carry an old profile over once.
        let old = base.join(concat!("K", "ite"));
        if !profile.exists() && old.exists() {
            let _ = std::fs::rename(&old, &profile);
        }
        let p_filters = proxy.clone();
        let browser = Self::with_profile(target, proxy, profile.clone(), std::env::args().nth(1));
        filters::spawn(profile, false, move |c| {
            p_filters.send(UserEvent::Filters(c));
        });
        browser
    }

    fn with_profile(
        target: &EventLoopWindowTarget<QueuedEvent>,
        proxy: EventSender,
        profile: std::path::PathBuf,
        arg_url: Option<String>,
    ) -> Self {
        let store = Store::open(profile.clone());
        let tab_layout = Layout::read(&profile).unwrap_or_else(|e| {
            eprintln!("[seoran] using the default tab layout: {e}");
            Layout::default()
        });

        let window = WindowBuilder::new()
            .with_title("Seoran")
            .with_decorations(false)
            .with_inner_size(LogicalSize::new(1280.0, 820.0))
            .with_min_inner_size(LogicalSize::new(520.0, 360.0))
            .with_window_icon(app_icon())
            .with_visible(false)
            .build(target)
            .expect("create window");
        {
            use tao::platform::windows::WindowExtWindows;
            window.set_undecorated_shadow(true);
        }
        let bg = theme_bg(&store.settings.theme, &window);
        window.set_background_color(Some(bg));

        let mut ctx = WebContext::new(Some(profile.join("WebView")));

        // The UI view always covers the whole window; a window region clips
        // it to the toolbar plus whatever menu is open (see `apply_region`).
        let p_chrome = proxy.clone();
        let size = window.inner_size().to_logical::<f64>(window.scale_factor());
        let chrome = WebViewBuilder::new_with_web_context(&mut ctx)
            .with_url(nav::internal_url("chrome"))
            .with_bounds(Rect {
                position: LogicalPosition::new(0.0, 0.0).into(),
                size: LogicalSize::new(size.width, size.height).into(),
            })
            .with_background_color(bg)
            .with_devtools(cfg!(debug_assertions))
            .with_hotkeys_zoom(false)
            .with_custom_protocol("seoran".into(), |_id, req| engine::serve_internal(req))
            .with_ipc_handler(move |req: Request<String>| {
                // Only our own UI page may drive the browser.
                if nav::internal_page(&req.uri().to_string()) == Some("chrome") {
                    p_chrome.send(UserEvent::Chrome(req.into_body()));
                }
            })
            .with_navigation_handler(|url| nav::internal_page(&url) == Some("chrome"))
            .build_as_child(&window)
            .expect("create browser UI");
        engine::lock_down_ui(&chrome);

        let mut shield = Shield::new();
        shield.set_enabled(store.settings.shields_enabled);
        shield.set_blocked_total(store.settings.blocked_total);
        for site in &store.settings.allowed_sites {
            shield.set_site_allowed(site, true);
        }

        let mut browser = Self {
            window,
            chrome,
            ctx,
            proxy: proxy.clone(),
            shield: Rc::new(RefCell::new(shield)),
            store,
            tab_layout,
            tabs: Tabs::default(),
            pages: HashMap::new(),
            next_id: 1,
            closed: Vec::new(),
            panels: Vec::new(),
            devtools: DevTools { height: 320.0, ..Default::default() },
            dirty: true,
            last_flush: Instant::now(),
            chrome_ready: false,
            last_maintenance: Instant::now(),
        };

        // Restore the last session lazily: only the active tab loads now.
        let session = browser.store.load_session();
        if browser.store.settings.restore_session {
            browser.tabs = Tabs::from_session(&session, || browser.alloc_id());
        }
        if let Some(url) = arg_url {
            let url = nav::resolve_input(&url, &browser.store.settings);
            browser.open_tab(&url, true);
        } else if browser.tabs.ordered.is_empty() {
            browser.open_tab(nav::NEW_TAB_URL, true);
        } else if let Some(i) = browser.tabs.active_index() {
            browser.activate(i);
        }

        browser.layout();
        // Show the window only once its views exist: no blank first frame.
        browser.window.set_visible(true);

        browser
    }

    fn bg(&self) -> (u8, u8, u8, u8) {
        theme_bg(&self.store.settings.theme, &self.window)
    }

    fn apply_theme(&self) {
        let bg = self.bg();
        self.window.set_background_color(Some(bg));
        let _ = self.chrome.set_background_color(bg);
        if let Some(view) = &self.devtools.view {
            let _ = view.set_background_color(bg);
        }
        for page in self.pages.values() {
            let _ = page.view.set_background_color(bg);
        }
    }

    /// Clip the UI view to the toolbar plus open panels.
    fn apply_region(&self) -> std::io::Result<()> {
        let scale = self.window.scale_factor();
        let s = self.logical_size();
        let px = |v: f64| (v * scale).round() as i32;
        let header = px(self.tab_layout.header_height());
        let rail = px(self.tab_layout.rail_width());
        let mut rects = vec![engine::RegionRect { x: 0, y: 0, w: px(s.width), h: header, r: 0 }];
        if rail > 0 {
            rects.push(engine::RegionRect { x: 0, y: header, w: rail, h: (px(s.height) - header).max(0), r: 0 });
        }
        rects.extend(self.panels.iter().map(|p| engine::RegionRect {
            x: px(p[0]),
            y: px(p[1]),
            w: px(p[2]),
            h: px(p[3]),
            r: px(p[4]),
        }));
        engine::set_region(&self.chrome, &rects)
    }

    fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn logical_size(&self) -> LogicalSize<f64> {
        self.window.inner_size().to_logical(self.window.scale_factor())
    }

    /// Height the DevTools panel takes at the bottom of the window.
    fn devtools_height(&self) -> f64 {
        if !self.devtools.open {
            return 0.0;
        }
        let room = self.logical_size().height - self.tab_layout.header_height() - 80.0;
        self.devtools.height.min(room).max(DEVTOOLS_MIN.min(room)).max(0.0)
    }

    fn content_bounds(&self) -> Rect {
        let s = self.logical_size();
        let header = self.tab_layout.header_height();
        let rail = self.tab_layout.rail_width();
        Rect {
            position: LogicalPosition::new(rail, header).into(),
            size: LogicalSize::new((s.width - rail).max(1.0), (s.height - header - self.devtools_height()).max(1.0)).into(),
        }
    }

    fn devtools_bounds(&self) -> Rect {
        let s = self.logical_size();
        let h = self.devtools_height();
        let rail = self.tab_layout.rail_width();
        Rect {
            position: LogicalPosition::new(rail, s.height - h).into(),
            size: LogicalSize::new((s.width - rail).max(1.0), h.max(1.0)).into(),
        }
    }

    fn chrome_bounds(&self) -> Rect {
        let s = self.logical_size();
        Rect {
            position: LogicalPosition::new(0.0, 0.0).into(),
            size: LogicalSize::new(s.width, s.height).into(),
        }
    }

    fn layout(&self) {
        if let Err(error) = self.apply_bounds() {
            eprintln!("[seoran] layout: {error}");
        }
    }

    fn apply_bounds(&self) -> wry::Result<()> {
        self.chrome.set_bounds(self.chrome_bounds())?;
        if let Some(wv) = self.active_view() {
            wv.set_bounds(self.content_bounds())?;
        }
        if let Some(view) = &self.devtools.view {
            view.set_bounds(self.devtools_bounds())?;
        }
        self.apply_region()?;
        Ok(())
    }

    fn change_layout(&mut self, candidate: std::io::Result<Layout>, save: bool) {
        let result = candidate.and_then(|layout| {
            self.tab_layout = layout;
            self.apply_bounds().map_err(std::io::Error::other)?;
            if save {
                layout.save(self.store.dir())?;
            }
            Ok(())
        });
        if let Err(error) = result {
            eprintln!("[seoran] tab layout: {error}");
            self.tab_layout = Layout::default();
            self.layout();
            self.chrome_call("toast", json!({ "text": "Could not apply the tab layout. Using top tabs." }));
        }
        self.chrome_call("state", self.state_json());
    }

    fn active_view(&self) -> Option<&WebView> {
        self.pages.get(&self.tabs.active_id()?).map(|page| &page.view)
    }

    fn ensure_webview(&mut self, i: usize, visible: bool) {
        if self.pages.contains_key(&self.tabs.ordered[i].id) {
            return;
        }
        let bounds = self.content_bounds();
        let tab = &self.tabs.ordered[i];
        let lifetime = Arc::new(());
        let sender = self.proxy.for_page(&lifetime);
        let spec = ContentSpec { tab: tab.id, url: &tab.url, bounds, visible };
        let bg = self.bg();
        match engine::create_content(&self.window, &mut self.ctx, spec, bg, &sender, &self.shield, &tab.hooks) {
            Ok(wv) => {
                self.pages.insert(tab.id, LivePage { lifetime, view: wv, cdp_subs: HashSet::new() });
                self.tabs.ordered[i].loading = true;
                // New views stack on top; keep the UI above the page.
                engine::raise(&self.chrome);
            }
            Err(e) => eprintln!("[seoran] could not create tab: {e}"),
        }
    }

    fn open_tab(&mut self, url: &str, activate: bool) {
        let id = self.alloc_id();
        let insert_at = self.tabs.active_index().map_or(0, |i| i + 1);
        self.tabs.ordered.insert(insert_at, Tab::new(id, url, ""));
        if activate {
            self.activate(insert_at);
            if nav::internal_page(url) == Some("newtab") {
                self.chrome_call("focusOmnibox", json!({}));
            }
        } else {
            // Background tabs (links opened in a new tab) start loading now.
            self.ensure_webview(insert_at, false);
        }
        self.dirty = true;
    }

    fn activate(&mut self, i: usize) {
        let prev = self.tabs.active_index();
        if !self.tabs.select(i) {
            return;
        }
        self.ensure_webview(i, true);
        let bounds = self.content_bounds();
        let tab = &mut self.tabs.ordered[i];
        tab.last_active = Instant::now();
        // Show the new page before hiding the old one, so there is never a
        // frame with no page at all.
        if let Some(page) = self.pages.get(&tab.id) {
            let _ = page.view.set_bounds(bounds);
            let _ = page.view.set_visible(true);
            let _ = page.view.focus();
        }
        if let Some(prev) = prev.filter(|&prev| prev != i) {
            let old = &mut self.tabs.ordered[prev];
            old.last_active = Instant::now();
            if let Some(page) = self.pages.get(&old.id) {
                let _ = page.view.set_visible(false);
            }
        }
        self.refresh_nav(i);
        self.update_window_title();
        if self.devtools.open {
            self.devtools_attach();
        }
        self.dirty = true;
    }

    fn close_tab(&mut self, id: u32) {
        let was_active = self.tabs.active_id() == Some(id);
        let Some(tab) = self.tabs.remove(id) else { return };
        self.pages.remove(&id);
        if self.devtools.tab == Some(id) {
            self.devtools.tab = None;
        }
        if nav::internal_page(&tab.url).is_none_or(|p| p != "newtab") {
            self.closed.push(SessionTab { url: tab.url.clone(), title: tab.title.clone() });
            if self.closed.len() > 25 {
                self.closed.remove(0);
            }
            self.chrome_call(
                "toast",
                json!({ "text": format!("Closed “{}”", tab.display_title()), "action": "reopen", "actionLabel": "Undo" }),
            );
        }
        if !was_active {
            self.dirty = true;
        } else if let Some(next) = self.tabs.active_index() {
            self.activate(next);
        } else {
            self.open_tab(nav::NEW_TAB_URL, true);
        }
    }

    fn reopen_closed(&mut self) {
        if let Some(t) = self.closed.pop() {
            self.open_tab(&t.url, true);
        }
    }

    fn move_tab(&mut self, id: u32, to: usize) {
        if self.tabs.move_to(id, to) {
            self.dirty = true;
        }
    }

    fn navigate(&mut self, input: &str) {
        let url = nav::resolve_input(input, &self.store.settings);
        let Some(i) = self.tabs.active_index() else {
            self.open_tab(&url, true);
            return;
        };
        self.tabs.ordered[i].url = url.clone();
        match self.pages.get(&self.tabs.ordered[i].id) {
            Some(page) => {
                let _ = page.view.load_url(&url);
                let _ = page.view.focus();
            }
            None => self.ensure_webview(i, true),
        }
        self.tabs.ordered[i].loading = true;
        self.dirty = true;
    }

    fn refresh_nav(&mut self, i: usize) {
        if let Some(tab) = self.tabs.ordered.get_mut(i)
            && let Some(page) = self.pages.get(&tab.id) {
                let s = engine::nav_state(&page.view);
                tab.can_back = s.can_back;
                tab.can_forward = s.can_forward;
            }
    }

    fn update_window_title(&self) {
        if let Some(t) = self.active_tab() {
            self.window.set_title(&format!("{} — Seoran", t.display_title()));
        }
    }

    fn active_tab(&self) -> Option<&Tab> {
        self.tabs.ordered.get(self.tabs.active_index()?)
    }

    // ---- talking to the UI -------------------------------------------------

    fn chrome_call(&self, func: &str, arg: Value) {
        let js = format!("window.seoran && seoran.{func}({arg})");
        let _ = self.chrome.evaluate_script(&js);
    }

    fn page_reply(&self, tab: u32, msg: Value) {
        if let Some(page) = self.pages.get(&tab) {
            let _ = page.view.evaluate_script(&format!("window.__seoranReceive && __seoranReceive({msg})"));
        }
    }

    fn state_json(&self) -> Value {
        let shield = self.shield.borrow();
        let active = self.active_tab();
        let url = active.map(|t| t.url.as_str()).unwrap_or("");
        let site = shield::site_of(url).filter(|_| !nav::is_internal(url));
        json!({
            "tabs": self.tabs.ordered.iter().map(|t| t.chrome_state(self.pages.contains_key(&t.id))).collect::<Vec<_>>(),
            "active": self.tabs.active_id(),
            "tabPosition": self.tab_layout.tabs,
            "chromeHeight": self.tab_layout.header_height(),
            "railWidth": self.tab_layout.rail_width(),
            "url": nav::display_url(url),
            "secure": url.starts_with("https://"),
            "internal": nav::is_internal(url),
            "canBack": active.is_some_and(|t| t.can_back),
            "canForward": active.is_some_and(|t| t.can_forward),
            "loading": active.is_some_and(|t| t.loading),
            "bookmarked": self.store.is_bookmarked(url),
            "maximized": self.window.is_maximized(),
            "theme": self.store.settings.theme,
            "plastic": self.store.settings.plastic,
            "closedCount": self.closed.len(),
            "shield": {
                "enabled": shield.enabled(),
                "site": site,
                "siteActive": site.is_some() && shield.active_on(url),
                "blocked": active.map(|t| t.hooks.blocked.get()).unwrap_or(0),
                "total": shield.blocked_total(),
                "lists": shield.sources(),
            },
        })
    }

    /// Push state to the UI, at most every 80 ms: a page blocking dozens of
    /// requests a second must not make the toolbar re-render dozens of times.
    fn flush(&mut self) -> Option<Duration> {
        if !self.dirty || !self.chrome_ready {
            return None;
        }
        let since = self.last_flush.elapsed();
        if since < FLUSH_EVERY {
            return Some(FLUSH_EVERY - since);
        }
        self.dirty = false;
        self.last_flush = Instant::now();
        self.chrome_call("state", self.state_json());
        None
    }

    // ---- messages ----------------------------------------------------------

    fn suggestions(&self, q: &str) -> Value {
        let q = q.trim();
        let mut items = Vec::new();
        if q.is_empty() {
            return json!([]);
        }
        let target = nav::resolve_input(q, &self.store.settings);
        let is_search = target.starts_with(&self.store.settings.search_url(""));
        items.push(json!({
            "kind": if is_search { "search" } else { "url" },
            "title": q,
            "url": target,
            "sub": if is_search {
                let engine = store::SEARCH_ENGINES.iter().find(|e| e.0 == self.store.settings.search_engine).map(|e| e.1).unwrap_or("the web");
                format!("Search {engine}")
            } else { "Open address".into() },
        }));
        let ql = q.to_lowercase();
        for t in &self.tabs.ordered {
            if Some(t.id) != self.tabs.active_id()
                && (t.title.to_lowercase().contains(&ql) || t.url.to_lowercase().contains(&ql))
            {
                items.push(json!({ "kind": "tab", "title": t.display_title(), "url": nav::display_url(&t.url), "tabId": t.id, "sub": "Switch to tab" }));
            }
            if items.len() > 3 {
                break;
            }
        }
        for (url, title, bookmarked) in self.store.search(q, 7) {
            if items.iter().any(|i| i["url"] == url) {
                continue;
            }
            items.push(json!({
                "kind": if bookmarked { "bookmark" } else { "history" },
                "title": if title.is_empty() { url } else { title },
                "url": url,
                "sub": url,
            }));
        }
        items.truncate(8);
        Value::Array(items)
    }

    fn handle_action(&mut self, action: &str) {
        match action {
            "newTab" => self.open_tab(nav::NEW_TAB_URL, true),
            "closeTab" => {
                if let Some(id) = self.tabs.active_id() {
                    self.close_tab(id);
                }
            }
            "reopen" => self.reopen_closed(),
            "nextTab" | "prevTab" => {
                if let Some(i) = self.tabs.active_index() {
                    let count = self.tabs.ordered.len();
                    let step = if action == "nextTab" { 1 } else { count - 1 };
                    self.activate((i + step) % count);
                }
            }
            "focusOmnibox" => {
                let _ = self.chrome.focus();
                self.chrome_call("focusOmnibox", json!({}));
            }
            "palette" => {
                let _ = self.chrome.focus();
                self.chrome_call("openPalette", json!({}));
            }
            "bookmark" => self.toggle_bookmark(),
            "reload" => {
                if let Some(wv) = self.active_view() {
                    let _ = wv.reload();
                }
            }
            "settings" => self.open_tab(&nav::internal_url("settings"), true),
            "devtools" => {
                if self.devtools.open {
                    self.close_devtools();
                } else {
                    self.open_devtools(None);
                }
            }
            "devtoolsConsole" => self.open_devtools(Some(json!({ "view": "console" }))),
            "devtoolsPick" => self.open_devtools(Some(json!({ "view": "elements", "pick": true }))),
            "devtoolsAdvanced" => {
                if let Some(wv) = self.active_view() {
                    engine::open_devtools(wv);
                }
            }
            a if a.starts_with("tab:") => {
                if let Ok(n) = a[4..].parse::<usize>() {
                    let i = if n == 9 { self.tabs.ordered.len().saturating_sub(1) } else { n.saturating_sub(1) };
                    self.activate(i);
                }
            }
            _ => {}
        }
    }

    fn toggle_bookmark(&mut self) {
        let Some(t) = self.active_tab() else { return };
        if nav::is_internal(&t.url) {
            return;
        }
        let (url, title) = (t.url.clone(), t.display_title());
        self.store.toggle_bookmark(&url, &title);
        self.dirty = true;
    }

    /// The UI reports which panels it shows over the page.
    fn set_panels(&mut self, v: &Value) {
        let panels: Vec<[f64; 5]> = v
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|r| {
                        let n = |i: usize| r.get(i).and_then(Value::as_f64);
                        Some([n(0)?, n(1)?, n(2)?.clamp(0.0, 8000.0), n(3)?.clamp(0.0, 8000.0), n(4).unwrap_or(0.0)])
                    })
                    .take(8)
                    .collect()
            })
            .unwrap_or_default();
        if panels != self.panels {
            self.panels = panels;
            if let Err(error) = self.apply_region() {
                eprintln!("[seoran] chrome clipping: {error}");
            }
        }
    }

    fn focus_content(&self) {
        if let Some(wv) = self.active_view() {
            let _ = wv.focus();
        }
    }

    fn handle_chrome(&mut self, body: &str, target: &EventLoopWindowTarget<QueuedEvent>, flow: &mut ControlFlow) {
        let Ok(msg) = serde_json::from_str::<Value>(body) else { return };
        let id = msg["id"].as_u64().map(|v| v as u32);
        match msg["t"].as_str().unwrap_or("") {
            "ready" => {
                self.chrome_ready = true;
                self.dirty = true;
            }
            "action" => self.handle_action(msg["a"].as_str().unwrap_or("")),
            "openTab" => {
                let url = nav::resolve_input(msg["url"].as_str().unwrap_or(""), &self.store.settings);
                self.open_tab(&url, !msg["background"].as_bool().unwrap_or(false));
            }
            "closeTab" => {
                if let Some(id) = id {
                    self.close_tab(id);
                }
            }
            "activate" => {
                if let Some(i) = id.and_then(|id| self.tabs.index_of(id)) {
                    self.activate(i);
                }
            }
            "moveTab" => {
                if let (Some(id), Some(to)) = (id, msg["to"].as_u64()) {
                    self.move_tab(id, to as usize);
                }
            }
            "navigate" => self.navigate(msg["input"].as_str().unwrap_or("")),
            "back" | "forward" | "stop" => {
                if let Some(wv) = self.active_view() {
                    match msg["t"].as_str() {
                        Some("back") => engine::go_back(wv),
                        Some("forward") => engine::go_forward(wv),
                        _ => engine::stop(wv),
                    }
                }
            }
            "shieldSite" => {
                let on = msg["on"].as_bool().unwrap_or(true);
                if let Some(site) = self.active_tab().and_then(|t| shield::site_of(&t.url)) {
                    self.shield.borrow_mut().set_site_allowed(&site, !on);
                    let s = &mut self.store.settings.allowed_sites;
                    s.retain(|x| x != &site);
                    if !on {
                        s.push(site.clone());
                    }
                    self.store.save_settings();
                    self.handle_action("reload");
                }
                self.dirty = true;
            }
            "shieldGlobal" => {
                let on = msg["on"].as_bool().unwrap_or(true);
                self.shield.borrow_mut().set_enabled(on);
                self.store.settings.shields_enabled = on;
                self.store.save_settings();
                self.dirty = true;
            }
            "suggest" => {
                let items = self.suggestions(msg["q"].as_str().unwrap_or(""));
                self.chrome_call("suggestions", json!({ "seq": msg["seq"], "items": items }));
            }
            "history" => {
                let q = msg["q"].as_str().unwrap_or("");
                let items: Vec<Value> = self
                    .store
                    .search(if q.is_empty() { "." } else { q }, 12)
                    .into_iter()
                    .map(|(u, t, b)| json!({ "url": u, "title": t, "bookmark": b }))
                    .collect();
                self.chrome_call("historyResults", json!({ "seq": msg["seq"], "items": items }));
            }
            "setLayout" => self.change_layout(serde_json::from_value(msg["layout"].clone()).map_err(Into::into), true),
            "reloadLayout" => self.change_layout(Layout::read(self.store.dir()), false),
            "panels" => self.set_panels(&msg["rects"]),
            "focusContent" => self.focus_content(),
            "theme" => {
                if let Some(t) = msg["value"].as_str().filter(|t| ["system", "light", "dark"].contains(t)) {
                    self.store.settings.theme = t.into();
                    self.store.save_settings();
                    self.apply_theme();
                    self.dirty = true;
                }
            }
            "plastic" => {
                if let Some(p) = msg["value"].as_str().filter(|p| store::PLASTICS.contains(p)) {
                    self.store.settings.plastic = p.into();
                    self.store.save_settings();
                    self.broadcast_look();
                    self.dirty = true;
                }
            }
            "win" => match msg["a"].as_str().unwrap_or("") {
                "min" => self.window.set_minimized(true),
                "max" => {
                    self.window.set_maximized(!self.window.is_maximized());
                    self.dirty = true;
                }
                "close" => {
                    self.shutdown();
                    *flow = ControlFlow::Exit;
                }
                "drag" => {
                    let _ = self.window.drag_window();
                }
                "resizeTop" => {
                    let _ = self.window.drag_resize_window(ResizeDirection::North);
                }
                _ => {}
            },
            "updateFilters" => self.update_filters(),
            _ => {}
        }
        let _ = target;
    }

    fn update_filters(&mut self) {
        let p = self.proxy.clone();
        filters::spawn(self.store.dir().to_path_buf(), true, move |c| {
            p.send(UserEvent::Filters(c));
        });
    }

    fn handle_content(&mut self, tab: u32, origin: &str, body: &str) {
        let Ok(msg) = serde_json::from_str::<Value>(body) else { return };
        let Some(i) = self.tabs.index_of(tab) else { return };
        if msg["t"] == "key" {
            if self.tabs.ordered[i].allow_key() {
                let a = msg["a"].as_str().unwrap_or("").to_owned();
                self.handle_action(&a);
            }
            return;
        }
        // Everything else is only for our own internal pages.
        let Some(page) = nav::internal_page(origin) else { return };
        if !nav::is_internal(&self.tabs.ordered[i].url) {
            return;
        }
        match msg["t"].as_str().unwrap_or("") {
            "hello" => {
                let data = self.page_data(page);
                self.page_reply(tab, json!({ "t": "data", "data": data }));
            }
            "navigate" => {
                let input = msg["input"].as_str().unwrap_or("").to_owned();
                if self.tabs.active_id() == Some(tab) {
                    self.navigate(&input);
                }
            }
            "setting" if page == "settings" => {
                let v = &msg["value"];
                let s = &mut self.store.settings;
                match msg["key"].as_str().unwrap_or("") {
                    "search_engine" => {
                        if let Some(e) = v.as_str().filter(|e| store::SEARCH_ENGINES.iter().any(|x| x.0 == *e)) {
                            s.search_engine = e.into();
                        }
                    }
                    "theme" => {
                        if let Some(t) = v.as_str().filter(|t| ["system", "light", "dark"].contains(t)) {
                            s.theme = t.into();
                            self.apply_theme();
                        }
                    }
                    "plastic" => {
                        if let Some(p) = v.as_str().filter(|p| store::PLASTICS.contains(p)) {
                            s.plastic = p.into();
                        }
                    }
                    "restore_session" => s.restore_session = v.as_bool().unwrap_or(true),
                    "sleep_tabs_after_min" => s.sleep_tabs_after_min = v.as_u64().unwrap_or(20).min(240),
                    "shields_enabled" => {
                        s.shields_enabled = v.as_bool().unwrap_or(true);
                        self.shield.borrow_mut().set_enabled(s.shields_enabled);
                    }
                    _ => {}
                }
                self.store.save_settings();
                self.broadcast_look();
                self.dirty = true;
                let data = self.page_data(page);
                self.page_reply(tab, json!({ "t": "data", "data": data }));
            }
            "allowSite" if page == "settings" => {
                if let Some(site) = msg["site"].as_str() {
                    let allow = msg["allow"].as_bool().unwrap_or(false);
                    self.shield.borrow_mut().set_site_allowed(site, allow);
                    self.store.settings.allowed_sites.retain(|x| x != site);
                    if allow {
                        self.store.settings.allowed_sites.push(site.into());
                    }
                    self.store.save_settings();
                    let data = self.page_data(page);
                    self.page_reply(tab, json!({ "t": "data", "data": data }));
                }
            }
            "clearHistory" if page == "settings" => {
                self.store.clear_history();
                self.chrome_call("toast", json!({ "text": "History cleared" }));
                let data = self.page_data(page);
                self.page_reply(tab, json!({ "t": "data", "data": data }));
            }
            "updateFilters" if page == "settings" => self.update_filters(),
            "forgetSite" if page == "newtab" => {
                if let Some(site) = msg["site"].as_str() {
                    self.store.history.retain(|u, _| shield::site_of(u).as_deref() != Some(site));
                    self.store.save_history();
                    let data = self.page_data(page);
                    self.page_reply(tab, json!({ "t": "data", "data": data }));
                }
            }
            _ => {}
        }
    }

    /// Theme and plastic, for every seoran:// page.
    fn look(&self) -> Value {
        json!({ "theme": self.store.settings.theme, "plastic": self.store.settings.plastic })
    }

    /// Tell every open seoran:// page and the DevTools panel the new look.
    fn broadcast_look(&self) {
        let js = format!("window.__seoranLook && __seoranLook({})", self.look());
        for t in &self.tabs.ordered {
            if nav::is_internal(&t.url) && let Some(page) = self.pages.get(&t.id) {
                let _ = page.view.evaluate_script(&js);
            }
        }
        if let Some(view) = &self.devtools.view {
            let _ = view.evaluate_script(&js);
        }
    }

    fn page_data(&self, page: &str) -> Value {
        let shield = self.shield.borrow();
        let s = &self.store.settings;
        let engine_name = store::SEARCH_ENGINES.iter().find(|e| e.0 == s.search_engine).map(|e| e.1).unwrap_or("");
        match page {
            "newtab" => json!({
                "theme": s.theme, "plastic": s.plastic, "plastics": store::PLASTICS,
                "engine": engine_name,
                "blockedTotal": shield.blocked_total(),
                "shieldOn": shield.enabled(),
                "topSites": self.store.top_sites(8).iter().map(|h| json!({
                    "url": h.url,
                    "title": h.title,
                    "site": shield::site_of(&h.url),
                })).collect::<Vec<_>>(),
            }),
            "settings" => json!({
                "theme": s.theme, "plastic": s.plastic, "plastics": store::PLASTICS,
                "settings": s,
                "engines": store::SEARCH_ENGINES.iter().map(|e| json!({ "id": e.0, "name": e.1 })).collect::<Vec<_>>(),
                "lists": shield.sources(),
                "allowed": s.allowed_sites,
                "blockedTotal": shield.blocked_total(),
                "historyCount": self.store.history.len(),
                "bookmarkCount": self.store.bookmarks.len(),
                "profile": self.store.dir().display().to_string(),
                "version": env!("CARGO_PKG_VERSION"),
            }),
            _ => self.look(),
        }
    }

    fn handle(&mut self, message: QueuedEvent, target: &EventLoopWindowTarget<QueuedEvent>, flow: &mut ControlFlow) {
        let Some(event) = message.into_live() else { return };
        match event {
            UserEvent::Chrome(body) => self.handle_chrome(&body, target, flow),
            UserEvent::Content { tab, origin, body } => self.handle_content(tab, &origin, &body),
            UserEvent::Title(tab, title) => {
                if let Some(i) = self.tabs.index_of(tab) {
                    let url = self.tabs.ordered[i].url.clone();
                    self.store.set_title(&url, &title);
                    self.tabs.ordered[i].title = title;
                    if self.tabs.active_id() == Some(tab) {
                        self.update_window_title();
                    }
                    self.dirty = true;
                }
            }
            UserEvent::Load { tab, started, url } => {
                if let Some(i) = self.tabs.index_of(tab) {
                    let t = &mut self.tabs.ordered[i];
                    t.loading = started;
                    if !url.is_empty() {
                        t.url = url;
                    }
                    if !started {
                        let (u, ti) = (t.url.clone(), t.title.clone());
                        self.store.record_visit(&u, &ti);
                    }
                    self.refresh_nav(i);
                    self.dirty = true;
                }
            }
            UserEvent::NavStarting { tab, url } => {
                if let Some(i) = self.tabs.index_of(tab) {
                    self.tabs.ordered[i].loading = true;
                    if !nav::is_internal(&url) {
                        self.tabs.ordered[i].favicon.clear();
                    }
                    self.dirty = true;
                }
            }
            UserEvent::Url { tab, url } => {
                if let Some(i) = self.tabs.index_of(tab) {
                    let spa = !self.tabs.ordered[i].loading && self.tabs.ordered[i].url != url;
                    self.tabs.ordered[i].url = url.clone();
                    if spa {
                        let title = self.tabs.ordered[i].title.clone();
                        self.store.record_visit(&url, &title);
                    }
                    self.refresh_nav(i);
                    self.dirty = true;
                }
            }
            UserEvent::History(tab) => {
                if let Some(i) = self.tabs.index_of(tab) {
                    self.refresh_nav(i);
                    self.dirty = true;
                }
            }
            UserEvent::Favicon { tab, url } => {
                if let Some(i) = self.tabs.index_of(tab) {
                    self.tabs.ordered[i].favicon = url;
                    self.dirty = true;
                }
            }
            UserEvent::Blocked(_) => self.dirty = true,
            UserEvent::OpenTab { url, background } => self.open_tab(&url, !background),
            UserEvent::Filters(c) => {
                // Lists update silently in the background; settings shows them.
                self.shield.borrow_mut().load_compiled(&c.bytes, c.sources);
                self.dirty = true;
            }
            UserEvent::Action(a) => self.handle_action(&a),
            UserEvent::DevTools(body) => self.handle_devtools(&body),
            UserEvent::CdpResult { id, ok, json } => {
                let data = if serde_json::from_str::<Value>(&json).is_ok() && !json.trim().is_empty() {
                    json
                } else {
                    Value::String(json).to_string()
                };
                self.devtools_raw("result", &format!("{id},{ok},{data}"));
            }
            UserEvent::CdpEvent { tab, name, json } => {
                if self.devtools.open && self.devtools.tab == Some(tab) && serde_json::from_str::<Value>(&json).is_ok() {
                    self.devtools_raw("event", &format!("{},{json}", Value::String(name)));
                }
            }
            UserEvent::Inspect { tab, x, y } => {
                if let Some(i) = self.tabs.index_of(tab) {
                    if self.tabs.active_id() != Some(tab) {
                        self.activate(i);
                    }
                    self.open_devtools(Some(json!({ "view": "elements", "inspectAt": { "x": x, "y": y } })));
                }
            }
        }
    }

    // ---- DevTools panel ------------------------------------------------------

    fn ensure_devtools_view(&mut self) {
        if self.devtools.view.is_some() {
            return;
        }
        let p = self.proxy.clone();
        let (bounds, bg) = (self.devtools_bounds(), self.bg());
        let view = WebViewBuilder::new_with_web_context(&mut self.ctx)
            .with_url(nav::internal_url("devtools"))
            .with_bounds(bounds)
            .with_background_color(bg)
            .with_hotkeys_zoom(false)
            .with_custom_protocol("seoran".into(), |_id, req| engine::serve_internal(req))
            .with_ipc_handler(move |req: Request<String>| {
                if nav::internal_page(&req.uri().to_string()) == Some("devtools") {
                    p.send(UserEvent::DevTools(req.into_body()));
                }
            })
            .with_navigation_handler(|url| nav::internal_page(&url) == Some("devtools"))
            .build_as_child(&self.window);
        match view {
            Ok(view) => {
                engine::lock_down_ui(&view);
                self.devtools.view = Some(view);
                // Browser menus still draw above the panel.
                engine::raise(&self.chrome);
            }
            Err(e) => eprintln!("[seoran] could not open DevTools: {e}"),
        }
    }

    /// Open the panel; `show` optionally picks a view, element, etc.
    fn open_devtools(&mut self, show: Option<Value>) {
        self.ensure_devtools_view();
        let Some(view) = &self.devtools.view else { return };
        let _ = view.set_visible(true);
        self.devtools.open = true;
        self.layout();
        self.devtools_attach();
        if let Some(arg) = show {
            self.devtools_call("show", arg);
        }
        self.dirty = true;
    }

    fn close_devtools(&mut self) {
        self.devtools_detach();
        self.devtools.open = false;
        if let Some(view) = &self.devtools.view {
            let _ = view.set_visible(false);
        }
        self.layout();
        self.focus_content();
        self.dirty = true;
    }

    /// Point the panel at the active tab.
    fn devtools_attach(&mut self) {
        let Some(tab) = self.active_tab() else { return };
        let (id, url, title) = (tab.id, tab.url.clone(), tab.display_title());
        if self.devtools.tab == Some(id) {
            return;
        }
        self.devtools_detach();
        self.devtools.tab = Some(id);
        let theme = self.store.settings.theme.clone();
        self.devtools_call("attach", json!({ "tabId": id, "url": url, "title": title, "theme": theme, "plastic": self.store.settings.plastic }));
    }

    /// Stop the old tab's instrumentation so it runs at full speed again.
    fn devtools_detach(&mut self) {
        let Some(old) = self.devtools.tab.take() else { return };
        if let Some(page) = self.pages.get(&old) {
            for m in ["Overlay.hideHighlight", "Overlay.disable", "Network.disable", "CSS.disable", "DOM.disable", "Log.disable", "Runtime.disable"] {
                engine::cdp_call(&page.view, m, "{}", |_, _| {});
            }
        }
    }

    fn devtools_call(&mut self, func: &str, arg: Value) {
        if !self.devtools.ready {
            self.devtools.queued.push((func.into(), arg));
            return;
        }
        self.devtools_raw(func, &arg.to_string());
    }

    fn devtools_raw(&self, func: &str, args: &str) {
        if let Some(view) = &self.devtools.view {
            let _ = view.evaluate_script(&format!("window.sd && sd.{func}({args})"));
        }
    }

    fn devtools_page(&self) -> Option<&LivePage> {
        let id = self.devtools.tab?;
        self.pages.get(&id)
    }

    fn handle_devtools(&mut self, body: &str) {
        let Ok(msg) = serde_json::from_str::<Value>(body) else { return };
        match msg["t"].as_str().unwrap_or("") {
            "ready" => {
                self.devtools.ready = true;
                // Re-send the attach so a reloaded panel starts clean.
                let tab = self.devtools.tab.take();
                if self.devtools.open {
                    self.devtools_attach();
                } else {
                    self.devtools.tab = tab;
                }
                for (f, a) in std::mem::take(&mut self.devtools.queued) {
                    self.devtools_raw(&f, &a.to_string());
                }
            }
            "cdp" => {
                let id = msg["id"].as_u64().unwrap_or(0);
                let method = msg["method"].as_str().unwrap_or("").to_owned();
                let params = if msg["params"].is_object() { msg["params"].to_string() } else { "{}".into() };
                match self.devtools_page() {
                    Some(page) => {
                        let sender = self.proxy.for_page(&page.lifetime);
                        engine::cdp_call(&page.view, &method, &params, move |ok, json| {
                            sender.send(UserEvent::CdpResult { id, ok, json });
                        });
                    }
                    None => {
                        self.proxy.send(UserEvent::CdpResult {
                            id,
                            ok: false,
                            json: json!({ "message": "This tab is asleep. Switch to it to wake it up." }).to_string(),
                        });
                    }
                }
            }
            "subscribe" => {
                let Some(id) = self.devtools.tab else { return };
                let Some(page) = self.pages.get_mut(&id) else { return };
                let events: Vec<String> = msg["events"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|e| e.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                for name in events {
                    register_subscription(&mut page.cdp_subs, name, |name| {
                        let sender = self.proxy.for_page(&page.lifetime);
                        let event_name = name.to_owned();
                        engine::cdp_subscribe(&page.view, name, move |json| {
                            sender.send(UserEvent::CdpEvent { tab: id, name: event_name.clone(), json });
                        })
                    });
                }
            }
            "resize" => {
                if let Some(h) = msg["h"].as_f64() {
                    self.devtools.height = h.max(DEVTOOLS_MIN);
                    self.layout();
                }
            }
            "close" => self.close_devtools(),
            "advanced" => self.handle_action("devtoolsAdvanced"),
            "shield" => {
                let report = self.shield_report();
                self.devtools_call("shield", report);
            }
            "shieldSite" => {
                let on = msg["on"].as_bool().unwrap_or(true);
                let body = json!({ "t": "shieldSite", "on": on }).to_string();
                self.proxy.send(UserEvent::Chrome(body));
            }
            _ => {}
        }
    }

    /// What the shield did on the inspected page.
    fn shield_report(&self) -> Value {
        let Some(tab) = self.devtools.tab.and_then(|id| self.tabs.index_of(id)).map(|i| &self.tabs.ordered[i]) else {
            return json!({});
        };
        let shield = self.shield.borrow();
        let url = tab.hooks.page_url.borrow().clone();
        let log: Vec<Value> = tab
            .hooks
            .blocked_log
            .borrow()
            .iter()
            .map(|b| json!({ "url": b.url, "type": b.kind, "at": b.at_ms }))
            .collect();
        let site = shield::site_of(&url).filter(|_| !nav::is_internal(&url));
        json!({
            "url": url,
            "site": site,
            "enabled": shield.enabled(),
            "active": site.is_some() && shield.active_on(&url),
            "blocked": log,
            "scriptlets": shield.scriptlet_calls(&url),
            "cosmetic": shield.cosmetic_selectors(&url).len(),
            "lists": shield.sources(),
        })
    }

    /// Periodic housekeeping: persist data, put idle tabs to sleep.
    fn maintenance(&mut self) {
        self.last_maintenance = Instant::now();
        if self.store.history_dirty {
            self.store.save_history();
        }
        self.save_session();
        let limit = Duration::from_secs(self.store.settings.sleep_tabs_after_min.max(1) * 60);
        let mut slept = false;
        let active = self.tabs.active_id();
        for tab in &mut self.tabs.ordered {
            if Some(tab.id) == active || tab.last_active.elapsed() < limit {
                continue;
            }
            if self.devtools.open && self.devtools.tab == Some(tab.id) {
                continue;
            }
            if self.store.settings.sleep_tabs_after_min == 0 {
                continue;
            }
            let Some(page) = self.pages.get(&tab.id) else { continue };
            if engine::playing_audio(&page.view) {
                continue;
            }
            self.pages.remove(&tab.id);
            tab.loading = false;
            slept = true;
        }
        if slept {
            self.dirty = true;
        }
    }

    fn save_session(&self) {
        self.store.save_session(&self.tabs.session());
    }

    fn shutdown(&mut self) {
        self.store.settings.blocked_total = self.shield.borrow().blocked_total();
        self.store.save_settings();
        self.store.save_history();
        self.save_session();
    }
}

fn main() {
    let event_loop = EventLoopBuilder::<QueuedEvent>::with_user_event().build();
    let proxy = EventSender { queue: event_loop.create_proxy(), source: None };
    let mut browser: Option<Browser> = Some(Browser::new(&event_loop, proxy));

    event_loop.run(move |event, target, flow| {
        let Some(b) = browser.as_mut() else { return };
        *flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_secs(5));
        match event {
            Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                if b.last_maintenance.elapsed() >= MAINTENANCE_EVERY {
                    b.maintenance();
                }
            }
            Event::UserEvent(message) => b.handle(message, target, flow),
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    b.layout();
                    b.dirty = true;
                }
                WindowEvent::CloseRequested => {
                    b.shutdown();
                    *flow = ControlFlow::Exit;
                }
                _ => {}
            },
            Event::MainEventsCleared => {
                if let Some(wait) = b.flush() {
                    *flow = ControlFlow::WaitUntil(Instant::now() + wait);
                }
            }
            _ => {}
        }
        if *flow == ControlFlow::Exit {
            // Drop webviews before the window goes away.
            browser = None;
        }
    });
}

#[cfg(test)]
mod lifecycle_tests;
