use async_compat::Compat;
use bevy::app::{App, Plugin};
use bevy::log::{error, info};
use bevy::prelude::{DetectChanges, ResMut, Resource};
use discord_sdk::activity::ActivityArgs;
use discord_sdk::wheel::{UserSpoke, UserState, Wheel};
use discord_sdk::{AppId, Discord, DiscordApp, Subscriptions};
use futures_lite::future;

const DISCORD_APP_ID: AppId = 1129481858116243506;

pub(crate) struct DiscordPlugin;

impl Plugin for DiscordPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DiscordClient::new(DISCORD_APP_ID))
            .init_resource::<CurrentDiscordActivity>()
            .add_system(update_discord_status);
    }
}

#[derive(Default, Resource)]
pub(crate) struct CurrentDiscordActivity(pub(crate) ActivityArgs);

#[derive(Resource)]
pub(crate) struct DiscordClient {
    pub(crate) discord: Discord,
    pub(crate) user: UserSpoke,
    pub(crate) wheel: Wheel,
    pub(crate) update_deferred: bool,
}

impl DiscordClient {
    fn new(app_id: AppId) -> DiscordClient {
        info!("Initializing Discord integration...");
        future::block_on(Compat::new(async {
            let (wheel, handler) = Wheel::new(Box::new(|err| {
                error!("Error creating event wheel: {}", err);
            }));

            let discord = Discord::new(
                DiscordApp::PlainId(app_id),
                Subscriptions::ACTIVITY,
                Box::new(handler),
            )
            .unwrap();

            info!("Waiting for Discord handshake...");

            DiscordClient {
                discord,
                user: wheel.user(),
                wheel,
                update_deferred: false,
            }
        }))
    }
}

fn update_discord_status(
    mut discord_client: ResMut<DiscordClient>,
    mut current_activity: ResMut<CurrentDiscordActivity>,
) {
    if discord_client.user.0.has_changed().unwrap() {
        match &*discord_client.user.0.borrow_and_update() {
            UserState::Connected(_) => info!("Successfully connected to Discord"),
            UserState::Disconnected(err) => error!("Discord connection failed: {}", err),
        }
    }
    if current_activity.is_changed() || discord_client.update_deferred {
        if matches!(&*discord_client.user.0.borrow(), UserState::Connected(_)) {
            let activity_result = future::block_on(
                discord_client
                    .discord
                    .update_activity(std::mem::take(&mut current_activity.0)),
            );
            if let Err(err) = activity_result {
                error!("Error when setting Discord activity: {}", err);
            }
            discord_client.update_deferred = false;
        } else {
            discord_client.update_deferred = true;
        }
    }
}
