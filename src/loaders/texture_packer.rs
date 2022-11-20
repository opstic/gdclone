use bevy::asset::{AssetLoader, BoxedFuture, Handle, LoadContext, LoadedAsset};
use bevy::math::Rect;
use bevy::prelude::{FromWorld, Image, TextureAtlas, Vec2, World};
use bevy::reflect::TypeUuid;
use bevy::render::renderer::RenderDevice;
use bevy::render::texture::{CompressedImageFormats, ImageType};
use bevy::utils::HashMap;
use plist::Dictionary;
use std::path::Path;
use serde::Deserialize;

#[derive(Debug, TypeUuid)]
#[uuid = "f2c8ed94-b8c8-4d9e-99e9-7ba9b7e8603b"]
pub struct TexturePackerAtlas {
    pub(crate) index: HashMap<String, (usize, Vec2, bool)>,
    pub(crate) texture_atlas: Handle<TextureAtlas>,
}

#[derive(Deserialize)]
struct AtlasFile {
    frames: HashMap<String, Frame>,
    metadata: Metadata,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    real_texture_file_name: String,
    size: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Frame {
    aliases: Vec<String>,
    sprite_offset: String,
    sprite_size: String,
    sprite_source_size: String,
    texture_rect: String,
    texture_rotated: bool,
}

pub struct TexturePackerAtlasLoader {
    supported_compressed_formats: CompressedImageFormats,
}

impl FromWorld for TexturePackerAtlasLoader {
    fn from_world(world: &mut World) -> Self {
        let supported_compressed_formats = match world.get_resource::<RenderDevice>() {
            Some(render_device) => CompressedImageFormats::from_features(render_device.features()),

            None => CompressedImageFormats::all(),
        };
        Self {
            supported_compressed_formats,
        }
    }
}

impl AssetLoader for TexturePackerAtlasLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            let manifest: AtlasFile = plist::from_bytes(bytes).expect("Invalid manifest");
            let texture = load_texture(
                load_context,
                &manifest.metadata.real_texture_file_name,
                self.supported_compressed_formats,
            )
            .await?;
            let texture_handle =
                load_context.set_labeled_asset("texture", LoadedAsset::new(texture));
            let mut texture_atlas = TextureAtlas::new_empty(texture_handle, texture_packer_size_to_vec2(&manifest.metadata.size));
            let mut index = HashMap::new();
            for (frame_name, frame) in manifest.frames {
                let sprite_source_size = texture_packer_size_to_vec2(
                    &frame.sprite_source_size
                );
                let sprite_size = texture_packer_size_to_vec2(
                    &frame.sprite_size
                );
                let offset = texture_packer_size_to_vec2(
                    &frame.sprite_offset
                );
                let texture_index = texture_atlas.add_texture(
                    texture_packer_rect_to_bevy_rect(&frame.texture_rect, frame.texture_rotated)
                );

                index.insert(
                    frame_name.clone(),
                    (
                        texture_index,
                        Vec2::new(offset.y / sprite_size.y * if frame.texture_rotated {-1.} else {1.}, offset.x / sprite_size.x),
                        frame.texture_rotated,
                    ),
                );
            }
            let texture_atlas_handle =
                load_context.set_labeled_asset("texture_atlas", LoadedAsset::new(texture_atlas));
            load_context.set_default_asset(LoadedAsset::new(TexturePackerAtlas {
                index,
                texture_atlas: texture_atlas_handle,
            }));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["plist"]
    }
}

fn texture_packer_size_to_vec2(size_string: &str) -> Vec2 {
    let stripped_str = strip_texture_packer(size_string);
    let split_str: Vec<f32> = stripped_str
        .split(',')
        .map(|str| str.parse().unwrap())
        .collect();
    Vec2 {
        x: split_str[0],
        y: split_str[1],
    }
}

fn texture_packer_rect_to_bevy_rect(rect_string: &str, is_rotated: bool) -> Rect {
    let stripped_str = strip_texture_packer(rect_string);
    let dimensions: Vec<f32> = stripped_str
        .split(',')
        .map(|s| strip_texture_packer(s).parse().unwrap())
        .collect();
    if is_rotated {
        Rect {
            min: Vec2::new(dimensions[0], dimensions[1]),
            max: Vec2::new(dimensions[0] + dimensions[3], dimensions[1] + dimensions[2]),
        }
    } else {
        Rect {
            min: Vec2::new(dimensions[0], dimensions[1]),
            max: Vec2::new(dimensions[0] + dimensions[2], dimensions[1] + dimensions[3]),
        }
    }
}

fn strip_texture_packer(string: &str) -> &str {
    string.trim_matches(|c| c == '{' || c == '}')
}

async fn load_texture<'a>(
    load_context: &LoadContext<'a>,
    filename: &str,
    supported_compressed_formats: CompressedImageFormats,
) -> Result<Image, bevy::asset::Error> {
    let parent_dir = load_context.path().parent().unwrap();
    let image_path = parent_dir.join(filename);
    let bytes = load_context.read_asset_bytes(image_path.clone()).await?;
    let extension = Path::new(filename).extension().unwrap().to_str().unwrap();
    let image_type = ImageType::Extension(extension);
    Ok(Image::from_buffer(&bytes, image_type, supported_compressed_formats, true).unwrap())
}
