use bevy::audio::AudioSource;
use bevy::utils::HashMap;

use crate::level::{LevelData, LevelInfo, SongInfo};

pub(crate) mod robtop;

pub(crate) trait ServerApi {
    async fn search_levels(
        &self,
        query: String,
    ) -> Result<(Vec<LevelInfo>, HashMap<u64, SongInfo>), anyhow::Error>;
    async fn get_level_data(&self, id: u64) -> Result<LevelData, anyhow::Error>;
    async fn get_song(&self, song_info: SongInfo) -> Result<AudioSource, anyhow::Error>;
}
