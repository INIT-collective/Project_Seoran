//! The web engine layer. Today it drives WebView2 (Chromium) through `wry`
//! plus a few direct WebView2 COM calls for request blocking and history.
//! Everything engine-specific lives here, so a CEF or Servo backend can
//! replace it without touching the browser logic in `main.rs`.

use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

use shield::{ResourceKind, Shield};
use tao::window::Window;
use webview2_com::{
    AcceleratorKeyPressedEventHandler, AddScriptToExecuteOnDocumentCreatedCompletedHandler,
    CallDevToolsProtocolMethodCompletedHandler, ContextMenuRequestedEventHandler,
    CustomItemSelectedEventHandler, DevToolsProtocolEventReceivedEventHandler, FaviconChangedEventHandler,
    HistoryChangedEventHandler, Microsoft::Web::WebView2::Win32::*, NavigationStartingEventHandler,
    SourceChangedEventHandler, WebResourceRequestedEventHandler, take_pwstr,
};
use windows::{
    Win32::{
        Foundation::POINT,
        System::Com::IStream,
        UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_SHIFT},
    },
    core::{BOOL, HSTRING, Interface, PWSTR, w},
};
use wry::{
    NewWindowResponse, PageLoadEvent, Rect, WebContext, WebView, WebViewBuilder, WebViewExtWindows,
    http::{Request, Response, header::CONTENT_TYPE},
};

use crate::{EventSender, UserEvent, nav, pages, source_is_live};

/// Script every content page gets: forwards the browser's own keyboard
/// shortcuts (pages swallow key events, so the shell would never see them)
/// and gives internal `seoran://` pages a small message API.
const CONTENT_JS: &str = include_str!("../ui/content.js");

/// One request the shield blocked, for the DevTools Shield panel.
#[derive(Clone, Debug)]
pub struct BlockedRequest {
    pub url: String,
    pub kind: &'static str,
    pub at_ms: u64,
}

const BLOCKED_LOG_CAP: usize = 500;

/// State shared between a tab and its engine callbacks.
#[derive(Clone, Default)]
pub struct TabHooks {
    pub blocked: Rc<Cell<u32>>,
    pub blocked_log: Rc<RefCell<VecDeque<BlockedRequest>>>,
    pub page_url: Rc<RefCell<String>>,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Serves `seoran://` pages from files compiled into the binary.
pub fn serve_internal(request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    // wry hands us `seoran://<page>/<path>` (the page loads as
    // `http://seoran.<page>/` on Windows); accept either form.
    let uri = request.uri();
    let host = uri.host().unwrap_or("");
    let page = host.strip_prefix("seoran.").unwrap_or(host);
    let path = uri.path();
    match pages::asset(page, path) {
        Some((mime, body)) => Response::builder()
            .header(CONTENT_TYPE, mime)
            .header("Content-Security-Policy", pages::CSP)
            .body(Cow::Borrowed(body))
            .unwrap(),
        None => Response::builder()
            .status(404)
            .header(CONTENT_TYPE, "text/plain")
            .body(Cow::Borrowed(&b"Not found"[..]))
            .unwrap(),
    }
}

fn resource_kind(ctx: COREWEBVIEW2_WEB_RESOURCE_CONTEXT, url: &str, page: &str) -> ResourceKind {
    match ctx {
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT => {
            if url == page { ResourceKind::Document } else { ResourceKind::Subdocument }
        }
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_STYLESHEET => ResourceKind::Stylesheet,
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE => ResourceKind::Image,
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_MEDIA => ResourceKind::Media,
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FONT => ResourceKind::Font,
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SCRIPT => ResourceKind::Script,
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_XML_HTTP_REQUEST
        | COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FETCH
        | COREWEBVIEW2_WEB_RESOURCE_CONTEXT_EVENT_SOURCE => ResourceKind::Xhr,
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_WEBSOCKET => ResourceKind::WebSocket,
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_PING => ResourceKind::Ping,
        _ => ResourceKind::Other,
    }
}

pub struct ContentSpec<'a> {
    pub tab: u32,
    pub url: &'a str,
    pub bounds: Rect,
    pub visible: bool,
}

