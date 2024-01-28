use bevy::asset::Asset;
use bevy::ecs::{system::lifetimeless::SRes, system::SystemParamItem};
use bevy::math::Vec2;
use bevy::prelude::Image;
use bevy::reflect::Reflect;
use bevy::render::{
    render_asset::{PrepareAssetError, RenderAsset},
    render_resource::{TextureDescriptor, TextureViewDescriptor},
    renderer::{RenderDevice, RenderQueue},
    texture::{DefaultImageSampler, GpuImage, ImageSampler},
};
use zstd::bulk::{Compressor, Decompressor};

/// A [`CompressedImage`] asset to store only the compressed version of the image in memory
///
/// Proven very effective at saving memory space, especially since Bevy currently doesn't
/// allow unloading assets after sending them to the GPU :/
///
/// (https://github.com/bevyengine/bevy/pull/10520)
#[derive(Asset, Reflect, Debug, Clone)]
#[reflect_value]
pub(crate) struct CompressedImage {
    pub data: Vec<u8>,
    pub uncompressed_size: usize,
    pub texture_descriptor: TextureDescriptor<'static>,
    pub sampler: ImageSampler,
    pub texture_view_descriptor: Option<TextureViewDescriptor<'static>>,
}

impl From<Image> for CompressedImage {
    fn from(image: Image) -> Self {
        let mut zstd_compressor = Compressor::new(0).expect("Failed to create zstd compressor");
        let zstd_compressed_image = zstd_compressor
            .compress(&image.data)
            .expect("Failed to compress image data");

        CompressedImage {
            data: zstd_compressed_image,
            uncompressed_size: image.data.len(),
            texture_descriptor: image.texture_descriptor,
            sampler: image.sampler,
            texture_view_descriptor: image.texture_view_descriptor,
        }
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

    /// Clones the [`CompressedImage`].
    fn extract_asset(&self) -> Self::ExtractedAsset {
        self.clone()
    }

    /// Converts the extracted [`Image`] into a [`GpuImage`].
    fn prepare_asset(
        image: Self::ExtractedAsset,
        (render_device, render_queue, default_sampler): &mut SystemParamItem<Self::Param>,
    ) -> Result<Self::PreparedAsset, PrepareAssetError<Self::ExtractedAsset>> {
        let mut zstd_decompressor =
            Decompressor::new().expect("Failed to create zstd decompressor");
        let decompressed_image = zstd_decompressor
            .decompress(&image.data, image.uncompressed_size)
            .expect("Failed to decompress image data");

        let texture = render_device.create_texture_with_data(
            render_queue,
            &image.texture_descriptor,
            &decompressed_image,
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
        let sampler = match image.sampler {
            ImageSampler::Default => (***default_sampler).clone(),
            ImageSampler::Descriptor(descriptor) => {
                render_device.create_sampler(&descriptor.as_wgpu())
            }
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
