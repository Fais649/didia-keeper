# Didia Keeper

**Dia without the bloat.** For people who left Arc for Dia and want a clean browser.

Didia Keeper makes one of two browsers from official [Dia](https://www.diabrowser.com) releases
for macOS, and keeps it up to date automatically:

| | **Didia** | **Clia** |
|---|---|---|
| Dia's AI (Chat, agents, Morning Brief…) | switched off | switched off, replaced by a **Claude panel** on your own Claude Code subscription |
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

## Clia: your own Claude instead of Dia's AI

Dia's own AI only works with a Dia plan. Clia switches it off, together with its plan prompts and
Atlassian, just like Didia. In its place:

- **A Claude button** where Dia's Chat button sits, styled like it. It opens a **Claude
  sidebar** on the right of the window (or press **⇧⌘K**), in your window's theme colours, with a
  header (new chat, daily brief, connected apps, close) and a rounded composer.
- **Ask about the tab you're on.** "This tab" lets Claude read the open page (address, title and
  text) through Dia's AppleScript support, so Clia turns on Dia's "JavaScript from AppleScript"
  option. Page text is treated as untrusted.
- **Connected apps.** Claude uses the apps connected to your Claude account (Gmail, Calendar,
  Drive, Slack, Notion…), as `claude mcp list` shows them. Atlassian is always left out. Claude only
  reads from them unless you ask it to send or change something. The grid icon lists them;
  manage them at claude.ai → Settings → Connectors.
- **Daily brief.** Each morning (after 5:00) Clia asks Claude for your brief: today's
  meetings, email that needs you, messages and mentions, tasks and docs. It appears when you open
  the sidebar, or from the sun icon. Turn it off in Didia Keeper.

Answers come from the [Claude Code](https://claude.com/code) installed on your Mac (`claude -p`),
on the account it's signed in to, so your Claude Pro or Max subscription is used. Nothing goes
through Dia's servers. You need Claude Code installed and signed in once (run `claude` in Terminal).
Codex isn't supported.

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
- `~/Library/Application Support/Didia Keeper/`: the kept official copy and your settings
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
