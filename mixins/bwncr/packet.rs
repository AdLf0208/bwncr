// Runs at the head of `Game::native_client_packet` (API 408): name the sound
// packets the server sends while `/bwncr listen` is on.
fn observe_client_packet(
    game: &mut crate::game::Game<'_>,
    packet: &horizon_protocol::packets::play::Clientbound,
) {
    crate::game::bwncr_state::on_client_packet(game, packet);
}
