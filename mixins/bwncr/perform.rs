// Runs at the head of `Game::native_perform_command` (API 8): answer
// `/bwncr status|check|listen|heard|silence|allow|reload`.
fn observe_perform_command(
    game: &mut crate::game::Game<'_>,
    player: &crate::game::native_server::NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::bwncr_state::on_perform_command(game, player, command, ran);
}