/// Create the webview for one tab and attach blocking, scriptlet injection
/// and navigation hooks.
pub fn create_content(
    window: &Window,
    ctx: &mut WebContext,
    spec: ContentSpec,
    background: (u8, u8, u8, u8),
    proxy: &EventSender,
    shield: &Rc<RefCell<Shield>>,
    hooks: &TabHooks,
) -> wry::Result<WebView> {
    let tab = spec.tab;
    let p_ipc = proxy.clone();
    let p_title = proxy.clone();
    let p_load = proxy.clone();
    let p_new = proxy.clone();

    let webview = WebViewBuilder::new_with_web_context(ctx)
        .with_url(spec.url)
        .with_bounds(spec.bounds)
        .with_visible(spec.visible)
        // Paint the theme colour, not white, until the page draws.
        .with_background_color(background)
        .with_devtools(true)
        .with_hotkeys_zoom(true)
        .with_initialization_script_for_main_only(CONTENT_JS, true)
        .with_custom_protocol("seoran".into(), |_id, req| serve_internal(req))
        .with_ipc_handler(move |req: Request<String>| {
            p_ipc.send(UserEvent::Content {
                tab,
                origin: req.uri().to_string(),
                body: req.into_body(),
            });
        })
        .with_document_title_changed_handler(move |title| {
            p_title.send(UserEvent::Title(tab, title));
        })
        .with_on_page_load_handler(move |event, url| {
            let started = matches!(event, PageLoadEvent::Started);
            p_load.send(UserEvent::Load { tab, started, url });
        })
        .with_new_window_req_handler(move |url, _features| {
            p_new.send(UserEvent::OpenTab { url, background: false });
            NewWindowResponse::Deny
        })
        .build_as_child(window)?;

    if let Err(e) = attach_hooks(&webview, tab, proxy, shield, hooks) {
        eprintln!("[seoran] engine hooks failed for tab {tab}: {e}");
    }
    Ok(webview)
}

