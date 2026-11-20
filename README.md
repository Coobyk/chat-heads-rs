# Chat Heads

A native [Horizon](https://playhorizon.dev) port of
[dzwdz/chat_heads](https://github.com/dzwdz/chat_heads) (Fabric): it draws the
sender's 8×8 player face — the same face the Tab list draws — before their chat
messages.

## How it works

Three mixins and one injected module (`src/heads.rs`,
`crate::game::umbx_chat_heads`):

- **`record_add`** — `HEAD` on `horizon_client_render::chat::Chat::add`.
  Every server/system/client line's whole plain text goes into a small
  ring buffer (`src/lib.rs`, shared through the mod crate's own name).
- **`record_player`** — `AFTER_CALL` on `self.chat_received` inside
  `Game::player_chat_line`. Signed/unsigned player lines carry the packet's
  sender UUID here (and only here), so the record keeps `sender` next to the
  line text it was about to show.
- **`chat_draw`** — `OVERWRITE` of `Game::draw_chat`. The body is the
  original verbatim except the final `chat.draw_at` becomes
  `umbx_chat_heads::draw`, which re-does `Chat::layout` → decorate →
  `hud_layout::paint` plus the two prompts `draw_at` paints (restricted
  chat, PvP & HUD chat search).

The decorator walks the laid-out `GuiDraw`s. Each `Text` draw is one wrapped
chat line: a line that prefixes a recorded message's whole text is that
message's first line and gets its head; a line that is a substring of one is
a continuation. The head itself is `tab_list::face`'s `GuiDraw::Blit`s — the
face and hat regions of the skin textures the game already fetched for the
Tab list (plus `Game::face_skin` so pending downloads resolve to the default
skin instead of nothing) — tinted with the line's own alpha so heads fade
with their messages.

### Skin choice (first version)

No extra texture loading: heads reuse the skin paths the remote already
published into the HUD atlas (`entity/player/…`), with `TabPlayer::show_hat`
and the Dinnerbone/Grumm flip. A player whose skin is still downloading draws
as their default skin (Steve/Alex) — exactly what the Tab list shows at that
moment. Players found only by name heuristics get the matched tab-list
entry's skin; when no sender can be found the line gets no head and, if
"Indent Lines" is on, the same text column as headed lines.

## Settings

Options → Mods → Chat Heads (translated via `assets/chat_heads/lang`):

- **Chat Heads** — master toggle.
- **Head Position** — `Before Name` (upstream's `BEFORE_NAME` default) or
  `Before Line`.
- **Sender Detection** — `UUID & Heuristics` (default), `UUID Only`,
  `Heuristics Only`, `Off`.
- **Heads on System Messages** — name heuristics on non-player lines
  (upstream's `handleSystemMessages`).
- **Indent Lines Without Head** — upstream's `offsetNonPlayerText`.
- **Head Shadow** — upstream's `drawShadow`.

## Differences and limits versus the Fabric mod

- No `threeDeeNess` (3-D head extrusion), no `nameAliases`, no
  `rightPadding`/`smartHeuristics` switches, and no config-file import: the
  settings above are the whole surface in this first version.
- `BEFORE_NAME` places the head at the player name found in the *first* line's
  text; if the name is absent it falls back to the line start, like upstream's
  `BEFORE_LINE` fallback.
- Own/outgoing messages and client-side lines are recorded without a UUID and
  rely on name heuristics, same as upstream treats non-packet lines.

## Build

```sh
rsync -az --delete -e /tmp/ibssh . rowser:mods/chat-heads-rs/
instabox ssh rowser '~/.local/bin/horizon-launcher native-build --trust-native-code ~/mods/chat-heads-rs'
```
