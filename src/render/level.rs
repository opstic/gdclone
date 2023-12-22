use bevy::app::{App, Plugin};
use bevy::asset::{AssetId, Assets, Handle, load_internal_asset};
use bevy::core_pipeline::core_2d::Transparent2d;
use bevy::ecs::system::SystemState;
use bevy::math::{IVec2, Rect, Vec2};
use bevy::prelude::{
    Color, Entity, FromWorld, GlobalTransform, Image, Query, QueryState, Res, ResMut, Resource,
    Shader, Transform, World,
};
use bevy::render::{
    Extract,
    ExtractSchedule,
    mesh::PrimitiveTopology,
    render_phase::AddRenderCommand,
    render_resource::{
        BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendState,
        BufferBindingType, ColorTargetState, ColorWrites, FragmentState, FrontFace,
        ImageCopyTexture, ImageDataLayout, MultisampleState, Origin3d, PolygonMode, PrimitiveState,
        RenderPipelineDescriptor, SamplerBindingType, ShaderStages, ShaderType,
        SpecializedRenderPipeline, SpecializedRenderPipelines, TextureAspect, TextureFormat,
        TextureSampleType, TextureViewDescriptor, TextureViewDimension, VertexBufferLayout,
        VertexFormat, VertexState, VertexStepMode,
    },
    RenderApp,
    renderer::{RenderDevice, RenderQueue}, texture::{BevyDefault, DefaultImageSampler, GpuImage, ImageSampler, TextureFormatPixelInfo}, view::ViewUniform,
};
use dashmap::DashMap;

use crate::asset::{cocos2d_atlas::Cocos2dAtlas, compressed_image::CompressedImage};
use crate::level::{
    LevelWorld,
    section::{GlobalSections, VisibleGlobalSections},
};
use crate::utils::U64Hash;

#[derive(Default)]
pub(crate) struct LevelRenderPlugin;

pub const LEVEL_SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(58263810593726394857);

impl Plugin for LevelRenderPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, LEVEL_SHADER_HANDLE, "level.wgsl", Shader::from_wgsl);

        if let Ok(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<SpecializedRenderPipelines<LevelPipeline>>()
                .init_resource::<SpriteMeta>()
                .init_resource::<ExtractedLevelSections>()
                .init_resource::<SpriteAssetEvents>()
                .add_render_command::<Transparent2d, DrawSprite>()
                .add_systems(ExtractSchedule, extract_level)
                .add_systems(
                    Render,
                    (
                        queue_sprites
                            .in_set(RenderSet::Queue)
                            .ambiguous_with(queue_material2d_meshes::<ColorMaterial>),
                        prepare_sprites.in_set(RenderSet::PrepareBindGroups),
                    ),
                );
        };
    }
}

#[derive(Resource)]
pub struct LevelPipeline {
    view_layout: BindGroupLayout,
    material_layout: BindGroupLayout,
    pub dummy_white_gpu_image: GpuImage,
}

impl FromWorld for LevelPipeline {
    fn from_world(world: &mut World) -> Self {
        let mut system_state: SystemState<(
            Res<RenderDevice>,
            Res<DefaultImageSampler>,
            Res<RenderQueue>,
        )> = SystemState::new(world);
        let (render_device, default_sampler, render_queue) = system_state.get_mut(world);

        let view_layout = render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: Some(ViewUniform::min_size()),
                },
                count: None,
            }],
            label: Some("level_view_layout"),
        });

        let material_layout = render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        multisampled: false,
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("level_material_layout"),
        });
        let dummy_white_gpu_image = {
            let image = Image::default();
            let texture = render_device.create_texture(&image.texture_descriptor);
            let sampler = match image.sampler {
                ImageSampler::Default => (**default_sampler).clone(),
                ImageSampler::Descriptor(ref descriptor) => {
                    render_device.create_sampler(&descriptor.as_wgpu())
                }
            };

            let format_size = image.texture_descriptor.format.pixel_size();
            render_queue.write_texture(
                ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                &image.data,
                ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(image.width() * format_size as u32),
                    rows_per_image: None,
                },
                image.texture_descriptor.size,
            );
            let texture_view = texture.create_view(&TextureViewDescriptor::default());
            GpuImage {
                texture,
                texture_view,
                texture_format: image.texture_descriptor.format,
                sampler,
                size: image.size_f32(),
                mip_level_count: image.texture_descriptor.mip_level_count,
            }
        };

        LevelPipeline {
            view_layout,
            material_layout,
            dummy_white_gpu_image,
        }
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    #[repr(transparent)]
    // NOTE: Apparently quadro drivers support up to 64x MSAA.
    // MSAA uses the highest 3 bits for the MSAA log2(sample count) to support up to 128x MSAA.
    pub struct SpritePipelineKey: u32 {
        const NONE                              = 0;
        const NO_TEXTURE_ARRAY                  = (1 << 0);
        const MSAA_RESERVED_BITS                = Self::MSAA_MASK_BITS << Self::MSAA_SHIFT_BITS;
    }
}

