use bevy_app::{App, Plugin, Update};

use crate::{
    connection::poll_connections,
    endpoint::{find_new_connections, poll_endpoints},
    incoming::handle_incoming_responses,
};

#[derive(Debug)]
pub struct QuinnPlugin;

impl Plugin for QuinnPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                find_new_connections,
                poll_endpoints,
                poll_connections,
                handle_incoming_responses,
            ),
        );
    }
}
