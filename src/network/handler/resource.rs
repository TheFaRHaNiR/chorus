use crate::config::Config;
use crate::network::handler::PacketReceivedMessage;
use crate::network::session::Session;
use crate::network::session::state::{SessionState, SessionStateChangedMessage};
use bedrock::protocol::v662::enums::ResourcePackResponse;
use bedrock::protocol::v662::packets::ResourcePackChunkDataPacket;
use bedrock::protocol::v662::types::{BaseGameVersion, Experiments};
use bedrock::protocol::v818::packets::ResourcePacksInfoPacket;
use bedrock::protocol::v898::packets::ResourcePackStackPacket;
use bedrock::protocol::{ProtoVersionPackets, V944};
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{MessageWriter, ParamSet, Query, Res};

pub fn handle_resource(
    config: Res<Config>,
    mut packet_reader: MessageReader<PacketReceivedMessage>,
    mut state_message_set: ParamSet<(MessageReader<SessionStateChangedMessage>, MessageWriter<SessionStateChangedMessage>)>,
    mut sessions: Query<&mut Session>,
) {
    for ev in state_message_set.p0().read() {
        if ev.to != SessionState::Resource {
            continue;
        };

        let Ok(mut session) = sessions.get_mut(ev.entity) else {
            continue;
        };

        // TODO
        session.send(V944::ResourcePacksInfoPacket(ResourcePacksInfoPacket {
            resource_pack_required: config.force_accept_resource_packs,
            has_addon_packs: false,
            has_scripts: false,
            force_disable_vibrant_visuals: config.force_disable_vibrant_visuals,
            world_template_uuid: Default::default(),
            world_template_version: "".to_string(),
            resource_packs: vec![],
        }))
    }

    for ev in packet_reader.read() {
        let Ok(mut session) = sessions.get_mut(ev.entity) else {
            continue;
        };

        match &ev.packet {
            V944::ResourcePackChunkRequestPacket(packet) => handle_request(&mut session, packet),
            V944::ResourcePackClientResponsePacket(packet) => handle_response(&mut session, packet, &mut state_message_set.p1()),
            _ => continue,
        }
    }
}

fn handle_request(session: &mut Session, _packet: &<V944 as ProtoVersionPackets>::ResourcePackChunkRequestPacket) {
    // TODO
    session.send(V944::ResourcePackChunkDataPacket(ResourcePackChunkDataPacket {
        resource_name: "".to_string(),
        chunk_id: 0,
        byte_offset: 0,
        chunk_data: "".to_string(),
    }))
}

fn handle_response(session: &mut Session, packet: &<V944 as ProtoVersionPackets>::ResourcePackClientResponsePacket, state_writer: &mut MessageWriter<SessionStateChangedMessage>) {
    match packet.response {
        ResourcePackResponse::Cancel => session.close(Some("disconnectionScreen.noReason")),
        ResourcePackResponse::Downloading => {
            // TODO
        }
        ResourcePackResponse::DownloadingFinished => session.send(V944::ResourcePackStackPacket(ResourcePackStackPacket {
            texture_pack_required: false,
            addon_list: vec![],
            base_game_version: BaseGameVersion("*".to_string()),
            experiments: Experiments {
                experiments: vec![],
                ever_toggled: false,
            },
            include_editor_packs: false,
        })),
        ResourcePackResponse::ResourcePackStackFinished => {
            session.set_state(SessionState::Setup, state_writer);
        }
    }
}
