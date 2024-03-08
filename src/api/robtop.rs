use std::io::{Cursor, Read};

use bevy::utils::HashMap;
use bevy_kira_audio::AudioSource;
use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};

use crate::api::ServerApi;
use crate::level::{de, LevelData, LevelInfo, SongInfo};

pub(crate) struct RobtopApi {
    server: String,
}

impl RobtopApi {
    fn new(server: String) -> Self {
        Self { server }
    }
}

impl Default for RobtopApi {
    fn default() -> Self {
        Self::new("http://www.boomlings.com/database/".to_string())
    }
}

const COMMON_SECRET: &str = "Wmfd2893gb7";

impl ServerApi for RobtopApi {
    async fn search_levels(
        &self,
        query: String,
    ) -> Result<(Vec<LevelInfo>, HashMap<u64, SongInfo>), anyhow::Error> {
        let request =
            ureq::post(&(self.server.clone() + "getGJLevels21.php")).set("User-Agent", "");
        let body = request
            .send_form(&[("secret", COMMON_SECRET), ("str", &query), ("type", "0")])?
            .into_string()?;

        let split: Vec<&str> = de::from_str(&body, '#')?;

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
        let request =
            ureq::post(&(self.server.clone() + "downloadGJLevel22.php")).set("User-Agent", "");

        let mut buffer = Vec::new();
        request
            .send_form(&[("secret", COMMON_SECRET), ("levelID", &id.to_string())])?
            .into_reader()
            .read_to_end(&mut buffer)?;

        let body = simdutf8::basic::from_utf8(&buffer)?;

        Ok(de::from_str(body, ':')?)
    }

    async fn get_song(&self, song_info: SongInfo) -> Result<AudioSource, anyhow::Error> {
        let mut body = ureq::get(&song_info.url).call()?.into_reader();
        let mut raw_audio = Vec::new();
        body.read_to_end(&mut raw_audio)?;
        let sound_data =
            StaticSoundData::from_cursor(Cursor::new(raw_audio), StaticSoundSettings::new())?;
        Ok(AudioSource { sound: sound_data })
    }
}