fn attach_hooks(
    webview: &WebView,
    tab: u32,
    proxy: &EventSender,
    shield: &Rc<RefCell<Shield>>,
    hooks: &TabHooks,
) -> windows::core::Result<()> {
    let core = webview.webview();
    let env = webview.environment();
    let mut token = 0i64;
    unsafe {
        // See every request, including those from iframes and workers.
        match core.cast::<ICoreWebView2_22>() {
            Ok(core22) => core22.AddWebResourceRequestedFilterWithRequestSourceKinds(
                w!("*"),
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
            )?,
            Err(_) => core.AddWebResourceRequestedFilter(w!("*"), COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL)?,
        }

        // 1. Network blocking.
        {
            let shield = shield.clone();
            let page_url = hooks.page_url.clone();
            let blocked = hooks.blocked.clone();
            let log = hooks.blocked_log.clone();
            let proxy = proxy.clone();
            core.add_WebResourceRequested(
                &WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else { return Ok(()) };
                    let request = args.Request()?;
                    let mut raw = PWSTR::null();
                    request.Uri(&mut raw)?;
                    let url = take_pwstr(raw);
                    if !source_is_live(&proxy.source) {
                        return Ok(());
                    }
                    let page = page_url.borrow().clone();
                    if nav::is_internal(&page) || nav::is_internal(&url) {
                        return Ok(());
                    }
                    let mut ctx = COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL;
                    args.ResourceContext(&mut ctx)?;
                    let kind = resource_kind(ctx, &url, &page);
                    let block = shield
                        .try_borrow_mut()
                        .map(|mut s| s.should_block(&url, &page, kind))
                        .unwrap_or(false);
                    if block {
                        let response = env.CreateWebResourceResponse(
                            None::<&IStream>,
                            403,
                            w!("Blocked by Seoran Shield"),
                            w!(""),
                        )?;
                        args.SetResponse(&response)?;
                        blocked.set(blocked.get() + 1);
                        let mut log = log.borrow_mut();
                        if log.len() >= BLOCKED_LOG_CAP {
                            log.pop_front();
                        }
                        log.push_back(BlockedRequest { url, kind: kind.as_filter_type(), at_ms: now_ms() });
                        proxy.send(UserEvent::Blocked(tab));
                    }
                    Ok(())
                })),
                &mut token,
            )?;
        }

        // 2. Before each navigation: remember the page URL for blocking and
        //    register this page's scriptlets to run before its own scripts.
        {
            let shield = shield.clone();
            let page_url = hooks.page_url.clone();
            let script_ids = Rc::new(RefCell::new(Vec::<String>::new()));
            let blocked = hooks.blocked.clone();
            let log = hooks.blocked_log.clone();
            let proxy = proxy.clone();
            core.add_NavigationStarting(
                &NavigationStartingEventHandler::create(Box::new(move |sender, args| {
                    if !source_is_live(&proxy.source) {
                        return Ok(());
                    }
                    let (Some(sender), Some(args)) = (sender, args) else { return Ok(()) };
                    let mut raw = PWSTR::null();
                    args.Uri(&mut raw)?;
                    let url = take_pwstr(raw);
                    let mut redirected = BOOL::default();
                    let _ = args.IsRedirected(&mut redirected);
                    *page_url.borrow_mut() = url.clone();
                    if !redirected.as_bool() {
                        blocked.set(0);
                        log.borrow_mut().clear();
                    }
                    for id in script_ids.borrow_mut().drain(..) {
                        let _ = sender.RemoveScriptToExecuteOnDocumentCreated(&HSTRING::from(id));
                    }
                    let script = shield.try_borrow().ok().and_then(|s| s.document_start_script(&url));
                    if let Some(js) = script {
                        let ids = script_ids.clone();
                        sender.AddScriptToExecuteOnDocumentCreated(
                            &HSTRING::from(js),
                            &AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(
                                move |result, id| {
                                    if result.is_ok() {
                                        ids.borrow_mut().push(id);
                                    }
                                    Ok(())
                                },
                            )),
                        )?;
                    }
                    proxy.send(UserEvent::NavStarting { tab, url });
                    Ok(())
                })),
                &mut token,
            )?;
        }

        // 3. URL changes, including single-page-app navigations.
        {
            let proxy = proxy.clone();
            core.add_SourceChanged(
                &SourceChangedEventHandler::create(Box::new(move |sender, _| {
                    if let Some(sender) = sender {
                        let mut raw = PWSTR::null();
                        sender.Source(&mut raw)?;
                        proxy.send(UserEvent::Url { tab, url: take_pwstr(raw) });
                    }
                    Ok(())
                })),
                &mut token,
            )?;
        }
        {
            let proxy = proxy.clone();
            core.add_HistoryChanged(
                &HistoryChangedEventHandler::create(Box::new(move |_, _| {
                    proxy.send(UserEvent::History(tab));
                    Ok(())
                })),
                &mut token,
            )?;
        }
        if let Ok(core15) = core.cast::<ICoreWebView2_15>() {
            let proxy = proxy.clone();
            core15.add_FaviconChanged(
                &FaviconChangedEventHandler::create(Box::new(move |sender, _| {
                    if let Some(sender) = sender.and_then(|s| s.cast::<ICoreWebView2_15>().ok()) {
                        let mut raw = PWSTR::null();
                        sender.FaviconUri(&mut raw)?;
                        proxy.send(UserEvent::Favicon { tab, url: take_pwstr(raw) });
                    }
                    Ok(())
                })),
                &mut token,
            )?;
        }
    }
    attach_devtools_hooks(webview, tab, proxy)?;
    Ok(())
}

