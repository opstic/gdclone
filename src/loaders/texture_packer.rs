use bevy::asset::{AssetLoader, BoxedFuture, Handle, LoadContext, LoadedAsset};
use bevy::prelude::{FromWorld, Image, TextureAtlas, Vec2, World};
use bevy::reflect::TypeUuid;
use bevy::render::renderer::RenderDevice;
use bevy::render::texture::{CompressedImageFormats, ImageType};
use bevy::sprite::Rect;
use bevy::utils::HashMap;
use plist::Dictionary;
use std::path::Path;

#[derive(Debug, TypeUuid)]
#[uuid = "f2c8ed94-b8c8-4d9e-99e9-7ba9b7e8603b"]
pub struct TexturePackerAtlas {
    pub(crate) index: HashMap<String, (usize, bool)>,
    pub(crate) texture_atlas: Handle<TextureAtlas>,
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
            let manifest: Dictionary = plist::from_bytes(bytes).expect("Invalid manifest");
            let metadata = manifest.get("metadata").unwrap().as_dictionary().unwrap();
            let texture_filename = metadata
                .get("realTextureFileName")
                .unwrap()
                .as_string()
                .unwrap();
            let texture_dimensions =
                texture_packer_size_to_vec2(metadata.get("size").unwrap().as_string().unwrap());
            let texture = load_texture(
                load_context,
                texture_filename,
                self.supported_compressed_formats,
            )
            .await?;
            let texture_handle =
                load_context.set_labeled_asset("texture", LoadedAsset::new(texture));
            let mut texture_atlas = TextureAtlas::new_empty(texture_handle, texture_dimensions);
            let mut index = HashMap::new();
            for (frame_name, frame) in manifest.get("frames").unwrap().as_dictionary().unwrap() {
                let texture_index = texture_atlas.add_texture(texture_packer_rect_to_bevy_rect(
                    frame
                        .as_dictionary()
                        .unwrap()
                        .get("textureRect")
                        .unwrap()
                        .as_string()
                        .unwrap(),
                ));
                let rotated = frame
                    .as_dictionary()
                    .unwrap()
                    .get("textureRotated")
                    .unwrap()
                    .as_boolean()
                    .unwrap();
                index.insert(frame_name.clone(), (texture_index, rotated));
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

fn texture_packer_rect_to_bevy_rect(rect_string: &str) -> Rect {
    let stripped_str = strip_texture_packer(rect_string);
    let dimensions: Vec<f32> = stripped_str
        .split(",")
        .map(|s| strip_texture_packer(s).parse().unwrap())
        .collect();
    Rect {
        min: Vec2::new(dimensions[0], dimensions[1]),
        max: Vec2::new(dimensions[0] + dimensions[2], dimensions[1] + dimensions[3]),
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
