# How Didia works

Didia is the official Dia browser (studied at Dia 1.57.1, build 90935) with one small injected
library, an ad-hoc signature, Dia's own updater switched off and the display name "Didia".
Didia Keeper makes it from an official copy and keeps it current.

## Updates
| Value | Source |
|---|---|
| Feed: `https://releases.diabrowser.com/BoostBrowser-updates.xml` (Sparkle RSS) | Dia's official update host |
| Newest full release = the `<enclosure>` with a `.zip` URL and the highest `sparkle:version`. `.delta` enclosures are skipped. | feed contents |
| A download is accepted only if `codesign --verify --deep --strict` passes, the TeamIdentifier is `S6N382Y83G` (The Browser Company) and `spctl -a -t exec` passes. | `codesign -dv` of official Dia |
| The background run happens every 600 s, and the feed is checked at most every 14400 s (Dia itself uses `SUScheduledCheckInterval` 28800). | Dia Info.plist; design choice |
| A build made while Didia is open is staged, then swapped in once Didia is closed. | design choice |
| Didia counts as running when a process path contains `/Applications/Didia.app/Contents/MacOS/`. | design choice |

## Building Didia
- Copy official Dia and add `libdidia.dylib` to `Contents/Frameworks`.
- Add one `LC_LOAD_DYLIB` load command (`@executable_path/../Frameworks/libdidia.dylib`) to the
  main program, in the header padding (`src/macho.rs`).
- Remove `embedded.provisionprofile`.
- Info.plist: `SUEnableAutomaticChecks=false`, `SUAutomaticallyUpdate=false`, no `SUFeedURL`,
  `CFBundleDisplayName=Didia`.
- Sign ad-hoc with the audio input, camera and location entitlements, so web pages can still ask
  for the microphone, camera and location.
- Optional, off by default: `--enable-applescript-javascript`. The browser process re-executes
  itself once with the flag; helper processes (`--type=`) are left alone.

## Switching features off (found in the Dia program)
- Dia has a feature-flag system (LaunchDarkly + `FeatureFlagsClient`) with transient local
  overrides. They live in the UserDefaults key **`transientFeatureFlagOverrides`** (string at
  `0x106ba2b70`, part of Dia's UserDefaults key list at `0x1003cfca4` / `0x1003d10ec`).
- The parser `applyingTransientOverrides(launchArgument:)` (code around `0x1011d4080`–`0x1011d4500`):
  - splits the string on **`|`** (`mov w8,#0x7c` at `0x1011d4084`);
  - splits each part on **`=`** (`mov w8,#0x3d` at `0x1011d4278`);
  - needs exactly 2 parts, otherwise logs "Incorrect feature flag override format"
    (string `0x106beafb0`, used at `0x1011d4408`).
- Flag names are Dia's own kebab-case strings (e.g. `chat-enabled`, `ask-in-page-enabled`,
  `atlassian-first-party-experience-enabled`). The full lists are in `src/flags.rs`: 66 AI flags,
  9 Atlassian flags, plus `atlassian-identity-signup-rollout-percentage=0`.
- The value text `false` is **estimated**: Swift `Bool("false")` parsing is assumed, because the
  converter at `0x1011ced04` wasn't fully decoded.
- The patch puts the overrides (and `enablePageContentExtraction=false`) in the registration and
  launch-argument defaults domains. They win over saved preferences without changing them; a probe
  read back all 76 overrides, and extraction as off.
- Backup: every 0.5 s the patch hides views whose accessibility label, title or class matches:
  - the labels "Chat", "Chat with this tab", "… only available when you have chat usage",
    "Personalization on for new chats" and "Delete chat";
  - the classes `AssistantCapabilitiesToolbarView`, `AssistantPersonalityUI.PersonalizeButton` and
    `NavigationBar.AssistantBarView`;
  - any label containing "Atlassian".

  Page content views (`RenderWidgetHostView*`, `WebContents*`) are never touched.

## Clia: a Claude panel on the person's own Claude Code
- Dia's chat is gated by Dia's plan: a "View Plans" screen ("Title shown to unpaid users when chat
  is unavailable", `WebSupertabController`, `ChatUsageUpgradePlaceholderViewController`). Clia
  doesn't touch that gate. It switches Dia's AI off like Didia does, and hides those upsell views.
- The panel lives in `patch/didia_patch.m` (`CliaPanel`):
  - It adds a "Claude" menu (⇧⌘K) and an NSPanel next to the browser window.
  - It reads the page with `osascript` → Dia's `execute <tab> javascript` (the Dia.sdef command
    `CrSuExJa`), which needs `--enable-applescript-javascript`, getting the URL, the title and
    `document.body.innerText` (first 40 000 characters).
  - It runs `claude -p --output-format json --append-system-prompt … --allowedTools WebFetch,WebSearch
    [--resume <session_id>]` with the prompt on stdin. `ANTHROPIC_*` and `CLAUDE_CODE_*` variables
    are removed so the signed-in subscription is used. It runs in
    `~/Library/Application Support/Clia/Claude`.
- Self-test (2026-11-15): in the installed Clia, a test question about the open tab came back from
  Claude Code in about 30 s, with the page read (log line "clia: answered (237 chars, page read)").

## Clia sidebar, daily brief and connected apps
- Dia's own sidebar (view tree read inside Clia): `AssistantSidebarViewController` RootView holds the page
  container and a 480 pt panel at the right edge (`AssistantPanelRootView`). Its header has
  30×30 icon buttons on the right, and it sits on the window's theme background
  (`WindowBackgroundOverlayTintView` → `BackgroundColorView` + `GradientView`).
- Clia's own sidebar copies that look: the right edge, full height, the window's theme colour and
  gradient layers copied, the same icon header, a serif empty-state title and a rounded composer. Dia's
  chat stays switched off; a leftover open Dia chat panel is closed when Claude opens.
- Connected apps: `claude mcp list` is parsed (`name: url - status`); the tool prefix is `mcp__` + the name
  with non-alphanumerics turned into `_` (e.g. `mcp__claude_ai_Gmail`). Atlassian is skipped and also disallowed.
- Daily brief: `claude -p` with a read-only brief prompt, cached as
  `~/Library/Application Support/Clia/brief-YYYY-MM-DD.md`. It is made in the background after 05:00.
- Self-test 2026-11-15: brief made from Gmail, Slack, Drive and Notion (1974 characters). A Google Calendar
  question failed because that connector isn't authorized in the person's Claude account.

## Atlassian in Dia (strings in the Dia program)
`AtlassianSessionCookieController`, `AtlassianFirstPartyOnboarding*`, `AtlassianMCPCredentials`,
`AtlassianAccessibleProductsClient`, `https://id.atlassian.com/manage-profile`,
`https://mcp.atlassian.com`, and the Settings rows `manageAtlassianAccountButton` / `atlassianSealView`.

## Open
- Whether Dia's release build applies the transient overrides at runtime, and how the new-tab
  page looks with `chat-enabled=false`. Only opening a rebuilt Didia shows this.
- Dia renames or adds flags over time. New AI features may appear until `src/flags.rs` is updated.
