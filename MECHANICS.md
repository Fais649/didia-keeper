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

## Clia: Dia's agent on your own Claude (found in the Dia app bundle)
- Dia ships `Resources/agent-server-resources/dist/`, which holds `agent-server` and `handler`
  (Bun programs; their source maps `*.map` contain the source) and `claude` (Claude Code 2.1.310).
- At start-up, `agent-server` sets `ANTHROPIC_AUTH_TOKEN=sk-<random>`, `CLAUDE_CODE_USE_VERTEX=1`,
  `CLAUDE_CODE_SKIP_VERTEX_AUTH=1`, `ANTHROPIC_VERTEX_PROJECT_ID=agent-server` and
  `CLOUD_ML_REGION=us-central1`. Claude Code's model calls go to Dia's gateway on localhost.
- `handler` runs `APP_BUNDLE/claude` (or `CLAUDE_CODE_EXECUTABLE`) under
  `/usr/bin/sandbox-exec -f agent-claude-code.sb`, with the parameters `CONTEXT_DIR`, `APP_BUNDLE`,
  `BIN_DIR`, `AGENT_TMPDIR` and `GATEWAY_PORT`.
- The profile denies by default. It allows exec of `BIN_DIR/claude`, `/bin/sh` and a few tools,
  reading under `BIN_DIR`, and network only to `localhost:GATEWAY_PORT`.
- Clia (`src/byo.rs`):
  - renames `claude` → `claude-dia`;
  - writes a POSIX-sh launcher `claude` (shell built-ins only) that reads the token file into
    `CLAUDE_CODE_OAUTH_TOKEN`, unsets the Vertex/gateway variables, rewrites model ids
    `name@date` → `name-date`, and runs `claude-dia`;
  - appends sandbox rules: exec `claude-dia`, read the token file, outbound TCP `*:443`, and
    `/private/var/run/mDNSResponder`.
- Checked:
  - the launcher run outside the sandbox (token set, Vertex variables unset, model id rewritten);
  - under `sandbox-exec`, the launcher → `claude-dia` exec chain works;
  - the Clia app passes `codesign --verify --deep --strict`.
- Not yet seen: a real Dia agent turn running on the person's subscription.

## Atlassian in Dia (strings in the Dia program)
`AtlassianSessionCookieController`, `AtlassianFirstPartyOnboarding*`, `AtlassianMCPCredentials`,
`AtlassianAccessibleProductsClient`, `https://id.atlassian.com/manage-profile`,
`https://mcp.atlassian.com`, and the Settings rows `manageAtlassianAccountButton` / `atlassianSealView`.

## Open
- Whether Dia's release build applies the transient overrides at runtime, and how the new-tab
  page looks with `chat-enabled=false`. Only opening a rebuilt Didia shows this.
- Dia renames or adds flags over time. New AI features may appear until `src/flags.rs` is updated.
