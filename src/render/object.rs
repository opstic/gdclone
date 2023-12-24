use std::hash::Hash;
use std::num::NonZeroU32;
use std::ops::Range;

use bevy::app::{App, Plugin};
use bevy::asset::{load_internal_asset, AssetId, Assets, Handle};
use bevy::core::{Pod, Zeroable};
use bevy::core_pipeline::core_2d::Transparent2d;
use bevy::ecs::system::lifetimeless::{Read, SRes};
use bevy::ecs::system::{SystemParamItem, SystemState};
use bevy::log::{info_span, warn};
use bevy::math::{Affine3A, Quat, Rect, Vec2, Vec4};
use bevy::prelude::{
    Color, Commands, Component, Entity, FromWorld, GlobalTransform, Image, IntoSystemConfigs,
    Local, Msaa, Query, Res, ResMut, Resource, Shader, World,
};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::{
    AddRenderCommand, DrawFunctions, PhaseItem, RenderCommand, RenderCommandResult, RenderPhase,
    SetItemPipeline, TrackedRenderPass,
};
use bevy::render::render_resource::{
    BindGroup, BindGroupEntries, BufferUsages, BufferVec, PipelineCache, WgpuFeatures,
};
use bevy::render::view::{ViewUniformOffset, ViewUniforms};
use bevy::render::{
    mesh::PrimitiveTopology,
    render_resource::{
        BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendState,
        BufferBindingType, ColorTargetState, ColorWrites, FragmentState, FrontFace,
        ImageCopyTexture, ImageDataLayout, MultisampleState, Origin3d, PolygonMode, PrimitiveState,
        RenderPipelineDescriptor, SamplerBindingType, ShaderStages, ShaderType,
        SpecializedRenderPipeline, SpecializedRenderPipelines, TextureAspect, TextureFormat,
        TextureSampleType, TextureViewDescriptor, TextureViewDimension, VertexBufferLayout,
        VertexFormat, VertexState, VertexStepMode,
    },
    renderer::{RenderDevice, RenderQueue},
    texture::{BevyDefault, DefaultImageSampler, GpuImage, ImageSampler, TextureFormatPixelInfo},
    view::ViewUniform,
    Extract, ExtractSchedule, Render, RenderApp, RenderSet,
};
use bevy::tasks::ComputeTaskPool;
use bevy::utils::syncunsafecell::SyncUnsafeCell;
use bevy::utils::{FloatOrd, HashMap};

use crate::asset::{cocos2d_atlas::Cocos2dAtlas, compressed_image::CompressedImage};
use crate::level::section::SectionIndex;
use crate::level::{
    object::Object,
    section::{GlobalSections, VisibleGlobalSections},
    LevelWorld,
};

#[derive(Default)]
pub(crate) struct ObjectRenderPlugin;

pub const OBJECT_SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(58263810593726394857);

impl Plugin for ObjectRenderPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, OBJECT_SHADER_HANDLE, "object.wgsl", Shader::from_wgsl);

        if let Ok(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<ImageBindGroups>()
                .init_resource::<SpecializedRenderPipelines<ObjectPipeline>>()
                .init_resource::<ObjectMeta>()
                .init_resource::<ExtractedLayers>()
                .init_resource::<ExtractSystemStateCache>()
                // .init_resource::<SpriteAssetEvents>()
                .add_render_command::<Transparent2d, DrawObject>()
                .add_systems(ExtractSchedule, extract_objects)
                .add_systems(
                    Render,
                    (
                        queue_objects.in_set(RenderSet::Queue),
                        prepare_objects.in_set(RenderSet::PrepareBindGroups),
                    ),
                );
        };
    }

    fn finish(&self, app: &mut App) {
        if let Ok(render_app) = app.get_sub_app_mut(RenderApp) {
            let mut fallbacks = Fallbacks::default();

            let render_device = render_app.world.resource::<RenderDevice>();

            if !render_device
                .features()
                .contains(WgpuFeatures::TEXTURE_BINDING_ARRAY)
            {
                warn!(
                "Current GPU does not support texture arrays, switching to fallback implementation"
            );
                fallbacks.no_texture_array = true;
            }

            fallbacks.no_texture_array = true;

            render_app
                .insert_resource(fallbacks)
                .init_resource::<ObjectPipeline>();
        };
    }
}

#[derive(Default, Resource)]
pub struct Fallbacks {
    no_texture_array: bool,
}

#[derive(Resource)]
pub struct ObjectPipeline {
    view_layout: BindGroupLayout,
    material_layout: BindGroupLayout,
    pub dummy_white_gpu_image: GpuImage,
}

