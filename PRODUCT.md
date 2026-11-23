# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

(Desktop browser shell: the UI is HTML/CSS rendered in WebView2 child views on Windows. Not a website; it behaves like native app chrome.)

## Users

Young power users: students, developers, gamers and creators who keep the browser open all day. They want ad-free YouTube and a private, fast browser, and they enjoy a browser with personality that they can customize. Many are technical and live on the keyboard.

## Product Purpose

The proposed first audience is people who regularly answer technical questions in Discord or GitHub communities, including mod authors, project maintainers and volunteer support helpers. They can test Seoran on one real question while keeping their usual browser. [Pilot and milestones](ROADMAP.md)

Seoran is an open-source desktop browser (Rust shell, Chromium engine) focused on customization, privacy, security and AI integration. Success is the team and its users switching to it as their daily browser.

## Positioning

The first product bet is a verifiable answer another community member can use. A helper collects public sources and private notes, chooses which excerpts a model may receive, and reviews a cited Markdown draft before sharing it. The recipient can check the evidence in their own browser. Test whether helpers return for a second case before investing in a wider release.

The original brief also proposed these directions:

- **Personality plus privacy:** Brave-level ad and tracker blocking, YouTube included, with an expressive, customizable identity instead of a corporate one.
- **Two modes (planned):** one browser that switches between Normal browsing and a Markets/News mode.
- **AI that drives the browser (planned):** an agent that can operate tabs and draft emails, with strict confirmation and safety rules.

## Operating Context

All-day use on Windows 10 and 11, often in a maximized window beside games, code editors and Discord. Heavy use of YouTube, search and many tabs. Built in the open by a small Discord team (roadmap: Browser Roadmap doc).

## Capabilities and Constraints

- Built today: tabs (session restore, sleeping tabs, drag to reorder, undo close), address bar with history/bookmark/tab suggestions, command palette (Ctrl K), bookmarks, new tab page, settings, Seoran Shield (network blocking, cosmetic filtering, scriptlets, per-site toggle), Seoran DevTools (Elements, Console, Network, Storage, Shield).
- Planned, not built: AI sidebar and agent, Markets/News mode, CEF engine, macOS and Linux.
- Technical: the browser UI covers the window and is clipped by a native window region to the toolbar plus open panels. Panels must be opaque rectangles (rounded corners allowed); no shadows or translucency outside them. No layout shift or flicker when panels open. UI pages are offline (compiled into the binary): no web fonts or images from the network.
- Internal pages use the `seoran://` scheme (newtab, settings, devtools).

## Brand Commitments

- Name: Seoran (working name).
- The current diamond logo is not binding; a new mark may be designed.
- The owner's personal site (haiclop.xyz) is a reference for energy and attitude only: its themes and elements must not be copied.

## Evidence on Hand

- Real counts the UI can show: ads and trackers blocked per page and in total, filter lists in use, scriptlets running.
- No users, testimonials, benchmarks or press yet; do not invent any.

## Product Principles

1. The page is the star: browser chrome must never get in the way of fast, all-day browsing.
2. Personality lives in details and moments, not in friction.
3. Privacy is visible: show what the shield did, in real numbers.
4. Keyboard first: every action is reachable from the keyboard and the command palette.
5. Honest by default: no fake data, no dark patterns.

## Accessibility & Inclusion

WCAG 2.1 AA: text contrast, visible focus states, full keyboard operation, and `prefers-reduced-motion` respected. Light and dark themes are both first-class, following Windows by default.
