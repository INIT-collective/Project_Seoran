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
