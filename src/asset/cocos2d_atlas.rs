use std::path::Path;

use bevy::asset::{
    io::Reader, Asset, AssetEvent, AssetId, AssetLoader, AssetPath, Assets, AsyncReadExt,
    BoxedFuture, Handle, LoadContext,
};
use bevy::log::info;
use bevy::math::{Rect, Vec2};
use bevy::prelude::{EventReader, FromWorld, Image, ResMut, Resource, World};
use bevy::reflect::TypePath;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::texture::ImageSamplerDescriptor;
use bevy::render::{
    renderer::RenderDevice,
    texture::{CompressedImageFormats, ImageSampler, ImageType},
};
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy::utils::{HashMap, Instant};
use serde::{Deserialize, Deserializer};

use crate::utils::fast_scale;

#[derive(Asset, TypePath, Debug)]
pub struct Cocos2dAtlas {
    texture: Handle<Image>,
    squared: Handle<Image>,
    frames: HashMap<String, Cocos2dFrame>,
}

#[derive(Clone, Resource, Default)]
pub(crate) struct Cocos2dFrames {
    pub(crate) index: HashMap<String, usize>,
    pub(crate) frames: Vec<(Cocos2dFrame, AssetId<Image>, AssetId<Image>)>,
}

pub(crate) fn move_frames_to_resource(
    mut frames: ResMut<Cocos2dFrames>,
    mut atlas_events: EventReader<AssetEvent<Cocos2dAtlas>>,
    mut atlases: ResMut<Assets<Cocos2dAtlas>>,
) {
    for atlas_event in atlas_events.read() {
        match atlas_event {
            AssetEvent::Added { id } => {
                if let Some(atlas) = atlases.get_mut(*id) {
                    for (texture_name, frame_info) in std::mem::take(&mut atlas.frames) {
                        let frame_index = frames.frames.len();
                        frames.index.insert(texture_name, frame_index);
                        frames
                            .frames
                            .push((frame_info, atlas.texture.id(), atlas.squared.id()));
                    }
                }
            }
            AssetEvent::Removed { .. } => {
                // TODO: Remove the frames
            }
            _ => (),
        }
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct Cocos2dFrame {
    pub(crate) rect: Rect,
    pub(crate) anchor: Vec2,
    pub(crate) rotated: bool,
}

#[derive(Deserialize)]
struct AtlasFile {
    frames: HashMap<String, Frame>,
    metadata: Metadata,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    // format: u8,
    real_texture_file_name: String,
    // #[serde(deserialize_with = "to_vec2")]
    // size: Vec2,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Frame {
    // aliases: Vec<String>,
    #[serde(deserialize_with = "to_vec2")]
    sprite_offset: Vec2,
    #[serde(deserialize_with = "to_vec2")]
    sprite_size: Vec2,
    // #[serde(deserialize_with = "to_vec2")]
    // sprite_source_size: Vec2,
    #[serde(deserialize_with = "to_rect")]
    texture_rect: Rect,
    texture_rotated: bool,
}

pub struct Cocos2dAtlasLoader {
    supported_compressed_formats: CompressedImageFormats,
}

impl FromWorld for Cocos2dAtlasLoader {
    fn from_world(world: &mut World) -> Self {
        let supported_compressed_formats = match world.get_resource::<RenderDevice>() {
            Some(render_device) => CompressedImageFormats::from_features(render_device.features()),
            None => CompressedImageFormats::NONE,
        };
        Self {
            supported_compressed_formats,
        }
    }
}

impl AssetLoader for Cocos2dAtlasLoader {
    type Asset = Cocos2dAtlas;
    type Settings = ();
    type Error = anyhow::Error;
    fn load<'a>(
        &'a self,
        reader: &'a mut Reader,
        _settings: &'a (),
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let timer = Instant::now();

            let mut manifest_bytes = Vec::new();
            reader.read_to_end(&mut manifest_bytes).await?;
            let manifest: AtlasFile = plist::from_bytes(&manifest_bytes)?;

            let async_compute = AsyncComputeTaskPool::get();

            let frames_future = async_compute.spawn(async move {
                let mut frames = HashMap::with_capacity(manifest.frames.len());
                for (frame_name, frame) in manifest.frames {
                    let frame_rect = if frame.texture_rotated {
                        Rect {
                            min: frame.texture_rect.min,
                            max: Vec2 {
                                x: frame.texture_rect.min.x + frame.sprite_size.y,
                                y: frame.texture_rect.min.y + frame.sprite_size.x,
                            },
                        }
                    } else {
                        frame.texture_rect
                    };

                    let mut anchor = -(frame.sprite_offset / frame.sprite_size);
                    if frame.texture_rotated {
                        anchor = Vec2 {
                            x: anchor.y,
                            y: -anchor.x,
                        };
                    }

                    frames.insert(
                        frame_name,
                        Cocos2dFrame {
                            rect: frame_rect,
                            anchor,
                            rotated: frame.texture_rotated,
                        },
                    );
                }
                frames
            });

            let mut texture = load_texture(
                load_context,
                &manifest.metadata.real_texture_file_name,
                self.supported_compressed_formats,
            )
            .await?;

            let texture_future: Task<Result<(Image, Image), anyhow::Error>> =
                async_compute.spawn(async move {
                    let mut thread_chunk_size = texture.data.len() / async_compute.thread_num();
                    if thread_chunk_size % 4 != 0 {
                        thread_chunk_size += 4 - thread_chunk_size % 4;
                    }

                    async_compute.scope(|scope| {
                        for chunk in texture.data.chunks_mut(thread_chunk_size) {
                            scope.spawn(async move {
                                for pixel in chunk.chunks_exact_mut(4) {
                                    pixel[0] = fast_scale(pixel[0], pixel[3]);
                                    pixel[1] = fast_scale(pixel[1], pixel[3]);
                                    pixel[2] = fast_scale(pixel[2], pixel[3]);
                                }
                            });
                        }
                    });

                    texture.texture_descriptor.format =
                        texture.texture_descriptor.format.remove_srgb_suffix();

                    texture.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());

                    let mut squared_alpha = texture.clone();
                    async_compute.scope(|scope| {
                        for chunk in squared_alpha.data.chunks_mut(thread_chunk_size) {
                            scope.spawn(async move {
                                for pixel in chunk.chunks_exact_mut(4) {
                                    pixel[0] = fast_scale(pixel[0], pixel[3]);
                                    pixel[1] = fast_scale(pixel[1], pixel[3]);
                                    pixel[2] = fast_scale(pixel[2], pixel[3]);
                                    pixel[3] = fast_scale(pixel[3], pixel[3]);
                                }
                            });
                        }
                    });

                    Ok((texture, squared_alpha))
                });

            let (texture, squared) = texture_future.await?;

            let texture_handle = load_context.add_labeled_asset("texture".to_string(), texture);

            let squared_handle = load_context.add_labeled_asset("squared".to_string(), squared);

            info!(
                "Loaded {}, took {:?}.",
                load_context
                    .asset_path()
                    .path()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap(),
                timer.elapsed()
            );
            Ok(Cocos2dAtlas {
                texture: texture_handle,
                squared: squared_handle,
                frames: frames_future.await,
            })
        })
    }

    fn extensions(&self) -> &[&str] {
        &["plist"]
    }
}