/// F12 / Ctrl+Shift+I/J/C and right-click > Inspect open Seoran DevTools
/// instead of the engine's own DevTools window.
fn attach_devtools_hooks(webview: &WebView, tab: u32, proxy: &EventSender) -> windows::core::Result<()> {
    let mut token = 0i64;
    unsafe {
        // Keys are caught here, before the engine's own accelerators run,
        // so a page can neither see nor block them.
        let p = proxy.clone();
        webview.controller().add_AcceleratorKeyPressed(
            &AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else { return Ok(()) };
                let mut kind = COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN;
                args.KeyEventKind(&mut kind)?;
                if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN {
                    return Ok(());
                }
                let mut vk = 0u32;
                args.VirtualKey(&mut vk)?;
                let ctrl = GetKeyState(VK_CONTROL.0 as i32) < 0;
                let shift = GetKeyState(VK_SHIFT.0 as i32) < 0;
                let action = match (vk, ctrl, shift) {
                    (0x7B, false, false) | (0x49, true, true) => Some("devtools"), // F12, Ctrl+Shift+I
                    (0x4A, true, true) => Some("devtoolsConsole"),                  // Ctrl+Shift+J
                    (0x43, true, true) => Some("devtoolsPick"),                     // Ctrl+Shift+C
                    _ => None,
                };
                if let Some(a) = action {
                    args.SetHandled(true)?;
                    let mut status = COREWEBVIEW2_PHYSICAL_KEY_STATUS::default();
                    let _ = args.PhysicalKeyStatus(&mut status);
                    if !status.WasKeyDown.as_bool() {
                        p.send(UserEvent::Action(a.into()));
                    }
                }
                Ok(())
            })),
            &mut token,
        )?;

        // Replace the engine's "Inspect" menu item with ours.
        let core = webview.webview();
        let (Ok(core11), Ok(env9)) =
            (core.cast::<ICoreWebView2_11>(), webview.environment().cast::<ICoreWebView2Environment9>())
        else {
            return Ok(());
        };
        let p = proxy.clone();
        core11.add_ContextMenuRequested(
            &ContextMenuRequestedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else { return Ok(()) };
                let items = args.MenuItems()?;
                let mut count = 0u32;
                items.Count(&mut count)?;
                let mut at = count;
                for i in (0..count).rev() {
                    let item = items.GetValueAtIndex(i)?;
                    let mut name = PWSTR::null();
                    item.Name(&mut name)?;
                    if take_pwstr(name) == "inspectElement" {
                        items.RemoveValueAtIndex(i)?;
                        at = i;
                    }
                }
                let mut pt = POINT::default();
                args.Location(&mut pt)?;
                let ours = env9.CreateContextMenuItem(
                    w!("Inspect"),
                    None::<&IStream>,
                    COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_COMMAND,
                )?;
                let p2 = p.clone();
                let mut t = 0i64;
                ours.add_CustomItemSelected(
                    &CustomItemSelectedEventHandler::create(Box::new(move |_, _| {
                        p2.send(UserEvent::Inspect { tab, x: pt.x, y: pt.y });
                        Ok(())
                    })),
                    &mut t,
                )?;
                items.InsertValueAtIndex(at.min(count), &ours)?;
                Ok(())
            })),
            &mut token,
        )?;
    }
    Ok(())
}

/// Call a DevTools Protocol method on a page. `done(ok, json)` gets the
/// result object, or the error, as JSON.
pub fn cdp_call(webview: &WebView, method: &str, params: &str, done: impl FnOnce(bool, String) + 'static) {
    let done = Rc::new(RefCell::new(Some(done)));
    let d2 = done.clone();
    let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |result, json| {
        if let Some(f) = d2.borrow_mut().take() {
            f(result.is_ok(), json);
        }
        Ok(())
    }));
    let started =
        unsafe { webview.webview().CallDevToolsProtocolMethod(&HSTRING::from(method), &HSTRING::from(params), &handler) };
    if let Err(e) = started {
        if let Some(f) = done.borrow_mut().take() {
            f(false, serde_json::json!({ "message": e.message() }).to_string());
        }
    }
}

/// Forward a DevTools Protocol event from a page to `on_event(json)`.
pub fn cdp_subscribe(webview: &WebView, event: &str, on_event: impl Fn(String) + 'static) -> windows::core::Result<()> {
    unsafe {
        let receiver = webview.webview().GetDevToolsProtocolEventReceiver(&HSTRING::from(event))?;
        let mut token = 0i64;
        receiver.add_DevToolsProtocolEventReceived(
            &DevToolsProtocolEventReceivedEventHandler::create(Box::new(move |_, args| {
                if let Some(args) = args {
                    let mut raw = PWSTR::null();
                    args.ParameterObjectAsJson(&mut raw)?;
                    on_event(take_pwstr(raw));
                }
                Ok(())
            })),
            &mut token,
        )
    }
}

/// Navigation state read straight from the engine.
pub struct NavState {
    pub can_back: bool,
    pub can_forward: bool,
}

pub fn nav_state(webview: &WebView) -> NavState {
    let core = webview.webview();
    let mut back = BOOL::default();
    let mut fwd = BOOL::default();
    unsafe {
        let _ = core.CanGoBack(&mut back);
        let _ = core.CanGoForward(&mut fwd);
    }
    NavState { can_back: back.as_bool(), can_forward: fwd.as_bool() }
}

