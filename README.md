# Tauri + Solid + Typescript

This template should help get you started developing with Tauri, Solid and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Linux

To install:

Might need: sudo apt install libx11-dev (device_query)

To run cli:

cargo build --example answer && ./target/debug/examples/answer

## Submoduled rdev-fast

Added with: `git submodule add -b fast git@github.com:ollipal/rdev-fast.git`

Access token generated here: https://github.com/settings/personal-access-tokens/new

Requires access to rdev-fast & linkmouse repo, contents = Read-only

TODO: how to update????

After cloning fresh repo: `git submodule update --init --recursive`

## Icon

Current icon from: https://icons8.com/icons/set/new-tab

Downloaded to: `./app-icon.png`

Generate new icons with: `npm run tauri icon`

More info: https://tauri.app/v1/guides/features/icons/