async fn load_texture<'a>(
    load_context: &mut LoadContext<'a>,
    filename: &str,
    supported_compressed_formats: CompressedImageFormats,
) -> Result<Image, anyhow::Error> {
    let asset_path = load_context.asset_path().clone();
    let asset_parent = asset_path.path().parent().unwrap();
    let bytes = load_context
        .read_asset_bytes(
            AssetPath::from(asset_parent.join(filename)).with_source(asset_path.source()),
        )
        .await?;
    let extension = Path::new(filename).extension().unwrap().to_str().unwrap();
    let image_type = ImageType::Extension(extension);
    Ok(Image::from_buffer(
        &bytes,
        image_type,
        supported_compressed_formats,
        true,
        ImageSampler::Default,
        RenderAssetUsages::RENDER_WORLD,
    )
    .unwrap())
}

fn to_vec2<'de, D>(deserializer: D) -> Result<Vec2, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    let split_str: Vec<f32> = s
        .trim_matches(|c| c == '{' || c == '}')
        .split(',')
        .map(|str| str.parse().unwrap())
        .collect();
    Ok(Vec2 {
        x: split_str[0],
        y: split_str[1],
    })
}

fn to_rect<'de, D>(deserializer: D) -> Result<Rect, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    let dimensions: Vec<f32> = s
        .trim_matches(|c| c == '{' || c == '}')
        .split(',')
        .map(|s| s.trim_matches(|c| c == '{' || c == '}').parse().unwrap())
        .collect();
    Ok(Rect {
        min: Vec2::new(dimensions[0], dimensions[1]),
        max: Vec2::new(dimensions[0] + dimensions[2], dimensions[1] + dimensions[3]),
    })
}
