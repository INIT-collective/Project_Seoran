//! Logical tab state. Resident native pages are owned separately by the browser.

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use crate::{engine::TabHooks, nav};

pub(super) struct Tab {
    pub(super) id: u32,
    pub(super) hooks: TabHooks,
    pub(super) url: String,
    pub(super) title: String,
    pub(super) favicon: String,
    pub(super) loading: bool,
    pub(super) can_back: bool,
    pub(super) can_forward: bool,
    pub(super) last_active: Instant,
    key_times: VecDeque<Instant>,
}

impl Tab {
    pub(super) fn new(id: u32, url: &str, title: &str) -> Self {
        Self {
            id,
            hooks: TabHooks::default(),
            url: url.to_owned(),
            title: title.to_owned(),
            favicon: String::new(),
            loading: false,
            can_back: false,
            can_forward: false,
            last_active: Instant::now(),
            key_times: VecDeque::new(),
        }
    }

    pub(super) fn display_title(&self) -> String {
        if nav::internal_page(&self.url) == Some("newtab") {
            return "New tab".into();
        }
        if !self.title.trim().is_empty() {
            return self.title.clone();
        }
        shield::site_of(&self.url).unwrap_or_else(|| self.url.clone())
    }

    /// Snapshot for the built-in, trusted chrome.
    pub(super) fn chrome_state(&self, resident: bool) -> Value {
        json!({
            "id": self.id,
            "title": self.display_title(),
            "url": nav::display_url(&self.url),
            "loading": self.loading,
            "blocked": self.hooks.blocked.get(),
            "favicon": self.favicon,
            "internal": nav::is_internal(&self.url),
            "sleeping": !resident,
        })
    }

    /// Pages may send key messages directly; cap them so a page can't spam
    /// "open tab" and the like.
    pub(super) fn allow_key(&mut self) -> bool {
        let now = Instant::now();
        while self.key_times.front().is_some_and(|t| now.duration_since(*t) > Duration::from_secs(1)) {
            self.key_times.pop_front();
        }
        if self.key_times.len() >= 8 {
            return false;
        }
        self.key_times.push_back(now);
        true
    }
}

#[derive(Default)]
pub(super) struct Tabs {
    pub(super) ordered: Vec<Tab>,
    active: Option<u32>,
}

impl Tabs {
    pub(super) fn active_id(&self) -> Option<u32> {
        self.active
    }

    pub(super) fn index_of(&self, id: u32) -> Option<usize> {
        self.ordered.iter().position(|t| t.id == id)
    }

    pub(super) fn active_index(&self) -> Option<usize> {
        self.index_of(self.active?)
    }

    pub(super) fn select(&mut self, index: usize) -> bool {
        let Some(tab) = self.ordered.get(index) else { return false };
        self.active = Some(tab.id);
        true
    }

    pub(super) fn remove(&mut self, id: u32) -> Option<Tab> {
        let i = self.index_of(id)?;
        let tab = self.ordered.remove(i);
        if self.active == Some(id) {
            self.active = self.ordered.get(i).or_else(|| self.ordered.last()).map(|t| t.id);
        }
        Some(tab)
    }

    pub(super) fn move_to(&mut self, id: u32, to: usize) -> bool {
        let Some(from) = self.index_of(id) else { return false };
        let to = to.min(self.ordered.len() - 1);
        if from == to {
            return false;
        }
        let tab = self.ordered.remove(from);
        self.ordered.insert(to, tab);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_chrome_snapshot_preserves_tab_identity_across_sleep() {
        let tab = Tab::new(7, "https://example.org/", "Example");
        let mut expected = json!({
            "id": 7, "title": "Example", "url": "https://example.org/", "loading": false,
            "blocked": 0, "favicon": "", "internal": false, "sleeping": false,
        });
        assert_eq!(tab.chrome_state(true), expected);
        expected["sleeping"] = json!(true);
        assert_eq!(tab.chrome_state(false), expected);
    }

    #[test]
    fn reordering_keeps_selection_and_session_position_in_sync() {
        let mut tabs = Tabs::default();
        tabs.ordered = [7, 11, 13].map(|id| Tab::new(id, nav::NEW_TAB_URL, "")).into();
        tabs.select(1);
        assert!(tabs.move_to(7, usize::MAX));
        assert_eq!(tabs.active_id(), Some(11));
        assert_eq!(tabs.active_index(), Some(0));
        assert!(tabs.move_to(11, 1));
        assert_eq!(tabs.active_id(), Some(11));
        assert_eq!(tabs.active_index(), Some(1));
        assert!(!tabs.move_to(42, 0));
        assert!(!tabs.select(50));
        assert_eq!(tabs.active_id(), Some(11));
    }

    #[test]
    fn closing_selects_the_surviving_neighbor_only_when_needed() {
        let mut tabs = Tabs::default();
        tabs.ordered = [1, 2, 3, 4].map(|id| Tab::new(id, nav::NEW_TAB_URL, "")).into();
        tabs.select(1);
        tabs.remove(1);
        assert_eq!(tabs.active_id(), Some(2));
        tabs.remove(2);
        assert_eq!(tabs.active_id(), Some(3));
        tabs.select(1);
        tabs.remove(4);
        assert_eq!(tabs.active_id(), Some(3));
        tabs.remove(3);
        assert_eq!(tabs.active_id(), None);
    }
}
