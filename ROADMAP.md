# Seoran roadmap

Seoran is a general-purpose desktop browser. Its direction is deep customization for users and developers, privacy, security and optional AI. The INIT:// community is testing how far the current Windows prototype can support those goals. The project license remains an open decision for the original rights holders and matters for external contributions and distribution. [License and third-party notices](https://github.com/INIT-collective/Project_Seoran/issues/4)

## Current work

| Work | Evidence for review |
| --- | --- |
| Contributor setup | A newcomer can claim a task, hand it back, test a change and get a review. Windows CI and the licensing discussion are visible to contributors. [Contribution guide](CONTRIBUTING.md) · [Windows checks](https://github.com/INIT-collective/Project_Seoran/pull/17) |
| Windows baseline | Record the revision, toolchain, build and test results. Ask an independent tester to launch the browser and exercise navigation, tabs, keyboard controls, session restore, Shield and DevTools. [Windows baseline](https://github.com/INIT-collective/Project_Seoran/issues/5) |
| Customization proof of concept | Separate tab identity and state from presentation, then switch between top and left controls in one window while a page stays live. Check continuity, keyboard focus, native geometry and recovery. Keep each PR runnable with the existing top layout. |
| Component authoring experiment | Document a small host-rendered v0 tab contract. Test capability limits and fallback. A different contributor must deliver a replacement using only the public contract before the community can claim independent extensibility. |
| Evaluation | Publish measurements, failed cases and what the component author could or could not express. Contributors can recommend continuing, revising or stopping before expanding the architecture. |

## Decisions still open

- The current Rust and WebView2 combination supplies a Windows test bed. Proposals for CEF, a Chromium fork and other engines need evidence of a capability gap and a sustainable maintenance path. [Engine discussion](https://github.com/INIT-collective/Project_Seoran/issues/12)
- Safely isolated executable components remain an option beyond the declarative v0 experiment. Split view needs its own feasibility investigation.
- Local and cloud AI, optional workflows and specialized modes can be proposed independently. Their audience, providers and delivery order need community discussion. Existing workflow and beta proposals carry no release commitment.

The [milestones](https://github.com/INIT-collective/Project_Seoran/milestones) distinguish contributor and Windows work from unscheduled discussion groups. Proposed features enter the implementation queue after a scoped decision and a contributor who can own the work.
