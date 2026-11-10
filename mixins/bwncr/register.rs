// Runs at the head of `Game::native_register_commands` (API 8): declare the
// `/bwncr …` usages in the server's command tree.
fn observe_register_commands(commands: &mut Vec<crate::game::native_server::NativeCommand>) {
    crate::game::bwncr_state::on_register_commands(commands);
}
