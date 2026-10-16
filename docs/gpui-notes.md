# Working with gpui-kit

Findings about `gpui-kit` and `gpui` that are not obvious from their docs and cost time to
discover. Read this before touching theming, inputs, dialogs, charts or timers.

`gpui` changes quickly. Check the version pinned in `Cargo.toml` and read the real source under
`~/.cargo/registry/src/*/` (`gpui-kit`, `gpui-component`, `gpui-base`, `gpui-pre`) instead of
trusting memory or this file blindly. The findings below were first checked against `gpui-kit`
0.6.6, by compiling and running throwaway code and reading the sources. The project is now on
0.7.0, so a detail may have moved; where 0.7.0 is known to differ, it is said. Source locations
are given as `file:line` in the 0.6.6 crates and may have shifted.

## Themes

`Theme::change` only reads `Theme.light_theme` and `Theme.dark_theme`, and those default to the
two themes baked into `gpui-component`, not to whatever `ThemeRegistry::load_themes_from_str`
just loaded (that call only fills the by-name lookup map). So after loading a family, fetch the
two configs by name and assign them yourself, then call `Theme::change` again. Skipping this
silently keeps the built-in theme. This is what `theme::install` does.

- Load themes after `gpui_kit::init(cx)`, not before: `ThemeRegistry` is set up as a global
  inside `init`.
- `load_themes_from_str` silently skips a theme whose name is already in the registry, so
  calling it again with the same names does **not** reload. To change a theme live (font or size
  changes in Settings) parse the `ThemeConfig` JSON directly and assign it to
  `Theme::global_mut(cx).light_theme` / `.dark_theme`, as `theme::apply_settings` does.
- `Theme::sync_system_appearance(window, cx)` keeps working: it calls `Theme::change`, which
  re-reads the two fields above.

### Mapping a token to a theme field

Vano's own views never read `cx.theme()`: colors come from `Palette`. The theme JSON only
matters for the borrowed stock widgets (`TitleBar`, `Button`, `Dialog`, the scrollbar, `Input`
caret, selection and border, tree chevrons, checkboxes, switches). The mapping is built in
`theme/palette.rs` (`theme_colors_and_highlight`). The non-obvious points:

- `surface` has to be set on **three** keys, `sidebar.background`, `title_bar.background` and
  `tab_bar.background`. The title bar and tab bar fall back to `background`, not to the sidebar.
- `accent` goes to `primary.background`. gpui-component's own `accent.background` means
  something else (hover background on menu and list items) and must not receive the brand
  color; it gets `accent_subtle`, together with `selection.background` and
  `list.active.background`. Selection and active alpha are clamped by gpui itself, so pass an
  opaque color.
- `hover` has to be set on `list.hover.background`, `button.hover.background` and
  `table.hover.background`: the last two do not fall back to the first.
- `raised` maps to `secondary.background`, `overlay` to `popover.background`, `border_strong` to
  `input.border`, `accent_text` to `link`.
- The modal backdrop is a different field from the modal fill: the theme's `overlay` key is the
  scrim `Dialog` paints. Vano sets it transparent and paints its own scrim from
  `Palette::scrim`, so the fill and the scrim cannot be confused.
- Some tokens have **no** theme field and live only in `Palette`: `fg_subtle`, every
  `*_subtle` badge background, the five method colors and the shadow. The five `chart.N` slots
  are for charts, not HTTP methods, and reusing them would break the load test colors.
- Shadows are not colors: `ThemeConfig.shadow` is only an on/off switch. Keep the raw shadow
  values in `Palette` and apply them with `.shadow(vec![BoxShadow { .. }])` on menus, popovers
  and modals. `BoxShadow::blur_radius` is the gaussian's standard deviation (the shadow shader
  calls `gaussian(y, blur_radius)` and draws 3 times it past the shape), half of what the same
  number means in CSS.

### Syntax highlight

`highlight.syntax` has one field per dotted capture name. Resolution is exact match first, then
it retries with everything before the first dot (`constant.builtin` falls back to `constant`,
`string.escape` to `string`). Captures emitted by the grammars Vano uses:

