use bevy::utils::HashMap;
use bevy_kira_audio::AudioSource;
#[cfg(target_arch = "wasm32")]
use gloo_net::http::Request;

use crate::api::proxy::ProxyApi;
use crate::level::{LevelData, LevelInfo, SongInfo};

mod proxy;
pub(crate) mod robtop;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type DefaultApi = RobtopApi;
#[cfg(target_arch = "wasm32")]
pub(crate) type DefaultApi = ProxyApi;

pub(crate) trait ServerApi {
    async fn search_levels(
        &self,
        query: String,
    ) -> Result<(Vec<LevelInfo>, HashMap<u64, SongInfo>), anyhow::Error>;
    async fn download_level(&self, id: u64) -> Result<LevelData, anyhow::Error>;
    async fn get_song(&self, id: u64) -> Result<SongInfo, anyhow::Error>;
    async fn download_song(&self, song_info: SongInfo) -> Result<AudioSource, anyhow::Error>;
}

async fn get(url: &str) -> Result<Vec<u8>, anyhow::Error> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let request = ureq::get(&url);
        let mut buffer = Vec::new();
        request.call()?.into_reader().read_to_end(&mut buffer)?;
        Ok(buffer)
    }

    #[cfg(target_arch = "wasm32")]
    {
        let request_builder = Request::get(url);

        Ok(request_builder.build()?.send().await?.binary().await?)
    }
}

async fn get_query(url: &str, data: &[(&str, &str)]) -> Result<Vec<u8>, anyhow::Error> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let request = ureq::get(&url).query_pairs(data.to_vec());
        let mut buffer = Vec::new();
        request.call()?.into_reader().read_to_end(&mut buffer)?;
        Ok(buffer)
    }

    #[cfg(target_arch = "wasm32")]
    {
        let request_builder = Request::get(url).query(data.to_vec());

        Ok(request_builder.build()?.send().await?.binary().await?)
    }
}

async fn post_form(url: &str, data: &[(&str, &str)]) -> Result<Vec<u8>, anyhow::Error> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let request = ureq::post(url).set("User-Agent", "");
        let mut buffer = Vec::new();
        request
            .send_form(data)?
            .into_reader()
            .read_to_end(&mut buffer)?;
        Ok(buffer)
    }

    #[cfg(target_arch = "wasm32")]
    {
        let encoded = serde_urlencoded::to_string(data)?;

        let request_builder = Request::post(url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("User-Agent", "");

        let request = request_builder.body(encoded)?;

        Ok(request.send().await?.binary().await?)
    }
}
