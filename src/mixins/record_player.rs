// Runs as `line` is passed to `signed_chat_line` (the call sites' destructured
// `sender` local can't be captured): the record keys the signed player line;
// sender UUID 0 means no UUID match, the name heuristics still apply.
fn record_player(line: horizon_client::text::Text) -> horizon_client::text::Text {
    ::chat_heads_rs::note_player(line.strip_legacy().plain(), 0);
    line
}
