use super::*;
use tao::platform::windows::EventLoopBuilderExtWindows;

#[test]
fn queued_callbacks_expire_even_if_the_same_tab_id_is_reused_on_wake() {
    let owner = Arc::new(());
    let source = Arc::downgrade(&owner);
    let title = || QueuedEvent {
        source: Some(source.clone()),
        event: UserEvent::Title(7, "old page".into()),
    };
    assert!(title().into_live().is_some());
    let delayed = title();
    drop(owner);
    let replacement = Arc::new(());
    assert!(delayed.into_live().is_none());
    let fresh = QueuedEvent { source: Some(Arc::downgrade(&replacement)), event: UserEvent::History(7) };
    assert!(fresh.into_live().is_some());
    assert!(QueuedEvent { source: None, event: UserEvent::Action("newTab".into()) }.into_live().is_some());
}

#[test]
fn subscriptions_retry_failed_registration_and_deduplicate_success() {
    let mut names = HashSet::new();
    let event = "Runtime.consoleAPICalled";
    register_subscription(&mut names, event.into(), |_| Err(windows::core::Error::from_hresult(
        windows::Win32::Foundation::E_FAIL,
    )));
    assert!(names.is_empty());
    register_subscription(&mut names, event.into(), |_| Ok(()));
    assert!(names.contains(event));
    register_subscription(&mut names, event.into(), |_| panic!("registered twice on the same WebView"));
}

/// Check which side of each native boundary receives input, including after resize.
fn check_chrome_edges(browser: &Browser, x: i32, y: i32) {
    use windows::Win32::Graphics::Gdi::{CreateRectRgn, DeleteObject, GetWindowRgn, PtInRegion, RGN_ERROR};
    let hwnd = engine::host_window(&browser.chrome).unwrap();
    unsafe {
        let region = CreateRectRgn(0, 0, 0, 0);
        assert!(!region.is_invalid());
        let kind = GetWindowRgn(hwnd, region);
        let header = PtInRegion(region, x + 1, y - 1).as_bool();
        let content = PtInRegion(region, x, y).as_bool();
        let rail = PtInRegion(region, x - 1, y + 1).as_bool();
        let _ = DeleteObject(region.into());
        assert_ne!(kind, RGN_ERROR);
        assert!(header);
        assert!(!content);
        assert_eq!(rail, x > 0);
    }
}

