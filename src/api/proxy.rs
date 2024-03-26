use std::io::Cursor;

use bevy::utils::HashMap;
use bevy_kira_audio::AudioSource;
use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};

use crate::api::{get, get_query, ServerApi};
use crate::level::{de, LevelData, LevelInfo, SongInfo};

pub(crate) struct ProxyApi {
    server: String,
}

impl ProxyApi {
    fn new(server: String) -> Self {
        Self { server }
    }
}

impl Default for ProxyApi {
    fn default() -> Self {
        Self::new("https://gd-server-proxy.opstic.workers.dev".to_string())
    }
}

impl ServerApi for ProxyApi {
    async fn search_levels(
        &self,
        query: String,
    ) -> Result<(Vec<LevelInfo>, HashMap<u64, SongInfo>), anyhow::Error> {
        let body = get_query(&format!("{}/search", self.server), &[("query", &query)]).await?;

        let split: Vec<&str> = de::from_str(simdutf8::basic::from_utf8(&body)?, '#')?;

        let level_infos = if let Some(level_infos) = split.first() {
            let level_info_strings: Vec<&str> = de::from_str(level_infos, '|')?;
            level_info_strings
                .iter()
                .map(|level_info_string| de::from_str(level_info_string, ':'))
                .collect::<Result<Vec<LevelInfo>, _>>()
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let song_infos = if let Some(song_infos) = split.get(2) {
            let song_info_strings: Vec<&str> = de::from_str(song_infos, ':')?;
            song_info_strings
                .iter()
                .filter_map(|song_info_string| {
                    de::from_str_str::<SongInfo>(
                        song_info_string.trim_matches('~'),
                        "~|~".to_string(),
                    )
                    .ok()
                })
                .collect::<Vec<SongInfo>>()
        } else {
            Vec::new()
        };

        let song_infos = song_infos
            .iter()
            .map(|song_info| (song_info.id, song_info.clone()))
            .collect();

        Ok((level_infos, song_infos))
    }

    async fn get_level_data(&self, id: u64) -> Result<LevelData, anyhow::Error> {
        let body = get(&format!("{}/level/{}", self.server, id)).await?;

        let body = simdutf8::basic::from_utf8(&body)?;

        Ok(de::from_str(body, ':')?)
    }

    async fn get_song(&self, song_info: SongInfo) -> Result<AudioSource, anyhow::Error> {
        let body = get_query(&format!("{}/song", self.server), &[("url", &song_info.url)]).await?;
        let sound_data =
            StaticSoundData::from_cursor(Cursor::new(body), StaticSoundSettings::new())?;
        Ok(AudioSource { sound: sound_data })
    }
}
