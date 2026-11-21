// Injected into the game crate as `crate::game::umbx_chat_heads` (see
// `horizon-mod.toml`): everything the mixins call that needs game or render
// types. The message records themselves live in the mod crate
// (`chat_heads_rs`), shared with the `record_player` mixin.

use horizon_client::{
    chat::ChatSource,
    hud::{HudAtlas, HudBatch},
    hud_layout::{self, GuiDraw, PoseOp},
    tab_list,
    text::{Style, Text},
};

pub const ID: &str = "chat_heads";
/// The space a head takes on a chat line: one pixel, the 8px face, one pixel.
pub const HEAD_W: i32 = 10;
const FACE: i32 = 8;

#[derive(Clone, Copy)]
pub struct Settings {
    pub enabled: bool,
    /// true = before the player's name in the first line, false = before the
    /// first line (`RenderPosition` in the original mod).
    pub before_name: bool,
    /// `offsetNonPlayerText`: indent every line so heads never unalign text.
    pub offset: bool,
    /// `drawShadow`: a dark copy of the face one pixel down-right.
    pub shadow: bool,
}

pub fn settings(game: &crate::game::Game<'_>) -> Settings {
    let get = |key: &str| game.setting(ID, key);
    let on = |v: Option<String>| v.map_or(true, |v| v.trim() != "false");
    Settings {
        enabled: on(get("enabled")),
        before_name: get("position").map_or(true, |v| {
            let v = v.trim();
            v != "1" && v != "before_line"
        }),
        offset: on(get("offset_text")),
        shadow: on(get("head_shadow")),
    }
}

/// A player's face as the tab list resolves it.
struct PlayerEntry {
    name: String,
    skin: String,
    hat: bool,
    flip: bool,
}

/// A head for one message: the skin path in the HUD atlas, and the name to
/// find in the line's text for BEFORE_NAME placement.
struct Head {
    skin: String,
    hat: bool,
    flip: bool,
    name: String,
}

