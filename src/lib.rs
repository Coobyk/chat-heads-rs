//! Chat Heads' own crate.
//!
//! Besides the settings page (Options > Mods > Chat Heads) it holds the
//! message/sender records the mixins share: `record_add` (every `Chat::add`)
//! and `record_player` (each received player line's sender UUID) push here,
//! and the draw-side module under `src/heads.rs` reads them back to decide
//! which rendered chat line gets a head. Only plain data crosses the
//! boundary, so the same store works from both patched crates.

use std::collections::VecDeque;
use std::sync::Mutex;

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl mod_api::Hooks for Mod {
    fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        let _ = r.register_setting(
            mod_api::SettingSpec::toggle("enabled", "chat_heads.setting.enabled", true)
                .tooltip("chat_heads.setting.enabled.tooltip"),
        );
        let _ = r.register_setting(
            mod_api::SettingSpec::choice(
                "position",
                "chat_heads.setting.position",
                [
                    "chat_heads.position.before_name",
                    "chat_heads.position.before_line",
                ],
                0,
            )
            .tooltip("chat_heads.setting.position.tooltip"),
        );
        let _ = r.register_setting(
            mod_api::SettingSpec::choice(
                "sender_detection",
                "chat_heads.setting.sender_detection",
                [
                    "chat_heads.detection.uuid_and_heuristic",
                    "chat_heads.detection.uuid_only",
                    "chat_heads.detection.heuristic_only",
                    "chat_heads.detection.off",
                ],
                0,
            )
            .tooltip("chat_heads.setting.sender_detection.tooltip"),
        );
        let _ = r.register_setting(
            mod_api::SettingSpec::toggle(
                "system_messages",
                "chat_heads.setting.system_messages",
                true,
            )
            .tooltip("chat_heads.setting.system_messages.tooltip"),
        );
        let _ = r.register_setting(
            mod_api::SettingSpec::toggle(
                "offset_text",
                "chat_heads.setting.offset_text",
                true,
            )
            .tooltip("chat_heads.setting.offset_text.tooltip"),
        );
        let _ = r.register_setting(
            mod_api::SettingSpec::toggle(
                "head_shadow",
                "chat_heads.setting.head_shadow",
                true,
            )
            .tooltip("chat_heads.setting.head_shadow.tooltip"),
        );
    }
}

/// A chat message seen by the intake mixins. Newest first, like `Chat`'s own
/// message list.
#[derive(Clone, Debug)]
pub struct Record {
    /// `content.strip_legacy().plain()`: the whole message's text, before it
    /// is wrapped into lines (timestamps and the " (xN)" counter are not in
    /// it — they are added per displayed line).
    pub whole: String,
    /// The sender's UUID when the packet gave one; `None` for system,
    /// disguised and client-side lines.
    pub uuid: Option<u128>,
    /// `ChatSource::Player`: heuristics always apply to these, system lines
    /// only when the setting allows.
    pub player: bool,
}

/// The chat keeps at most 100 wrapped lines; 150 whole messages always cover
/// the scrollback while bounded.
const MAX_RECORDS: usize = 150;
static RECORDS: Mutex<VecDeque<Record>> = Mutex::new(VecDeque::new());

/// A line from `Chat::add` (server, system or client-side; signed player
/// messages are not added through it).
pub fn note_add(whole: String, player: bool) {
    push(Record {
        whole,
        uuid: None,
        player,
    });
}

/// A player message about to show, with the packet's sender (0 = disguised).
pub fn note_player(whole: String, sender: u128) {
    push(Record {
        whole,
        uuid: (sender != 0).then_some(sender),
        player: true,
    });
}

fn push(rec: Record) {
    if rec.whole.is_empty() {
        return;
    }
    let mut rs = RECORDS.lock().unwrap_or_else(|e| e.into_inner());
    rs.push_front(rec);
    rs.truncate(MAX_RECORDS);
}

/// A snapshot of the records, newest first.
pub fn records() -> Vec<Record> {
    RECORDS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .cloned()
        .collect()
}

/// The "[hh:mm] " prefix PvP & HUD's chat timestamps prepend to a message's
/// first wrapped line.
pub fn strip_timestamp(line: &str) -> &str {
    let b = line.as_bytes();
    if b.len() >= 8
        && b[0] == b'['
        && b[3] == b':'
        && b[6] == b']'
        && b[7] == b' '
        && b[1].is_ascii_digit()
        && b[2].is_ascii_digit()
        && b[4].is_ascii_digit()
        && b[5].is_ascii_digit()
    {
        &line[8..]
    } else {
        line
    }
}

/// Index of the newest record `line` starts (a message's first wrapped line
/// is a prefix of its whole text; `whole`-prefixes-`line` covers the " (xN)"
/// repeat counter at the end of a last wrapped line).
pub fn first_line_of<'a>(line: &str, recs: &'a [Record]) -> Option<&'a Record> {
    if line.is_empty() {
        return None;
    }
    let mut best: Option<(&Record, usize)> = None;
    for r in recs {
        let prefix = r.whole.starts_with(line) || line.starts_with(r.whole.as_str());
        if !prefix {
            continue;
        }
        let score = r.whole.len().min(line.len());
        if best.is_none_or(|(_, s)| score > s) {
            best = Some((r, score));
        }
    }
    best.map(|(r, _)| r)
}

/// Index of a record `line` continues (a wrapped line is a substring of its
/// message's whole text away from the start).
pub fn continuation_of<'a>(line: &str, recs: &'a [Record]) -> Option<&'a Record> {
    if line.is_empty() {
        return None;
    }
    recs.iter()
        .find(|r| r.whole.contains(line) && !r.whole.starts_with(line))
}
