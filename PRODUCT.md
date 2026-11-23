# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

(Desktop browser shell: the UI is HTML/CSS rendered in WebView2 child views on Windows. Not a website; it behaves like native app chrome.)

## Users

People who want to shape their daily desktop browser, from someone rearranging controls without code to a developer building a replacement component. The project welcomes different browsing habits and accessibility needs.

## Product Purpose

INIT:// is building a general-purpose desktop browser with deep customization, privacy, security and optional AI integration. The community intends to develop Seoran as open source. The original rights holders still need to choose a project license. [License decision](https://github.com/INIT-collective/Project_Seoran/issues/4)

## Current experiment

The Windows prototype lets the community test whether tab identity, live pages and trusted browser services can survive changes to the interface. A small declarative component contract is an experiment in independent authoring. The ultimate plugin architecture, engine, visual design and optional workflows remain open for discussion. [Architecture work and open decisions](ROADMAP.md)

## Operating Context

The current prototype runs on Windows. Contributors can use ordinary browsing sessions, keyboard navigation and local test pages to check behavior without disclosing private browsing history.

## Capabilities and Constraints

- Built today: tabs (session restore, sleeping tabs, drag to reorder, undo close), address bar with history/bookmark/tab suggestions, command palette (Ctrl K), bookmarks, new tab page, settings, Seoran Shield (network blocking, cosmetic filtering, scriptlets, per-site toggle), Seoran DevTools (Elements, Console, Network, Storage, Shield).
- Open proposals include optional AI assistance, additional platforms and alternative engines.
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
