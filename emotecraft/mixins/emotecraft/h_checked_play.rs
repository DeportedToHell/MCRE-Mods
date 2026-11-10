// A guest's packet as the server's decoder checks it: Emotecraft's messages may be up to 1 MiB,
// as Emotecraft's servers (the Fabric mod's codec, the Paper plugin's decoder) allow on its channel.
fn emotecraft_checked_play(packet: &play::Serverbound) -> core::ops::ControlFlow<horizon_protocol::Result<play::Serverbound>> {
    if let play::Serverbound::CustomPayload(p) = packet
        && p.channel == "emotecraft:emote"
        && p.data.len() <= 1_048_576
    {
        return core::ops::ControlFlow::Break(Ok(packet.clone()));
    }
    core::ops::ControlFlow::Continue(())
}