- JSON: `boolean`, `comment`, `constant.builtin`, `number`, `property` (object keys), `punctuation`,
  `string`, `string.escape`.
- JavaScript: `comment`, `constant`, `constant.builtin`, `embedded`, `function`,
  `function.builtin`, `function.method`, `keyword`, `number`, `operator`, `property`,
  `punctuation.*`, `string`, `string.special`, `type`, `variable`, `variable.builtin`. JavaScript
  `true` and `false` are `constant.builtin`, so they take the `constant` color, not `syn_bool`.
- HTML, used as the closest grammar for XML bodies: `attribute`, `comment`, `constant`,
  `punctuation.bracket`, `string`, `tag`, `tag.error`.

## Fonts

`cx.text_system().add_fonts(Vec<Cow<'static, [u8]>>)` registers TTF bytes, and
`all_font_names()` lists every installed family plus the ones added this way, which is the right
list for a font picker. Set `Theme.font_family`, `font_size`, `mono_font_family` and
`mono_font_size` to use them. There is no separate editor font setting: code editors read the
mono family.

## Icons

`gpui_component::IconName` is a small curated subset. The full Lucide catalog is
`gpui_kit::assets::IconName`. Both implement `IconNamed` and convert into `Icon`, so use
`Icon::new(gpui_kit::assets::IconName::SendHorizontal)` for any icon outside the subset. An icon
missing from the loaded asset set renders as empty space, silently; the app must be built with
`.with_assets(gpui_kit::assets::AllAssets)` (see `AGENTS.md`).

## Variable chips

**A single-line `Input` cannot style byte ranges.** `InputState` and `EditorState` are two
separate facades over one engine, and only the editor mode has a language and a highlighter. The
highlighter hooks compile on a plain `Input` but do nothing there. So the URL bar owns two
representations and picks one per render by focus: a read-only line of
`gpui::StyledText::with_highlights(runs)` when not focused, and the real `Input` when focused.
`HighlightStyle` supports a background color and a wavy underline in the same run, which is how a
defined `{{var}}` (accent background) and an unknown one (danger text, wavy underline) both
render from one call, using the byte ranges from `vano_core::variable_spans`.

**Inside a code editor, use text decorations.** `EditorState::create_decorations_collection`
returns a `TextDecorationCollection` of `TextDecoration { range, style: HighlightStyle }`. The
editor merges them with the syntax runs, so a `{{token}}` inside a JSON string keeps the string
color for the rest of the text. They follow edits, but `set_value` clears them, so set them again
after it (`views/request_editor.rs` does it when the variable context or the body text changes).
The tree-sitter `InputHighlighter` of `gpui-component` is `pub(crate)`, which is why wrapping it
was not an option.

**Hover tooltips.** A single-line field shows a plain `Tooltip` on each chip's `div`. The code
editor asks a `HoverProvider` (`state.lsp_mut().hover_provider`, then `state.refresh(cx)`), which
returns an `lsp_types::Hover` rendered as Markdown. Newlines of plain text collapse there, so the
provider uses a fenced first line and one paragraph per note (`views/variable_hover.rs`).

## Command palette

`gpui_component::command` (`Command`, `CommandItem`, `CommandGroup`, `CommandState`) is directly
usable and gives search, groups, icons, keywords, an action hint column, a checked state, arrow
navigation and Enter/Escape. Use `filterable(false)` plus an external filter when you need your
own ranking. There is no separate "palette" opener: put the `Command` inside a `Dialog`.

## Dialogs

There are two layers. `gpui_base::Dialog` is the low-level primitive. `gpui_component::dialog::
Dialog` is the ready-made wrapper (`.title`, `.content`, `.footer`, `.w`, `.overlay`,
`.overlay_closable`, `.keyboard` for Escape, `.close_button`, `.on_ok`/`.on_cancel`/`.on_close`).
Open it with `window.open_dialog(cx, |dialog, window, cx| ..)`. It centers itself and keeps a
margin from the window edges. Since 0.7.0 `Root` renders the dialog, sheet and notification
layers itself, so views must not render them again (see `AGENTS.md`).

