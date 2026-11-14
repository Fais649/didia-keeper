# Didia Keeper

**Dia without the bloat.** For people who left Arc for Dia and want a clean browser.

Didia Keeper makes one of two browsers from official [Dia](https://www.diabrowser.com) releases
for macOS, and keeps it up to date automatically:

| | **Didia** | **Clia** |
|---|---|---|
| Dia's AI (Chat, agents, Morning Brief…) | switched off | kept, running on **your own Claude subscription** |
| Atlassian (sign-in, Jira, Confluence, tools) | switched off | switched off |
| Dia plan, trial and usage prompts | switched off | switched off |
| Installed as | `/Applications/Didia.app` | `/Applications/Clia.app` |

Nothing from Dia is in this repo. Didia Keeper starts from an official copy: the Dia app in your
Applications folder (copied, never changed) or a fresh download from Dia's official release feed.
It then makes a separate app from that copy.

## Didia: what gets switched off

- **AI**:
  - Chat and "Chat with this tab", and Ask on page
  - Skills, agents and browse-for-me
  - Morning Brief
  - memory and personalization
  - AI tab clean-up and renaming
  - AI upsells and trial prompts
- **Atlassian**: Atlassian sign-in and sign-up, Jira and Confluence connections, and Atlassian
  tools and onboarding.

Both are on by default, and each can be turned off on its own in the app.

### How

Dia has its own feature switches, and Didia sets them through the
`transientFeatureFlagOverrides` setting (`flag=false|flag=false`). A tiny library added to Didia
does this each time it starts. As a backup, it hides any AI or Atlassian button that still shows.
Nothing is removed from Dia's files, and your saved Dia preferences are never changed.

- The switch lists are in [`src/flags.rs`](src/flags.rs).
- The patch library is [`patch/didia_patch.m`](patch/didia_patch.m).
- [`MECHANICS.md`](MECHANICS.md) explains how it was worked out.

## Clia: bring your own Claude

Dia's AI already runs on a built-in copy of Claude Code. Dia starts it in a sandbox that may only
talk to Dia's own model gateway. Clia changes three things:
- it puts a tiny launcher in front of that Claude Code;
- the launcher hands Claude Code your subscription token and drops the gateway settings;
- the sandbox may now reach Anthropic's API over HTTPS.

Dia's tools (tabs, pages, connected apps) keep going through Dia as before.

To connect, press **Connect my Claude subscription** in the app. It runs `claude setup-token`
(you need [Claude Code](https://claude.com/code) and a Pro or Max plan). You can also paste a
token from it. The token is stored only on your Mac, in
`~/Library/Application Support/Didia Keeper/claude-token` (readable only by you).

Codex isn't supported: Dia's agent is built around Claude Code. Some of Dia's AI may still go
through Dia's servers, or still need a Dia account.

## Updates

Turn on **Automatic updates** in the app. A background job then checks Dia's official release
feed every 4 hours. It accepts a download only if all of these pass:
- Apple's `codesign` check;
- the app is signed by The Browser Company (team `S6N382Y83G`);
- Gatekeeper accepts it.

The new Didia is then built and installed the next time you quit Didia. Dia's own updater is
switched off inside Didia, so it can't turn Didia back into plain Dia.

## Install

You need a Mac with Apple silicon, the Xcode command line tools (`xcode-select --install`) and
[Rust](https://rustup.rs).

```sh
git clone https://github.com/Fais649/didia-keeper
cd didia-keeper
cargo build --release
./target/release/didia-keeper
```

In the window, pick **Didia** or **Clia**, press **Rebuild … now**, then turn on
**Automatic updates**.

| Command | What it does |
|---|---|
| `didia-keeper` | opens the window |
| `didia-keeper --rebuild` | rebuilds and installs your edition with your current choices |
| `didia-keeper --rebuild --edition clia` | the same, for a given edition |
| `didia-keeper --update` | checks the official feed now |
| `didia-keeper --background` | the job automatic updates runs |

Files it uses:
- `~/Library/Application Support/Didia Keeper/`: the kept official copy, your settings and (Clia) your Claude token
- `~/Library/Logs/DidiaKeeper.log`: the Keeper's log
- `~/Library/Logs/Didia.log`: the patch's log
- `~/Library/LaunchAgents/com.didia.keeper.plist`: the background job, only while automatic updates are on

## Good to know

- Didia, Clia and Dia share Dia's profile and settings: your tabs, sidebar and logins carry over.
  Run only one at a time.
- Didia is signed on your own Mac (ad-hoc), so macOS may ask once more for camera, microphone
  or keychain access.
- Dia renames its switches now and then, so a new AI feature may show up until the list is updated.
  Issues and pull requests are welcome.
- To go back, turn off automatic updates, delete `/Applications/Didia.app` (or `Clia.app`) and use Dia.

Not affiliated with The Browser Company or Atlassian. Dia is a trademark of its owner.
