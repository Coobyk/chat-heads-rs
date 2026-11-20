// `Game::draw_chat` with `self` renamed `game`: the whole HUD chat draw, so
// the tail — `chat.draw_at` — runs through `umbx_chat_heads::draw`, which
// re-does its `layout`/`paint` with the heads inserted.
fn chat_draw(game: &mut Game<'_>, batch: &mut HudBatch) {
    let focused = game.screen == Screen::Chat;
    let options = game.pvp_chat();
    game.ui.chat.set_qol(&game.gui.atlas, options.qol());
    if game.pvp.chat_search && game.ui.chat_screen.is_none() {
        game.pvp.chat_search = false;
        game.pvp.chat_search_aside.clear();
    } else if game.pvp.chat_search && !options.search {
        game.toggle_chat_search();
    }
    let query = game
        .ui
        .chat_screen
        .as_ref()
        .filter(|_| focused && game.pvp.chat_search)
        .map(|s| s.value().to_owned());
    game.ui.chat.set_search(&game.gui.atlas, query.as_deref());
    game.ui.chat.search_prompt = focused && game.pvp.chat_search;
    game.ui.chat.queue = game.chat_queue_len();
    if game.ui.hide_gui && !focused {
        return;
    }
    game.ui.chat.smooth = game.settings.extras.chat_animation;
    game.ui.chat.hide_indicator = !game.settings.extras.chat_indicator;
    let now = game.visual_ticks() as f32 + game.clock.fraction() as f32;
    let (width, height) = (batch.width as i32, batch.height as i32);
    let (home_x, home_y) = (0, height - 40);
    let (mut x, mut y) = (home_x, home_y);
    game.native_customize_chat_overlay(width, height, focused, &mut x, &mut y);
    let offset = [(x - home_x) as f32, (y - home_y) as f32];
    crate::game::umbx_chat_heads::draw(game, batch, now, focused, offset, query.as_deref());
}