fn exercise_live_layout(browser: &mut Browser, events: &EventLoopWindowTarget<QueuedEvent>, first: u32, background: u32) {
    use crate::layout::TabPosition::{Left, Top};

    let foreground_page = Arc::downgrade(&browser.pages[&first].lifetime);
    let background_page = Arc::downgrade(&browser.pages[&background].lifetime);
    let dir = browser.store.dir().to_owned();
    let file = dir.join("layout.json");
    let mut flow = ControlFlow::Wait;
    let left = r#"{"t":"setLayout","layout":{"tabs":"left"}}"#;
    let top = r#"{"t":"setLayout","layout":{"tabs":"top"}}"#;
    browser.handle_content(first, "https://example.org", left);
    browser.handle_content(first, "https://example.org", r#"{"t":"key","a":"setLayout"}"#);
    assert_eq!(browser.tab_layout.tabs, Top);
    assert!(!file.exists());

    for i in 0..100 {
        if i == 50 {
            browser.window.set_inner_size(LogicalSize::new(520.0, 360.0));
        }
        let (msg, expected) = if i % 2 == 0 { (left, Left) } else { (top, Top) };
        browser.handle_chrome(msg, events, &mut flow);
        assert_eq!(browser.tab_layout.tabs, expected);
        assert_eq!(Layout::read(&dir).unwrap().tabs, expected);
        assert_eq!(browser.pages.len(), 2);
        assert!(foreground_page.ptr_eq(&Arc::downgrade(&browser.pages[&first].lifetime)));
        assert!(background_page.ptr_eq(&Arc::downgrade(&browser.pages[&background].lifetime)));
        assert_eq!(browser.tabs.active_id(), Some(first));
        assert_eq!(browser.devtools.tab, Some(first));
        assert_eq!(browser.tabs.ordered[1].id, background);
        let scale = browser.window.scale_factor();
        let rail = if expected == Left { 184.0 } else { 0.0 };
        let header = if expected == Left { 46.0 } else { 88.0 };
        let page = browser.active_view().unwrap().bounds().unwrap();
        let dock = browser.devtools.view.as_ref().unwrap().bounds().unwrap();
        let page_at = page.position.to_physical::<i32>(scale);
        let dock_at = dock.position.to_physical::<i32>(scale);
        assert_eq!(page_at.x, (rail * scale).round() as i32);
        assert_eq!(page_at.y, (header * scale).round() as i32);
        check_chrome_edges(browser, page_at.x, page_at.y);
        assert_eq!(dock_at.x, page_at.x);
        assert!((dock_at.y - page_at.y - page.size.to_physical::<i32>(scale).height).abs() <= 1);
    }

    std::fs::write(&file, r#"{"tabs":"left"}"#).unwrap();
    browser.handle_chrome(r#"{"t":"reloadLayout"}"#, events, &mut flow);
    assert_eq!(browser.tab_layout.tabs, Left);
    std::fs::write(&file, r#"{"tabs":"bottom"}"#).unwrap();
    browser.handle_chrome(r#"{"t":"reloadLayout"}"#, events, &mut flow);
    assert_eq!(browser.tab_layout.tabs, Top);
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap(); // Force the atomic replacement to fail.
    browser.handle_chrome(left, events, &mut flow);
    assert_eq!(browser.tab_layout.tabs, Top);
    assert!(foreground_page.ptr_eq(&Arc::downgrade(&browser.pages[&first].lifetime)));
    std::fs::remove_dir(&file).unwrap();
    browser.handle_chrome(left, events, &mut flow);
}

#[test]
#[ignore = "requires a Windows desktop and the WebView2 runtime"]
fn windows_tab_lifecycle() {
    let events = EventLoopBuilder::<QueuedEvent>::with_user_event().with_any_thread(true).build();
    let sender = EventSender { queue: events.create_proxy(), source: None };
    let dir = tempfile::tempdir().unwrap();
    let profile = dir.path().to_path_buf();
    let mut browser = Browser::with_profile(&events, sender.clone(), profile.clone(), None);
    let first = browser.tabs.active_id().unwrap();
    browser.open_tab(&nav::internal_url("settings"), false);
    let background = browser.tabs.ordered[1].id;
    assert_eq!(browser.tabs.active_id(), Some(first));
    assert!(browser.pages.contains_key(&background));

    let queued_from = |page: &LivePage, event| QueuedEvent {
        source: Some(Arc::downgrade(&page.lifetime)),
        event,
    };
    let subscribe = r#"{"t":"subscribe","events":["Runtime.consoleAPICalled","Runtime.consoleAPICalled"]}"#;
    browser.activate(1);
    browser.open_devtools(None);
    browser.handle_devtools(subscribe);
    assert_eq!(browser.pages[&background].cdp_subs.len(), 1);
    browser.activate(0);
    assert_eq!(browser.devtools.tab, Some(first));
    exercise_live_layout(&mut browser, &events, first, background);

    let title_event = |title: &str| UserEvent::Title(background, title.into());
    let before_sleep = queued_from(&browser.pages[&background], title_event("retired title"));
    let before_wake = queued_from(&browser.pages[&background], title_event("retired title"));
    let current = queued_from(&browser.pages[&background], title_event("resident title"));
    browser.handle(current, &events, &mut ControlFlow::Wait);
    assert_eq!(browser.tabs.ordered[1].title, "resident title");

    browser.store.settings.sleep_tabs_after_min = 1;
    for tab in &mut browser.tabs.ordered {
        tab.last_active = Instant::now() - Duration::from_secs(90);
    }
    browser.maintenance();
    assert!(!browser.pages.contains_key(&background));
    assert!(browser.pages.contains_key(&first));
    browser.handle(before_sleep, &events, &mut ControlFlow::Wait);
    assert_eq!(browser.tabs.ordered[1].title, "resident title");
    browser.activate(1);
    assert_eq!(browser.tabs.active_id(), Some(background));
    browser.handle(before_wake, &events, &mut ControlFlow::Wait);
    assert_eq!(browser.tabs.ordered[1].title, "resident title");
    assert!(browser.pages[&background].cdp_subs.is_empty());
    browser.handle_devtools(subscribe);
    assert_eq!(browser.pages[&background].cdp_subs.len(), 1);

    let foreground_age = browser.tabs.ordered[1].last_active;
    let retired = queued_from(&browser.pages[&first], UserEvent::OpenTab { url: nav::NEW_TAB_URL.into(), background: false });
    browser.close_tab(first);
    assert_eq!(browser.tabs.active_id(), Some(background));
    assert_eq!(browser.tabs.ordered[0].last_active, foreground_age);
    assert!(!browser.pages.contains_key(&first));
    browser.handle(retired, &events, &mut ControlFlow::Wait);
    assert_eq!(browser.tabs.ordered.len(), 1);
    browser.close_tab(first);
    assert_eq!(browser.tabs.ordered.len(), 1);

    browser.open_tab(nav::NEW_TAB_URL, false);
    browser.move_tab(background, 1);
    browser.shutdown();
    drop(browser);
    let mut restored = Browser::with_profile(&events, sender, profile, None);
    assert_eq!(restored.tab_layout.tabs, layout::TabPosition::Left);
    let active = restored.tabs.active_id().unwrap();
    let other = restored.tabs.ordered[0].id;
    assert_eq!(restored.tabs.active_index(), Some(1));
    assert_eq!(restored.tabs.ordered[1].url, nav::internal_url("settings"));
    assert_eq!(restored.pages.len(), 1);
    assert!(restored.pages.contains_key(&active));
    restored.close_tab(active);
    assert_eq!(restored.tabs.active_id(), Some(other));
    assert!(restored.pages.contains_key(&other));
    restored.close_tab(other);
    let fresh = restored.tabs.active_id().unwrap();
    assert_ne!(fresh, other);
    assert_eq!(restored.tabs.ordered.len(), 1);
    assert_eq!(restored.tabs.ordered[0].url, nav::NEW_TAB_URL);
    assert!(restored.pages.contains_key(&fresh));
}
