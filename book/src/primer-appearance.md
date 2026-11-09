# Primer appearance and theming

Primer keeps layout and behavior settings in `config.toml` and colors in Helix-compatible theme files. These settings do not require rebuilding Primer.

## Recommended configuration

Merge these keys into `~/.config/helix/config.toml`:

```toml
[editor]
border-style = "rounded"
color-modes = true
statusline-powerline = true
bufferline = "always"
bufferline-powerline = true
bufferline-separators = false
```

| Setting | Values | Default | Purpose |
| --- | --- | --- | --- |
| `border-style` | `rounded`, `square`, `double` | `rounded` | Box-drawing corners on bordered prompts, pickers, info panels, and popups |
| `color-modes` | boolean | `false` | Existing Helix setting for colored mode indicators |
| `statusline-powerline` | boolean | `true` | Show Powerline caps around the mode indicator, when `color-modes` is on and the theme supplies a background |
| `bufferline` | `always`, `multiple`, `never` | Helix default | Existing Helix setting for tab visibility |
| `bufferline-powerline` | boolean | `true` | Show caps around the active buffer tab |
| `bufferline-separators` | boolean | `false` | Show `│` between tabs instead of whitespace |

The corner styles use Unicode terminal characters: rounded `╭ ╮ ╰ ╯`, square `┌ ┐ └ ┘`, double `╔ ╗ ╚ ╝`. There is no `none` border-style value; the separate existing `popup-border` setting controls border visibility for some Helix popups. Terminal window corners are not affected.

## Theme colors

Create a theme in `~/.config/helix/themes/`, then select it with a top-level `theme = "your-theme"` in `config.toml`.

| Theme key | Controls |
| --- | --- |
| `ui.window` | Framed popup and picker border color |
| `ui.menu` | Completion menu appearance |
| `ui.popup` | Popup surface |
| `ui.statusline.normal` | Normal-mode indicator and active buffer tab foreground/background |
| `ui.statusline.insert` / `ui.statusline.select` | Other mode indicators |
| `ui.bufferline` | Inactive buffer tabs |
| `ui.bufferline.background` | Bufferline surface |
| `ui.bufferline.active` | Active tab base style, but its foreground/background are currently overridden by `ui.statusline.normal` |

Example theme entries:

```toml
"ui.window" = { fg = "#9B7AC8" }
"ui.statusline.normal" = { fg = "#1D162B", bg = "#9B7AC8" }
"ui.bufferline" = { fg = "#B0A5C8" }
```

See [Themes](./themes.md) for theme inheritance, palette definitions, and syntax. The active buffer tab currently follows the Normal-mode colors by design, so `ui.bufferline.active` does not independently override those two colors. Powerline caps `` and `` require a compatible font, such as a Nerd Font.

## Reloading and checking changes

Use `:config-reload` to reload editor settings. Use `:theme` to select a theme. Check the `:` command prompt, `/` search prompt, `Space` keymap help, and `Space b b` buffer picker when comparing borders. Open multiple files to inspect tabs.

Embedded terminal buffers are not yet implemented.