impl SpritePipelineKey {
    const MSAA_MASK_BITS: u32 = 0b111;
    const MSAA_SHIFT_BITS: u32 = 32 - Self::MSAA_MASK_BITS.count_ones();

    #[inline]
    pub const fn from_msaa_samples(msaa_samples: u32) -> Self {
        let msaa_bits =
            (msaa_samples.trailing_zeros() & Self::MSAA_MASK_BITS) << Self::MSAA_SHIFT_BITS;
        Self::from_bits_retain(msaa_bits)
    }

    #[inline]
    pub const fn msaa_samples(&self) -> u32 {
        1 << ((self.bits() >> Self::MSAA_SHIFT_BITS) & Self::MSAA_MASK_BITS)
    }
}

impl SpecializedRenderPipeline for LevelPipeline {
    type Key = SpritePipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let mut formats = vec![
            // @location(0) i_model_transpose_col0: vec4<f32>,
            VertexFormat::Float32x4,
            // @location(1) i_model_transpose_col1: vec4<f32>,
            VertexFormat::Float32x4,
            // @location(2) i_model_transpose_col2: vec4<f32>,
            VertexFormat::Float32x4,
            // @location(3) i_color: vec4<f32>,
            VertexFormat::Float32x4,
            // @location(4) i_uv_offset_scale: vec4<f32>,
            VertexFormat::Float32x4,
        ];

        let mut shader_defs = Vec::new();

        if key.contains(SpritePipelineKey::NO_TEXTURE_ARRAY) {
            shader_defs.push("NO_TEXTURE_ARRAY".into());
        } else {
            // @location(5) texture_index: u32
            formats.push(VertexFormat::Uint32);
        }

        let vertex_layout =
            VertexBufferLayout::from_vertex_formats(VertexStepMode::Instance, formats);

        RenderPipelineDescriptor {
            vertex: VertexState {
                shader: LEVEL_SHADER_HANDLE,
                entry_point: "vertex".into(),
                shader_defs: shader_defs.clone(),
                buffers: vec![vertex_layout],
            },
            fragment: Some(FragmentState {
                shader: LEVEL_SHADER_HANDLE,
                shader_defs,
                entry_point: "fragment".into(),
                targets: vec![Some(ColorTargetState {
                    format: TextureFormat::bevy_default(),
                    blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            layout: vec![self.view_layout.clone(), self.material_layout.clone()],
            primitive: PrimitiveState {
                front_face: FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: key.msaa_samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            label: Some("level_pipeline".into()),
            push_constant_ranges: Vec::new(),
        }
    }
}

pub struct ExtractedObject {
    pub transform: GlobalTransform,
    pub color: Color,
    /// Select an area of the texture
    pub rect: Option<Rect>,
    /// Change the on-screen size of the sprite
    pub custom_size: Option<Vec2>,
    /// Asset ID of the [`Image`] of this sprite
    /// PERF: storing an `AssetId` instead of `Handle<Image>` enables some optimizations (`ExtractedSprite` becomes `Copy` and doesn't need to be dropped)
    pub image_handle_id: AssetId<CompressedImage>,
    pub flip_x: bool,
    pub flip_y: bool,
    pub anchor: Vec2,
}

#[derive(Resource)]
struct ExtractedSections {
    sections: DashMap<IVec2, DashMap<Entity, ExtractedObject, U64Hash>>,
}

pub fn extract_objects(
    mut extracted_sections: ResMut<ExtractedSections>,
    texture_atlases: Extract<Res<Assets<Cocos2dAtlas>>>,
    level_world: Extract<Res<LevelWorld>>,
) {
    let LevelWorld::World(world) = &**level_world else {
        // There's nothing to render
        return;
    };

    // Should be fine since the queries are read-only, enforced with `as_readonly()`
    //
    // And the only reason making a query needs a mutable reference to the world
    // is because it will initialize components that are not there, which shouldn't happen
    // since everything should be initialized by the time `extract_objects` is called
    //
    // https://github.com/bevyengine/bevy/issues/3774
    let world_mut = unsafe {
        let const_ptr = world as *const World;
        let mut_ptr = const_ptr as *mut World;
        &mut *mut_ptr
    };

    let mut objects = world_mut.query::<&GlobalTransform>().as_readonly();
}
