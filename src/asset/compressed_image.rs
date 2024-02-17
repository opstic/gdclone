use bevy::asset::Asset;
use bevy::ecs::{system::lifetimeless::SRes, system::SystemParamItem};
use bevy::math::Vec2;
use bevy::prelude::Image;
use bevy::reflect::Reflect;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::TextureDataOrder;
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
    pub asset_usage: RenderAssetUsages,
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
            asset_usage: image.asset_usage,
        }
    }
}

impl RenderAsset for CompressedImage {
    type PreparedAsset = GpuImage;
    type Param = (
        SRes<RenderDevice>,
        SRes<RenderQueue>,
        SRes<DefaultImageSampler>,
    );

    fn asset_usage(&self) -> RenderAssetUsages {
        self.asset_usage
    }

    /// Converts the extracted [`Image`] into a [`GpuImage`].
    fn prepare_asset(
        self,
        (render_device, render_queue, default_sampler): &mut SystemParamItem<Self::Param>,
    ) -> Result<Self::PreparedAsset, PrepareAssetError<Self>> {
        let mut zstd_decompressor =
            Decompressor::new().expect("Failed to create zstd decompressor");
        let decompressed_image = zstd_decompressor
            .decompress(&self.data, self.uncompressed_size)
            .expect("Failed to decompress image data");

        let texture = render_device.create_texture_with_data(
            render_queue,
            &self.texture_descriptor,
            TextureDataOrder::default(),
            &decompressed_image,
        );

        let texture_view = texture.create_view(
            self.texture_view_descriptor
                .or_else(|| Some(TextureViewDescriptor::default()))
                .as_ref()
                .unwrap(),
        );
        let size = Vec2::new(
            self.texture_descriptor.size.width as f32,
            self.texture_descriptor.size.height as f32,
        );
        let sampler = match self.sampler {
            ImageSampler::Default => (***default_sampler).clone(),
            ImageSampler::Descriptor(descriptor) => {
                render_device.create_sampler(&descriptor.as_wgpu())
            }
        };

        Ok(GpuImage {
            texture,
            texture_view,
            texture_format: self.texture_descriptor.format,
            sampler,
            size,
            mip_level_count: self.texture_descriptor.mip_level_count,
        })
    }
}