impl FromWorld for ObjectPipeline {
    fn from_world(world: &mut World) -> Self {
        let mut system_state: SystemState<(
            Res<RenderDevice>,
            Res<DefaultImageSampler>,
            Res<RenderQueue>,
            Res<Fallbacks>,
        )> = SystemState::new(world);
        let (render_device, default_sampler, render_queue, fallbacks) = system_state.get_mut(world);

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
            label: Some("object_view_layout"),
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
                    count: if fallbacks.no_texture_array {
                        None
                    } else {
                        NonZeroU32::new(16)
                    },
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: if fallbacks.no_texture_array {
                        None
                    } else {
                        NonZeroU32::new(16)
                    },
                },
            ],
            label: Some("object_material_layout"),
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

        ObjectPipeline {
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
    pub struct ObjectPipelineKey: u32 {
        const NONE                              = 0;
        const NO_TEXTURE_ARRAY                  = (1 << 0);
        const MSAA_RESERVED_BITS                = Self::MSAA_MASK_BITS << Self::MSAA_SHIFT_BITS;
    }
}

impl ObjectPipelineKey {
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

impl SpecializedRenderPipeline for ObjectPipeline {
    type Key = ObjectPipelineKey;

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
            // @location(5) i_texture_index: u32
            VertexFormat::Uint32,
            // @location(5) i_padding: vec3<u32>
            VertexFormat::Uint32x3,
        ];

        let mut shader_defs = Vec::new();

        if key.contains(ObjectPipelineKey::NO_TEXTURE_ARRAY) {
            shader_defs.push("NO_TEXTURE_ARRAY".into());
        }

        let vertex_layout =
            VertexBufferLayout::from_vertex_formats(VertexStepMode::Instance, formats);

        RenderPipelineDescriptor {
            vertex: VertexState {
                shader: OBJECT_SHADER_HANDLE,
                entry_point: "vertex".into(),
                shader_defs: shader_defs.clone(),
                buffers: vec![vertex_layout],
            },
            fragment: Some(FragmentState {
                shader: OBJECT_SHADER_HANDLE,
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
            label: Some("object_pipeline".into()),
            push_constant_ranges: Vec::new(),
        }
    }
}

#[derive(Copy, Clone)]
pub struct ExtractedObject {
    transform: GlobalTransform,
    color: Color,
    /// Select an area of the texture
    rect: Option<Rect>,
    /// Change the on-screen size of the sprite
    custom_size: Option<Vec2>,
    /// Asset ID of the [`Image`] of this sprite
    /// PERF: storing an `AssetId` instead of `Handle<Image>` enables some optimizations (`ExtractedSprite` becomes `Copy` and doesn't need to be dropped)
    image_handle_id: AssetId<CompressedImage>,
    flip_x: bool,
    flip_y: bool,
    anchor: Vec2,
    z_layer: i32,
}

#[derive(Eq, PartialEq)]
struct LayerIndex(i32);

impl LayerIndex {
    fn from_u32(value: u32) -> Self {
        Self(unsafe { std::mem::transmute(value) })
    }

