// Runs at the head of `Game::native_server_about_to_start` (API 3): read the
// mod's `config/bwncr-common.toml`, writing the upstream defaults when absent.
fn observe_server_about_to_start(game: &mut crate::game::Game<'_>) {
    crate::game::bwncr_state::on_server_about_to_start(game);
}
