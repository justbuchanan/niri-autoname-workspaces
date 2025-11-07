# niri-autoname-workspaces [![Nix CI](https://github.com/justbuchanan/niri-autoname-workspaces/actions/workflows/nix.yml/badge.svg)](https://github.com/justbuchanan/niri-autoname-workspaces/actions/workflows/nix.yml)

> [!WARNING]
> This project is now archived. I've ported its functionality to a waybar cffi module, which is a cleaner approach. The module is able to display icons without modifying the workspace's actual name in niri. See https://github.com/justbuchanan/waybar-niri-workspaces-enhanced.

This program automatically updates [niri](https://github.com/YaLTeR/niri) workspace names to show icons for running programs in your bar. It can also apply a style (like bold or color) to the currently-focused window icon to visually show where you're at.

![screenshot](screenshot.png)

See a demo video on [reddit](https://www.reddit.com/r/unixporn/comments/1o7rzdl/oc_niri_addon_for_showing_window_icons_in_your_bar)

It's very similar in function to [workstyle](https://github.com/pierrechevalier83/workstyle/tree/main) and [swayest_workstyle](https://github.com/Lyr-7D1h/swayest_workstyle), but designed to work with the niri window manager.

## Installation

### Cargo

```
cargo install --git https://github.com/justbuchanan/niri-autoname-workspaces
```

### Nix

Add the flake to your system or home-manager configuration OR use nix profiles:

```
nix profile install github.com:justbuchanan/niri-autoname-workspaces
```

### Niri Configuration

Add this to your `~/.config/niri/config.kdl`:

```
spawn-at-startup niri-autoname-workspaces
```

Adding this config entry will tell niri to launch the program the next time it starts. To start running it now without restarting niri, do:

```
niri msg action spawn -- niri-autoname-workspaces
```

Optionally add a keyboard shortcut for renaming the current workspace:

```
binds {
    Mod+R spawn-sh { "niri-autoname-workspaces rename" }
}
```

## Customization

`niri-autoname-workspaces` comes with a default config at [default_config.toml](./default_config.toml), however you can customize it by creating your own config file at `~/.config/niri/autoname-workspaces.toml`.

Icon mappings for programs can be customized as well as the style of the currently-focused window icon. See the default config file for more info.