`Dialog` paints its own box (theme `background`, 1 px border, radius) and a shadow of its own,
black at 10%, set inside its open animation: a `.shadow(..)` on the dialog is overwritten on every
frame, and the shadow does not show over Vano's scrim. Its content is clipped to the box (an
`overflow_hidden` wrapper), so a shadow drawn by the content is cut at the box edge.
`components::modal::framed_dialog` works around both: it makes the box transparent, borderless and
larger than the visible card by the room the card's shadow needs (less on a small window), and
draws the card inside it. The card then slides and fades in with the dialog. The box also drops
its close button (it would sit in the transparent room), so modals draw their own in the header.

gpui clips children to a rectangle, never to rounded corners: a child that paints a background
into a rounded parent's corner must round that corner itself.

When a dialog closes, `Root` focuses whatever was focused when it opened, through a weak handle.
Opened from a popup menu, that is the menu, gone by then, so nothing gets the focus and window
shortcuts stop working. A dialog closed with Enter restores it 250 ms later
(`defer_close_dialog`), with the same result. `AppView` registers `cx.on_focus_lost` and takes
the focus back whenever nothing has it and no dialog is open (#96). gpui draws no frame after a
focus-lost listener moves the focus, so it defers a `window.refresh()`: without that frame, a
second loss is never reported.

## Single-line inputs

An `Input` has a default height of 32 px (`h_8`) and a default vertical padding of 8 px, which
leaves 16 px for a line that is 20 px tall. The text is clipped to that content box, so the bottom
of every descender is cut (`Content-Type` reads as `Content-Tvbe`, `application/json` as
`application/ison`), at any display scale. Always call `.py_0()` on a single-line `Input`, with or
without a fixed `.h(..)`, or build it with `components::text_field`. Making the box taller does not
help, only the padding matters. Found in #67; the sample suite request `headers/custom` shows the
header table, which is the quickest place to check it.

## Charts

`chart::LineChart` is a **single-series** element whose y scale always starts at 0 and is built
from its own data. There is no dual-axis mode. That is exactly what two series "each normalized
to its own max" need: stack two `LineChart`s with `.absolute()` inside the same bounds, each with
`x_axis(false)` and `grid(false)`. The `Y` type must be `f64`; there is no `f32` implementation,
so keep metrics as `f64` before handing them to a chart. Histograms and status-code bars need no
chart widget: plain `div()`s with a computed size and a background color.

## Periodic work

There is no standalone timer type. Use `cx.background_executor().timer(duration)` inside a
`cx.spawn`. For a refresh that runs only while something is active (the load test dashboard,
every 250 ms), a plain loop in one `cx.spawn` is enough; dropping the `Task` cancels it. A task
that reschedules itself needs an epoch counter so a stale reschedule becomes a no-op instead of
two timers racing (the cursor blink does this). Remember the idle rule: a timer that calls
`cx.notify()` while nothing changes re-renders the whole window.

## Title bar and window controls

`TitleBar` appends the platform's `WindowControls` itself, as the last child. On macOS it renders
nothing and the OS traffic lights stay. On Linux under server-side decorations it renders
nothing, so the controls never double up with the compositor's. Under real client-side
decorations it renders minimize, maximize and close, but only the ones the window manager
reports as supported. So `TitleBar::new().child(..)` with the app's own content is the whole
implementation, with `WindowDecorations::Client` in the window options on Linux.

## Debug hooks

`VANO_ENV`, `VANO_AUTOSEND` and `VANO_OPEN` are read once at startup, do nothing when
unset, appear in no menu and are not a supported feature. They exist so a screenshot of a given
UI state can be taken without clicking (`VANO_OPEN` takes `components`, `settings`,
`settings-requests`, `settings-advanced`, `settings-about`, `palette`, `snippet`, `loadtest` or `env:<name>`; see `state/debug_open.rs`). Opening a dialog needs a `Window`, so the hook runs where a window is
available.
