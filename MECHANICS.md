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
- Optional, off by default (setting "Allow JavaScript from AppleScript"): `--enable-applescript-javascript`. The browser process re-executes
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

## Atlassian in Dia (strings in the Dia program)
`AtlassianSessionCookieController`, `AtlassianFirstPartyOnboarding*`, `AtlassianMCPCredentials`,
`AtlassianAccessibleProductsClient`, `https://id.atlassian.com/manage-profile`,
`https://mcp.atlassian.com`, and the Settings rows `manageAtlassianAccountButton` / `atlassianSealView`.

## Open
- Whether Dia's release build applies the transient overrides at runtime, and how the new-tab
  page looks with `chat-enabled=false`. Only opening a rebuilt Didia shows this.
- Dia renames or adds flags over time. New AI features may appear until `src/flags.rs` is updated.
