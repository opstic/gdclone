use std::path::Path;

use bevy::asset::{io::Reader, Asset, AssetLoader, AsyncReadExt, BoxedFuture, Handle, LoadContext};
use bevy::log::info;
use bevy::math::{Rect, Vec2};
use bevy::prelude::{FromWorld, Image, World};
use bevy::reflect::TypePath;
use bevy::render::color::SrgbColorSpace;
use bevy::render::{
    renderer::RenderDevice,
    texture::{CompressedImageFormats, ImageSampler, ImageType},
};
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy::utils::{HashMap, Instant};
use serde::{Deserialize, Deserializer};

use crate::asset::compressed_image::CompressedImage;

#[derive(Asset, TypePath, Debug)]
pub struct Cocos2dAtlas {
    texture: Handle<CompressedImage>,
    frames: HashMap<String, Cocos2dFrame>,
}

#[derive(Clone, Debug)]
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
                            anchor: anchor * 2.,
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

            let texture_future: Task<Result<CompressedImage, anyhow::Error>> =
                async_compute.spawn(async move {
                    let mut thread_chunk_size = texture.data.len() / async_compute.thread_num();
                    if thread_chunk_size % 4 != 0 {
                        thread_chunk_size += 4 - thread_chunk_size % 4;
                    }

                    async_compute.scope(|scope| {
                        for chunk in texture.data.chunks_mut(thread_chunk_size) {
                            scope.spawn(async move {
                                for pixel in chunk.chunks_exact_mut(4) {
                                    let mut linear_r = (pixel[0] as f32 / u8::MAX as f32)
                                        .nonlinear_to_linear_srgb();
                                    let mut linear_g = (pixel[1] as f32 / u8::MAX as f32)
                                        .nonlinear_to_linear_srgb();
                                    let mut linear_b = (pixel[2] as f32 / u8::MAX as f32)
                                        .nonlinear_to_linear_srgb();

                                    let alpha = pixel[3] as f32 / u8::MAX as f32;

                                    linear_r *= alpha;
                                    linear_g *= alpha;
                                    linear_b *= alpha;

                                    pixel[0] = (linear_r * u8::MAX as f32) as u8;
                                    pixel[1] = (linear_g * u8::MAX as f32) as u8;
                                    pixel[2] = (linear_b * u8::MAX as f32) as u8;
                                }
                            });
                        }
                    });

                    Ok(CompressedImage::from(texture))
                });

            let texture_handle =
                load_context.add_labeled_asset("texture".to_string(), texture_future.await?);

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
    let parent_dir = load_context.path().parent().unwrap();
    let image_path = parent_dir.join(filename);
    let bytes = load_context.read_asset_bytes(image_path.clone()).await?;
    let extension = Path::new(filename).extension().unwrap().to_str().unwrap();
    let image_type = ImageType::Extension(extension);
    Ok(Image::from_buffer(
        &bytes,
        image_type,
        supported_compressed_formats,
        true,
        ImageSampler::Default,
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