pub fn go_back(webview: &WebView) {
    unsafe {
        let _ = webview.webview().GoBack();
    }
}

pub fn go_forward(webview: &WebView) {
    unsafe {
        let _ = webview.webview().GoForward();
    }
}

pub fn stop(webview: &WebView) {
    unsafe {
        let _ = webview.webview().Stop();
    }
}

/// Whether the page is playing sound (such tabs are never put to sleep).
pub fn playing_audio(webview: &WebView) -> bool {
    let mut playing = BOOL::default();
    unsafe {
        if let Ok(core8) = webview.webview().cast::<ICoreWebView2_8>() {
            let _ = core8.IsDocumentPlayingAudio(&mut playing);
        }
    }
    playing.as_bool()
}

/// The native window that hosts a webview (wry's container window).
pub(super) fn host_window(webview: &WebView) -> Option<windows::Win32::Foundation::HWND> {
    let mut hwnd = windows::Win32::Foundation::HWND::default();
    unsafe { webview.controller().ParentWindow(&mut hwnd).ok()? };
    (!hwnd.is_invalid()).then_some(hwnd)
}

/// Put a webview above its sibling webviews (the browser UI sits over the
/// page so its dropdowns can draw on top of it).
pub fn raise(webview: &WebView) {
    use windows::Win32::UI::WindowsAndMessaging::{
        HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
    };
    if let Some(hwnd) = host_window(webview) {
        unsafe {
            let _ = SetWindowPos(hwnd, Some(HWND_TOP), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }
}

/// A rectangle in physical pixels, with corner radius.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RegionRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub r: i32,
}

/// Clip a webview to the union of `rects`: it draws and takes input only
/// there, and the page beneath stays live and clickable everywhere else.
/// Changing the region needs no resize or repaint of either view, so
/// opening a menu can't make the page flicker.
pub fn set_region(webview: &WebView, rects: &[RegionRect]) -> std::io::Result<()> {
    use windows::Win32::Graphics::Gdi::{
        CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, RGN_ERROR, RGN_OR, SetWindowRgn,
    };
    let hwnd = host_window(webview).ok_or_else(|| std::io::Error::other("missing chrome window"))?;
    unsafe {
        let region = CreateRectRgn(0, 0, 0, 0);
        if region.is_invalid() {
            return Err(std::io::Error::other("could not create chrome region"));
        }
        for r in rects {
            let part = if r.r > 0 {
                CreateRoundRectRgn(r.x, r.y, r.x + r.w + 1, r.y + r.h + 1, r.r * 2, r.r * 2)
            } else {
                CreateRectRgn(r.x, r.y, r.x + r.w, r.y + r.h)
            };
            if part.is_invalid() {
                let _ = DeleteObject(region.into());
                return Err(std::io::Error::other("could not create chrome region segment"));
            }
            let combined = CombineRgn(Some(region), Some(region), Some(part), RGN_OR);
            let _ = DeleteObject(part.into());
            if combined == RGN_ERROR {
                let _ = DeleteObject(region.into());
                return Err(std::io::Error::other("could not combine chrome regions"));
            }
        }
        // The window takes ownership only on success.
        if SetWindowRgn(hwnd, Some(region), true) == 0 {
            let error = std::io::Error::last_os_error();
            let _ = DeleteObject(region.into());
            return Err(error);
        }
        Ok(())
    }
}

/// The browser UI is an app, not a page: no reload/zoom/print shortcuts,
/// no default context menu, no pinch zoom.
pub fn lock_down_ui(webview: &WebView) {
    unsafe {
        let Ok(settings) = webview.webview().Settings() else { return };
        if let Ok(s3) = settings.cast::<ICoreWebView2Settings3>() {
            let _ = s3.SetAreBrowserAcceleratorKeysEnabled(false);
        }
        let _ = settings.SetIsZoomControlEnabled(false);
        let _ = settings.SetIsStatusBarEnabled(false);
        if !cfg!(debug_assertions) {
            let _ = settings.SetAreDefaultContextMenusEnabled(false);
        }
        if let Ok(s5) = settings.cast::<ICoreWebView2Settings5>() {
            let _ = s5.SetIsPinchZoomEnabled(false);
        }
    }
}

pub fn open_devtools(webview: &WebView) {
    unsafe {
        let _ = webview.webview().OpenDevToolsWindow();
    }
}