/// The players the client knows: the tab list's on a server (with the skins
/// it resolved and downloaded into the atlas), the local player and LAN
/// guests otherwise, mirroring `social_players`/`draw_tab_list`.
fn collect_players(game: &mut crate::game::Game<'_>) -> Vec<PlayerEntry> {
    match game.remote_tab_players() {
        Some((players, _online_mode, upside_down)) => {
            let mut out = Vec::with_capacity(players.len());
            for p in players {
                // `face_skin` keeps the texture atlas-resident (default skin
                // until the download arrives).
                let skin = game.face_skin(p.uuid, &p.skin);
                out.push(PlayerEntry {
                    name: p.name,
                    skin,
                    hat: p.show_hat,
                    flip: upside_down.contains(&p.uuid),
                });
            }
            out
        }
        None => {
            let uuid = game.level.player_uuid.map_or(0, |u| {
                u.iter()
                    .fold(0u128, |a, &w| (a << 32) | u128::from(w as u32))
            });
            let mut me = tab_list::TabPlayer::new(uuid, crate::game::chat::PLAYER_NAME);
            me.skin = "entity/player/wide/steve".into();
            let guests = game.lan.as_ref().map_or(&[][..], |h| &h.hub.guests);
            crate::game::scoreboard::local_tab_players(me, guests)
                .into_iter()
                .map(|p| PlayerEntry {
                    flip: p.name == "Dinnerbone" || p.name == "Grumm",
                    name: p.name,
                    skin: p.skin,
                    hat: p.show_hat,
                })
                .collect()
        }
    }
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The earliest occurrence of a known player name in `whole` at a name
/// boundary (`<Name> `, `* Name`, `Name whispers`, join/leave and death lines,
/// ...). Longest name wins a tie, so "Al" never shadows "Alex".
fn name_scan<'a>(whole: &str, players: &'a [PlayerEntry]) -> Option<(String, &'a PlayerEntry)> {
    let mut best: Option<(usize, usize, &'a PlayerEntry)> = None;
    for p in players {
        if p.name.len() < 3 {
            continue;
        }
        for (pos, _) in whole.match_indices(&p.name) {
            let before_ok = whole[..pos]
                .chars()
                .next_back()
                .is_none_or(|c| !is_name_char(c));
            let after_ok = whole[pos + p.name.len()..]
                .chars()
                .next()
                .is_none_or(|c| !is_name_char(c));
            if !(before_ok && after_ok) {
                continue;
            }
            let better = match best {
                None => true,
                Some((bpos, blen, _)) => pos < bpos || (pos == bpos && p.name.len() > blen),
            };
            if better {
                best = Some((pos, p.name.len(), p));
            }
        }
    }
    best.map(|(_, _, p)| (p.name.clone(), p))
}

/// The head a message record draws: the earliest known player name the
/// heuristics find in its whole text.
fn resolve(rec: &chat_heads_rs::Record, players: &[PlayerEntry]) -> Option<Head> {
    name_scan(&rec.whole, players).map(|(name, p)| Head {
        skin: p.skin.clone(),
        hat: p.hat,
        flip: p.flip,
        name,
    })
}

/// The face's draws at (x, y): the same `PlayerFaceExtractor` blits the tab
/// list uses, `color` fading with the line's text alpha; `shadow` first, a
/// dark copy a pixel down-right like the font's own drop shadow.
fn head_draws(out: &mut Vec<GuiDraw>, head: &Head, x: i32, y: i32, color: u32, shadow: bool) {
    if shadow {
        let dark = (color & 0xFF00_0000) | 0x003F_3F3F;
        tab_list::face(out, &head.skin, x + 1, y + 1, FACE, head.hat, head.flip, dark);
    }
    tab_list::face(out, &head.skin, x, y, FACE, head.hat, head.flip, color);
}

/// `Chat::layout`'s draws with heads added and the text moved for them: every
/// `Text` draw is one wrapped line; the record whose whole text it prefixes
/// is the message's first line (and gets the head), a line inside one is a
/// continuation, anything else stands alone.
fn decorate(
    atlas: &HudAtlas,
    draws: Vec<GuiDraw>,
    recs: &[chat_heads_rs::Record],
    players: &[PlayerEntry],
    s: &Settings,
    shown_lines: &mut i32,
) -> Vec<GuiDraw> {
    let indent = if s.offset { HEAD_W } else { 0 };
    let mut out = Vec::with_capacity(draws.len() + 16);
    for d in draws {
        let GuiDraw::Text {
            text,
            x,
            y,
            color,
            shadow,
        } = d
        else {
            out.push(d);
            continue;
        };
        // The Chat Delay queue row is a Text draw as well; it is not a
        // message's line.
        if text.translation_key() == Some("chat.queue") {
            out.push(GuiDraw::Text {
                text,
                x,
                y,
                color,
                shadow,
            });
            continue;
        }
        *shown_lines += 1;
        let plain = text.plain();
        let plain = chat_heads_rs::strip_timestamp(&plain);
        if let Some(rec) = chat_heads_rs::first_line_of(plain, recs) {
            // A message's first line.
            match resolve(rec, players) {
                Some(head) if s.before_name => {
                    // Where the name sits in this line (the timestamp prefix
                    // shifts it; a name that never shows falls back to the
                    // line's start, as the original does).
                    let drawn = text.plain();
                    let k = if head.name.is_empty() {
                        0
                    } else {
                        drawn
                            .find(&head.name)
                            .map(|b| drawn[..b].chars().count())
                            .unwrap_or(0)
                    };
                    let n = text.chars().count();
                    let before = text.slice(0, k);
                    let bw = atlas.font.width_text(&before) as i32;
                    out.push(GuiDraw::Text {
                        text: before,
                        x: x + indent,
                        y,
                        color,
                        shadow,
                    });
                    head_draws(&mut out, &head, x + indent + bw + 1, y, color, s.shadow);
                    out.push(GuiDraw::Text {
                        text: text.slice(k, n),
                        x: x + indent + bw + HEAD_W,
                        y,
                        color,
                        shadow,
                    });
                }
                Some(head) => {
                    head_draws(&mut out, &head, x + 1, y, color, s.shadow);
                    out.push(GuiDraw::Text {
                        text,
                        x: x + HEAD_W,
                        y,
                        color,
                        shadow,
                    });
                }
                None => out.push(GuiDraw::Text {
                    text,
                    x: x + indent,
                    y,
                    color,
                    shadow,
                }),
            }
        } else if plain.len() >= 4 && chat_heads_rs::continuation_of(plain, recs).is_some() {
            // A wrapped line of a headed message: the same text column, no
            // head (the record's head stays on the first line).
            out.push(GuiDraw::Text {
                text,
                x: x + indent,
                y,
                color,
                shadow,
            });
        } else {
            out.push(GuiDraw::Text {
                text,
                x: x + indent,
                y,
                color,
                shadow,
            });
        }
    }
    out
}

/// `Chat::draw_at`'s pose for the chat-offset hook (`native_customize_chat_overlay`).
fn paint_offset(batch: &mut HudBatch, atlas: &HudAtlas, draws: &[GuiDraw], offset: [f32; 2]) {
    if offset == [0.0; 2] {
        hud_layout::paint(batch, atlas, draws);
    } else {
        let mut moved = Vec::with_capacity(draws.len() + 2);
        moved.push(GuiDraw::Push(vec![PoseOp::Translate(offset[0], offset[1])]));
        moved.extend_from_slice(draws);
        moved.push(GuiDraw::Pop);
        hud_layout::paint(batch, atlas, &moved);
    }
}

/// `Chat::prompt_above` (private): a row above the shown lines on the chat's
/// background, clipped to the chat's width. `shown` is the visible line count.
fn prompt_above(
    chat: &horizon_client::chat::Chat,
    atlas: &HudAtlas,
    height: i32,
    prompt: Text,
    row: i32,
    shown: i32,
) -> Vec<GuiDraw> {
    let font = &atlas.font;
    let o = &chat.options;
    let scale = o.scale as f32;
    let max_width = hud_layout::ceil(o.width_px() as f32 / scale);
    let chat_bottom = hud_layout::floor((height - 40) as f32 / scale);
    let entry_height = o.line_height();
    let bottom_to_text =
        (8.0 * (o.line_spacing + 1.0) - 4.0 * o.line_spacing + 0.5).floor() as i32;
    let top = chat_bottom - (shown + row) * entry_height;
    let mut prompt = prompt;
    // ComponentRenderUtils.clipText.
    let mut width = 0.0;
    let fits = prompt
        .chars()
        .take_while(|(c, style)| {
            width += font.advance(*c, style);
            width <= max_width as f32
        })
        .count();
    if fits < prompt.chars().count() {
        prompt = prompt.slice(0, fits);
    }
    vec![
        GuiDraw::Push(vec![
            PoseOp::Scale(scale, scale),
            PoseOp::Translate(4.0, 0.0),
        ]),
        hud_layout::fill(
            -2,
            top,
            max_width + 4 + 4,
            top + entry_height,
            hud_layout::as8(o.background_opacity as f32) << 24,
        ),
        GuiDraw::Text {
            text: prompt,
            x: 0,
            y: top + entry_height - bottom_to_text - 1,
            color: hud_layout::white(o.opacity as f32 * 0.9 + 0.1),
            shadow: true,
        },
        GuiDraw::Pop,
    ]
}

/// The `chat_draw` mixin's draw: `Chat::draw_at` recomposed as `layout` →
/// decorate → `paint`, with its restricted and chat-search prompts after.
pub fn draw(
    game: &mut crate::game::Game<'_>,
    batch: &mut HudBatch,
    now: f32,
    focused: bool,
    offset: [f32; 2],
    _query: Option<&str>,
) {
    let s = settings(game);
    if !s.enabled {
        game.ui
            .chat
            .draw_at(batch, &game.gui.atlas, now, focused, offset);
        return;
    }
    let height = batch.height as i32;
    let draws = game.ui.chat.layout(height, now, focused);
    let players = collect_players(game);
    let recs = chat_heads_rs::records();
    let mut shown = 0;
    let out = decorate(&game.gui.atlas, draws, &recs, &players, &s, &mut shown);
    paint_offset(batch, &game.gui.atlas, &out, offset);
    // `draw_at`'s extras: the restricted prompt (shown when the Chat option
    // or the server restricts chat) and the chat-search prompt.
    let restricted = focused
        && !(game.ui.chat.shows(ChatSource::Player) && game.ui.chat.shows(ChatSource::Server));
    if restricted {
        let prompt = Text::translate("chat_screen.restricted", &[]).with_style(&Style {
            color: Some(horizon_client::text::RED),
            underlined: Some(true),
            ..Style::default()
        });
        let d = prompt_above(&game.ui.chat, &game.gui.atlas, height, prompt, 1, shown);
        paint_offset(batch, &game.gui.atlas, &d, offset);
    }
    if focused && game.ui.chat.search_prompt {
        let row = if restricted { 2 } else { 1 };
        let label = Text::translate_with_fallback("horizon.chat.search", "Search chat:", &[])
            .with_color(horizon_client::text::YELLOW);
        let d = prompt_above(&game.ui.chat, &game.gui.atlas, height, label, row, shown);
        paint_offset(batch, &game.gui.atlas, &d, offset);
    }
}
