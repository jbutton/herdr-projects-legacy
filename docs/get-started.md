# Get started

You need [Herdr](https://herdr.dev) 0.9.3+ on macOS or Linux, Rust ([rustup.rs](https://rustup.rs)) to build the
program, and Claude Code, Codex, pi or another agent herdr supports. This copy of the 1.x board publishes no releases:
it is built from a checkout.

## Option A: do it yourself

1. Build and link it:

   ```sh
   git clone https://github.com/jbutton/herdr-projects-legacy && cd herdr-projects-legacy
   cargo build --release && herdr plugin link "$PWD" && sh bin/run.sh install
   ```

2. In Herdr, run the action **Open the work board**. Say yes to the key it offers (`prefix+a`).
3. Optional: add `~/.herdr-projects/data/bin` to your PATH so you can type `herdr-projects`.

## Option B: let an agent do it

Paste this into Claude Code, Codex or pi, running inside Herdr:

```text
Install the herdr-projects plugin for Herdr and set it up for me.
1. Check `herdr --version` is 0.9.3 or newer, and that git and cargo exist. Stop and tell me if not.
2. Clone https://github.com/jbutton/herdr-projects-legacy into a folder of mine and run `cargo build --release` in it.
3. In that folder, run `herdr plugin link "$PWD"`, then `sh bin/run.sh install`.
4. Add ~/.herdr-projects/data/bin to my PATH in my shell's start-up file, so `herdr-projects` works in new shells.
5. Run `herdr-projects doctor` and fix anything it reports.
6. Open the board: `herdr plugin action invoke open --plugin herdr-projects`.
Then tell me in two lines: it is ready, and to say yes to the key it offers (prefix+a) on its first screen.
Details if you need them: https://github.com/jbutton/herdr-projects-legacy/blob/main/docs/install.md
```

## Start it

- Press `prefix+a` in Herdr, or type `herdr-projects` in any pane.
- Type what needs doing and press `Enter`. The Coordinator makes it a task.
- Press `Ctrl+S` instead of `Enter` to put an agent on it right away.
- Press `?` to see everything else.

Update later with `git pull && cargo build --release` in the checkout (`herdr-projects update` downloads upstream's
releases, never this copy).

More detail (building from source, older installs, uninstalling): [installing](install.md).