    fn to_u32(&self) -> u32 {
        unsafe { std::mem::transmute(self.0) }
    }
}

#[derive(Default, Resource)]
struct ExtractedLayers {
    layers: Vec<(LayerIndex, SyncUnsafeCell<Vec<ExtractedObject>>)>,
    total_size: usize,
}

#[derive(Default, Resource)]
struct ExtractSystemStateCache {
    cached_system_state: Option<
        SystemState<(
            Res<'static, GlobalSections>,
            Res<'static, VisibleGlobalSections>,
            Query<
                'static,
                'static,
                (
                    &'static GlobalTransform,
                    &'static Object,
                    // &'static Handle<CompressedImage>,
                ),
            >,
        )>,
    >,
}

pub(crate) fn extract_objects(
    mut extract_system_state_cache: ResMut<ExtractSystemStateCache>,
    mut extracted_layers: ResMut<ExtractedLayers>,
    texture_atlases: Extract<Res<Assets<Cocos2dAtlas>>>,
    level_world: Extract<Res<LevelWorld>>,
) {
    let LevelWorld::World(world) = &**level_world else {
        // There's nothing to render
        return;
    };

    let valid_system_state = {
        if let Some(system_state) = &mut extract_system_state_cache.cached_system_state {
            if system_state.matches_world(world.id()) {
                Some(system_state)
            } else {
                None
            }
        } else {
            None
        }
    };

    let system_state = match valid_system_state {
        Some(system_state) => system_state,
        None => {
            // Should be fine since the queries are read-only
            //
            // And the only reason making a query needs a mutable reference to the world
            // is because it will try to initialize components that doesn't exist, which shouldn't happen
            // since everything should be initialized by the time `extract_objects` is called
            //
            // https://github.com/bevyengine/bevy/issues/3774
            let world_mut = unsafe { world.as_unsafe_world_cell_readonly().world_mut() };

            let mut system_state: SystemState<(
                Res<GlobalSections>,
                Res<VisibleGlobalSections>,
                Query<(
                    &GlobalTransform,
                    &Object,
                    // &Handle<CompressedImage>,
                )>,
            )> = SystemState::new(world_mut);

            extract_system_state_cache.cached_system_state = Some(system_state);

            extract_system_state_cache
                .cached_system_state
                .as_mut()
                .unwrap()
        }
    };

    let (global_sections, visible_global_sections, objects) = system_state.get(world);

    let objects = &objects;

    for (_, extracted_layer) in &mut extracted_layers.layers {
        extracted_layer.get_mut().clear();
    }

    for x in visible_global_sections.x.clone() {
        for y in visible_global_sections.y.clone() {
            let section_index = SectionIndex::new(x, y);

            let Some(global_section) = global_sections.0.get(&section_index) else {
                continue;
            };

            for (transform, object) in objects.iter_many(global_section.value()) {
                let extracted_layer = if let Some((_, extracted_layer)) = extracted_layers
                    .layers
                    .iter_mut()
                    .find(|(layer_index, _)| layer_index.0 == object.z_layer)
                {
                    extracted_layer
                } else {
                    extracted_layers.layers.push((
                        LayerIndex(object.z_layer),
                        SyncUnsafeCell::new(Vec::with_capacity(5000)),
                    ));
                    let layer_index = extracted_layers.layers.len() - 1;
                    &mut extracted_layers.layers[layer_index].1
                };

                let extracted_layer = extracted_layer.get_mut();

                extracted_layer.push(ExtractedObject {
                    transform: *transform,
                    color: Color::WHITE,
                    rect: None,
                    custom_size: None,
                    image_handle_id: AssetId::default(),
                    flip_x: false,
                    flip_y: false,
                    anchor: Vec2::ZERO,
                    z_layer: object.z_layer,
                });
            }
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default, Pod, Zeroable)]
struct ObjectInstance {
    // Affine 4x3 transposed to 3x4
    i_model_transpose: [Vec4; 3],
    i_color: [f32; 4],
    i_uv_offset_scale: [f32; 4],
    i_texture_index: u32,
    _padding: [u32; 3],
}

impl ObjectInstance {
    #[inline]
    fn from(
        transform: &Affine3A,
        color: &Color,
        uv_offset_scale: &Vec4,
        texture_index: u32,
    ) -> Self {
        let transpose_model_3x3 = transform.matrix3.transpose();
        Self {
            i_model_transpose: [
                transpose_model_3x3.x_axis.extend(transform.translation.x),
                transpose_model_3x3.y_axis.extend(transform.translation.y),
                transpose_model_3x3.z_axis.extend(transform.translation.z),
            ],
            // i_color: color.as_linear_rgba_f32(),
            i_color: [1.; 4],
            i_uv_offset_scale: uv_offset_scale.to_array(),
            i_texture_index: texture_index,
            _padding: [0; 3],
        }
    }
}

#[derive(Resource)]
pub struct ObjectMeta {
    view_bind_group: Option<BindGroup>,
    instance_buffer: BufferVec<ObjectInstance>,
}

impl Default for ObjectMeta {
    fn default() -> Self {
        Self {
            view_bind_group: None,
            instance_buffer: BufferVec::<ObjectInstance>::new(BufferUsages::VERTEX),
        }
    }
}

#[derive(Default, Component, PartialEq, Eq, Clone)]
pub struct ObjectBatch {
    pub(crate) image_handle_id: AssetId<CompressedImage>,
    pub(crate) range: Range<u32>,
}

#[allow(clippy::too_many_arguments)]
pub fn queue_objects(
    draw_functions: Res<DrawFunctions<Transparent2d>>,
    object_pipeline: Res<ObjectPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<ObjectPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    msaa: Res<Msaa>,
    mut extracted_layers: ResMut<ExtractedLayers>,
    mut phases: Query<&mut RenderPhase<Transparent2d>>,
) {
    let view_key = ObjectPipelineKey::from_msaa_samples(msaa.samples());

    let draw_object_function = draw_functions.read().id::<DrawObject>();

    let pipeline = pipelines.specialize(&pipeline_cache, &object_pipeline, view_key);

    for mut transparent_phase in &mut phases {
        transparent_phase
            .items
            .reserve(extracted_layers.layers.len());

        let mut total_size = 0;

        for (layer_index, extracted_layer) in &mut extracted_layers.layers {
            let extracted_layer = extracted_layer.get_mut();

            if extracted_layer.is_empty() {
                continue;
            }

            total_size += extracted_layer.len();

            let entity_bits = 55555_u64 << 32 | (layer_index.to_u32() as u64);
            transparent_phase.add(Transparent2d {
                draw_function: draw_object_function,
                pipeline,
                // Instead of passing an `Entity`, use this field to pass the index of this layer
                entity: Entity::from_bits(entity_bits),
                sort_key: FloatOrd(layer_index.0 as f32),
                batch_range: 0..1,
                dynamic_offset: None,
            });
        }

        extracted_layers.total_size = total_size;
    }

    let compute_task_pool = ComputeTaskPool::get();

    // Sort the layers
    compute_task_pool.scope(|scope| {
        for (_, extracted_layer) in extracted_layers.layers.iter() {
            scope.spawn(async move {
                radsort::sort_by_cached_key(
                    unsafe { &mut *extracted_layer.get() },
                    |extracted_object| extracted_object.transform.translation().z,
                )
            });
        }
    });
}

#[derive(Resource, Default)]
pub struct ImageBindGroups {
    values: HashMap<AssetId<CompressedImage>, BindGroup>,
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_objects(
    mut commands: Commands,
    mut previous_len: Local<usize>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut object_meta: ResMut<ObjectMeta>,
    view_uniforms: Res<ViewUniforms>,
    object_pipeline: Res<ObjectPipeline>,
    mut image_bind_groups: ResMut<ImageBindGroups>,
    gpu_images: Res<RenderAssets<CompressedImage>>,
    extracted_layers: Res<ExtractedLayers>,
    mut phases: Query<&mut RenderPhase<Transparent2d>>,
) {
    let Some(view_binding) = view_uniforms.uniforms.binding() else {
        return;
    };

    let mut batches: Vec<(usize, ObjectBatch)> = Vec::with_capacity(*previous_len);

    object_meta.view_bind_group = Some(render_device.create_bind_group(
        "object_view_bind_group",
        &object_pipeline.view_layout,
        &BindGroupEntries::single(view_binding),
    ));

    let instance_buffer_values = object_meta.instance_buffer.values_mut();

    instance_buffer_values.resize(extracted_layers.total_size, ObjectInstance::default());

    let mut index = 0;

    for mut transparent_phase in &mut phases {
        let mut batch_item_index = 0;
        let mut batch_image_size = Vec2::ZERO;
        let mut batch_image_handle = AssetId::invalid();

        let gpu_image = &object_pipeline.dummy_white_gpu_image;

        batch_image_size = Vec2::new(gpu_image.size.x, gpu_image.size.y);
        batch_image_handle = AssetId::default();
        image_bind_groups
            .values
            .entry(batch_image_handle)
            .or_insert_with(|| {
                render_device.create_bind_group(
                    "object_material_bind_group",
                    &object_pipeline.material_layout,
                    &BindGroupEntries::sequential((&gpu_image.texture_view, &gpu_image.sampler)),
                )
            });

        let mut instance_mut_ref = &mut instance_buffer_values[..];

        let compute_task_pool = ComputeTaskPool::get();

        compute_task_pool.scope(|scope| {
            // Iterate through the phase items and detect when successive sprites that can be batched.
            // Spawn an entity with a `SpriteBatch` component for each possible batch.
            // Compatible items share the same entity.
            for item_index in 0..transparent_phase.items.len() {
                let item = &transparent_phase.items[item_index];

                if item.entity.generation() != 55555 {
                    batch_image_handle = AssetId::invalid();
                    continue;
                }

                let item_layer_index = LayerIndex::from_u32(item.entity.index());

                let Some((_, extracted_layer)) = extracted_layers
                    .layers
                    .iter()
                    .find(|(layer_index, _)| layer_index.0 == item_layer_index.0)
                else {
                    batch_image_handle = AssetId::invalid();
                    continue;
                };

                let extracted_layer = unsafe { &*extracted_layer.get() };

                let (this_chunk, other_chunk) =
                    instance_mut_ref.split_at_mut(extracted_layer.len());
                instance_mut_ref = other_chunk;

                assert_eq!(extracted_layer.len(), this_chunk.len());

                let a = info_span!("prepare_objects: layer task");
                scope.spawn(async move {
                    let _a = a.enter();
                    for (extracted_object, buffer_entry) in extracted_layer.iter().zip(this_chunk) {
                        // By default, the size of the quad is the size of the texture
                        let mut quad_size = batch_image_size;

                        // Calculate vertex data for this item
                        let mut uv_offset_scale: Vec4;

                        // If a rect is specified, adjust UVs and the size of the quad
                        if let Some(rect) = extracted_object.rect {
                            let rect_size = rect.size();
                            uv_offset_scale = Vec4::new(
                                rect.min.x / batch_image_size.x,
                                rect.max.y / batch_image_size.y,
                                rect_size.x / batch_image_size.x,
                                -rect_size.y / batch_image_size.y,
                            );
                            quad_size = rect_size;
                        } else {
                            uv_offset_scale = Vec4::new(0.0, 1.0, 1.0, -1.0);
                        }

                        if extracted_object.flip_x {
                            uv_offset_scale.x += uv_offset_scale.z;
                            uv_offset_scale.z *= -1.0;
                        }
                        if extracted_object.flip_y {
                            uv_offset_scale.y += uv_offset_scale.w;
                            uv_offset_scale.w *= -1.0;
                        }

                        // Override the size if a custom one is specified
                        if let Some(custom_size) = extracted_object.custom_size {
                            quad_size = custom_size;
                        }
                        quad_size = Vec2::new(50., 50.);

                        let transform = extracted_object.transform.affine()
                            * Affine3A::from_scale_rotation_translation(
                                quad_size.extend(1.0),
                                Quat::IDENTITY,
                                (quad_size * (-extracted_object.anchor - Vec2::splat(0.5)))
                                    .extend(0.0),
                            );

                        // Store the vertex data and add the item to the render phase
                        *buffer_entry = ObjectInstance::from(
                            &transform,
                            &extracted_object.color,
                            &uv_offset_scale,
                            0,
                        );
                    }
                });

                batches.push((
                    item_index,
                    ObjectBatch {
                        image_handle_id: batch_image_handle,
                        range: index..index + (extracted_layer.len() as u32),
                    },
                ));

                index += extracted_layer.len() as u32;
            }
        });

        for (item_index, batch) in &mut batches {
            let batch_id = commands.spawn(std::mem::take(batch)).id();
            transparent_phase.items[*item_index].entity = batch_id;
        }
    }

    object_meta
        .instance_buffer
        .write_buffer(&render_device, &render_queue);

    *previous_len = batches.len();
}

pub type DrawObject = (
    SetItemPipeline,
    SetObjectViewBindGroup<0>,
    SetObjectTextureBindGroup<1>,
    DrawObjectBatch,
);

pub struct SetObjectViewBindGroup<const I: usize>;

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetObjectViewBindGroup<I> {
    type Param = SRes<ObjectMeta>;
    type ViewWorldQuery = Read<ViewUniformOffset>;
    type ItemWorldQuery = ();

    fn render<'w>(
        _item: &P,
        view_uniform: &'_ ViewUniformOffset,
        _entity: (),
        sprite_meta: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        pass.set_bind_group(
            I,
            sprite_meta.into_inner().view_bind_group.as_ref().unwrap(),
            &[view_uniform.offset],
        );
        RenderCommandResult::Success
    }
}

pub struct SetObjectTextureBindGroup<const I: usize>;

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetObjectTextureBindGroup<I> {
    type Param = SRes<ImageBindGroups>;
    type ViewWorldQuery = ();
    type ItemWorldQuery = Read<ObjectBatch>;

    fn render<'w>(
        _item: &P,
        _view: (),
        batch: &'_ ObjectBatch,
        image_bind_groups: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let image_bind_groups = image_bind_groups.into_inner();

        pass.set_bind_group(
            I,
            image_bind_groups
                .values
                .get(&batch.image_handle_id)
                .unwrap(),
            &[],
        );
        RenderCommandResult::Success
    }
}

pub struct DrawObjectBatch;

impl<P: PhaseItem> RenderCommand<P> for DrawObjectBatch {
    type Param = SRes<ObjectMeta>;
    type ViewWorldQuery = ();
    type ItemWorldQuery = Read<ObjectBatch>;

    fn render<'w>(
        _item: &P,
        _view: (),
        batch: &'_ ObjectBatch,
        sprite_meta: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let sprite_meta = sprite_meta.into_inner();
        pass.set_vertex_buffer(0, sprite_meta.instance_buffer.buffer().unwrap().slice(..));
        pass.draw(1..7, batch.range.clone());
        RenderCommandResult::Success
    }
}