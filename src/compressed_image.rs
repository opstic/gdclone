use bevy::app::{App, Plugin};
use bevy::asset::AddAsset;
use bevy::ecs::system::lifetimeless::SRes;
use bevy::ecs::system::SystemParamItem;
use bevy::math::Vec2;
use bevy::prelude::{Image, Reflect};
use bevy::reflect::TypeUuid;
use bevy::render::{
    render_asset::{PrepareAssetError, PrepareAssetSet, RenderAsset, RenderAssetPlugin},
    render_resource::TextureViewDescriptor,
    renderer::{RenderDevice, RenderQueue},
    texture::{CompressedImageFormats, DefaultImageSampler, GpuImage, ImageSampler, ImageType},
};
use image::codecs::png::PngEncoder;
use image::{ColorType, ImageEncoder};

pub(crate) struct CompressedImagePlugin;

impl Plugin for CompressedImagePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            RenderAssetPlugin::<CompressedImage>::with_prepare_asset_set(
                PrepareAssetSet::PreAssetPrepare,
            ),
        )
        .register_type::<CompressedImage>()
        .add_asset::<CompressedImage>()
        .register_asset_reflect::<CompressedImage>();
    }
}

#[derive(Reflect, Debug, Clone, TypeUuid)]
#[uuid = "d5415aa7-8a56-41d7-ae1f-57999372a7ba"]
#[reflect_value]
pub(crate) struct CompressedImage {
    data: Vec<u8>,
}

impl CompressedImage {
    pub(crate) fn from_image(image: Image) -> Result<CompressedImage, anyhow::Error> {
        let mut compressed_data = std::io::Cursor::new(Vec::new());
        let png_encoder = PngEncoder::new(&mut compressed_data);
        png_encoder.write_image(
            &image.data,
            image.texture_descriptor.size.height,
            image.texture_descriptor.size.width,
            ColorType::Rgba8,
        )?;
        Ok(CompressedImage {
            data: compressed_data.into_inner(),
        })
    }
}

impl RenderAsset for CompressedImage {
    type ExtractedAsset = CompressedImage;
    type PreparedAsset = GpuImage;
    type Param = (
        SRes<RenderDevice>,
        SRes<RenderQueue>,
        SRes<DefaultImageSampler>,
    );

    /// Clones the Image.
    fn extract_asset(&self) -> Self::ExtractedAsset {
        self.clone()
    }

    /// Converts the extracted image into a [`GpuImage`].
    fn prepare_asset(
        image: Self::ExtractedAsset,
        (render_device, render_queue, default_sampler): &mut SystemParamItem<Self::Param>,
    ) -> Result<Self::PreparedAsset, PrepareAssetError<Self::ExtractedAsset>> {
        let supported_compressed_formats =
            CompressedImageFormats::from_features(render_device.features());

        let image = Image::from_buffer(
            &image.data,
            ImageType::Extension("png"),
            supported_compressed_formats,
            true,
        )
        .unwrap();

        let texture = render_device.create_texture_with_data(
            render_queue,
            &image.texture_descriptor,
            &image.data,
        );

        let texture_view = texture.create_view(
            image
                .texture_view_descriptor
                .or_else(|| Some(TextureViewDescriptor::default()))
                .as_ref()
                .unwrap(),
        );
        let size = Vec2::new(
            image.texture_descriptor.size.width as f32,
            image.texture_descriptor.size.height as f32,
        );
        let sampler = match image.sampler_descriptor {
            ImageSampler::Default => (***default_sampler).clone(),
            ImageSampler::Descriptor(descriptor) => render_device.create_sampler(&descriptor),
        };

        Ok(GpuImage {
            texture,
            texture_view,
            texture_format: image.texture_descriptor.format,
            sampler,
            size,
            mip_level_count: image.texture_descriptor.mip_level_count,
        })
    }
}
