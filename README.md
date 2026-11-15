# Didia Keeper

**Dia without the AI.** For people who left Arc for Dia and want a clean browser.

Didia Keeper makes **Didia** from the official [Dia](https://www.diabrowser.com) browser for macOS
and keeps it up to date. Didia is Dia with its AI and Atlassian features switched off. It is saved
as `/Applications/Didia.app`.

Nothing from Dia is in this repo. Didia Keeper starts from an official copy: the Dia app in your
Applications folder (copied, never changed) or a fresh download from Dia's official release feed.
It then makes a separate app from that copy.

## Install

You need a Mac with Apple silicon.

1. Download the latest zip from [GitHub Releases](https://github.com/Fais649/didia-keeper/releases).
2. Unzip it and drag **Didia Keeper.app** to your Applications folder.
3. Open **Didia Keeper** and press **Build Didia**.
4. Turn on **Automatic updates**, and press **Make default browser** if you like.

Didia Keeper is signed on the developer's Mac only, so macOS may ask you to confirm the first
time you open it (right-click the app, then Open).

## What gets switched off

- **AI**:
  - Chat and "Chat with this tab", and Ask on page
  - Skills, agents and browse-for-me
  - Morning Brief
  - memory and personalization
  - AI tab clean-up and renaming
  - AI upsells, trial and plan prompts
- **Atlassian**: Atlassian sign-in and sign-up, Jira and Confluence connections, and Atlassian
  tools and onboarding.

This is always on; there is nothing to configure.

### How

Dia has its own feature switches, and Didia sets them through the
`transientFeatureFlagOverrides` setting (`flag=false|flag=false`). A tiny library added to Didia
does this each time it starts. As a backup, it hides any AI or Atlassian button that still shows.
Nothing is removed from Dia's files, and your saved Dia preferences are never changed.

- The switch lists are in [`src/flags.rs`](src/flags.rs).
- The patch library is [`patch/didia_patch.m`](patch/didia_patch.m).
- [`MECHANICS.md`](MECHANICS.md) explains how it was worked out.

## Updates

Turn on **Automatic updates** in the app. A background job then checks Dia's official release
feed every 4 hours. It accepts a download only if all of these pass:
- Apple's `codesign` check;
- the app is signed by The Browser Company (team `S6N382Y83G`);
- Gatekeeper accepts it.

The new Didia is then built and installed the next time you quit Didia. Dia's own updater is
switched off inside Didia, so it can't turn Didia back into plain Dia.

## The option

**Allow JavaScript from AppleScript (for automation tools)** is off by default. Turn it on only if
a tool of yours needs to run JavaScript in Didia's tabs through AppleScript. Didia then starts with
`--enable-applescript-javascript`. Press **Rebuild now** after changing it.

## Building from source

You need the Xcode command line tools (`xcode-select --install`) and [Rust](https://rustup.rs).

```sh
git clone https://github.com/Fais649/didia-keeper
cd didia-keeper
cargo build --release
./target/release/didia-keeper
```

`packaging/package.sh` makes the `dist/Didia Keeper.app` that goes into the release zip.

| Command | What it does |
|---|---|
| `didia-keeper` | opens the window |
| `didia-keeper --rebuild` | rebuilds Didia and installs it |
| `didia-keeper --update` | checks the official feed now |
| `didia-keeper --background` | the job automatic updates runs |
| `didia-keeper --enable-updates` | turns on automatic updates |
| `didia-keeper --make-default` | makes Didia the default browser |
| `didia-keeper --launch` | builds Didia if needed, then opens it |
| `didia-keeper --help` | lists these |

Files it uses:
- `~/Library/Application Support/Didia Keeper/`: the kept official copy and your settings
- `~/Library/Logs/DidiaKeeper.log`: the Keeper's log
- `~/Library/Logs/Didia.log`: the patch's log
- `~/Library/LaunchAgents/com.didia.keeper.plist`: the background job, only while automatic updates are on

## Good to know

- Didia and Dia share Dia's profile and settings: your tabs, sidebar and logins carry over.
  Run only one at a time.
- Didia is signed on your own Mac (ad-hoc), so macOS may ask once more for camera, microphone
  or keychain access.
- Dia renames its switches now and then, so a new AI feature may show up until the list is updated.
  Issues and pull requests are welcome.
- To go back, turn off automatic updates, delete `/Applications/Didia.app` and use Dia.

Not affiliated with The Browser Company or Atlassian. Dia is a trademark of its owner.
