use std::hash::Hash;
use std::num::NonZeroU32;
use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};

use bevy::app::{App, Plugin};
use bevy::asset::{load_internal_asset, AssetId, Handle};
use bevy::core::{Pod, Zeroable};
use bevy::core_pipeline::core_2d::Transparent2d;
use bevy::ecs::system::{
    lifetimeless::{Read, SRes},
    SystemParamItem, SystemState,
};
use bevy::log::warn;
use bevy::math::{Affine2, Rect, Vec2, Vec2Swizzles, Vec4};
use bevy::prelude::{
    Commands, Component, Entity, FromWorld, Image, IntoSystemConfigs, Msaa, Query, Res, ResMut,
    Resource, Shader, World,
};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::BindGroupLayoutEntries;
use bevy::render::{
    mesh::PrimitiveTopology,
    render_asset::RenderAssets,
    render_phase::{
        AddRenderCommand, DrawFunctions, PhaseItem, RenderCommand, RenderCommandResult,
        RenderPhase, SetItemPipeline, TrackedRenderPass,
    },
    render_resource::{
        BindGroup, BindGroupEntries, BindGroupEntry, BindingResource, BufferUsages, BufferVec,
        IndexFormat, PipelineCache, Sampler, TextureView, WgpuFeatures,
    },
    render_resource::{
        BindGroupLayout, BlendState, ColorTargetState, ColorWrites, FragmentState, FrontFace,
        ImageCopyTexture, ImageDataLayout, MultisampleState, Origin3d, PolygonMode, PrimitiveState,
        RenderPipelineDescriptor, SamplerBindingType, ShaderStages, SpecializedRenderPipeline,
        SpecializedRenderPipelines, TextureAspect, TextureFormat, TextureSampleType,
        TextureViewDescriptor, VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
    },
    renderer::{RenderDevice, RenderQueue},
    texture::{BevyDefault, DefaultImageSampler, GpuImage, ImageSampler, TextureFormatPixelInfo},
    view::ViewUniform,
    view::{ViewUniformOffset, ViewUniforms},
    Extract, ExtractSchedule, Render, RenderApp, RenderSet,
};
use bevy::tasks::ComputeTaskPool;
use bevy::utils::{syncunsafecell::SyncUnsafeCell, FloatOrd};

use crate::level::color::{HsvMod, ObjectColor, ObjectColorCalculated, ObjectColorKind};
use crate::level::transform::GlobalTransform2d;
use crate::level::{object::Object, section::GlobalSections, LevelWorld};
use crate::state::level::Options;

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
                fallbacks.texture_array_size = 1;
            } else {
                fallbacks.texture_array_size = 16;
            }

            render_app
                .insert_resource(fallbacks)
                .init_resource::<ObjectPipeline>();
        };
    }
}

#[derive(Default, Resource)]
pub struct Fallbacks {
    texture_array_size: usize,
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

        let view_layout = render_device.create_bind_group_layout(
            "sprite_view_layout",
            &BindGroupLayoutEntries::single(
                ShaderStages::VERTEX_FRAGMENT,
                uniform_buffer::<ViewUniform>(true),
            ),
        );

        let material_layout = render_device.create_bind_group_layout(
            "sprite_material_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                if fallbacks.texture_array_size != 1 {
                    let texture_count =
                        NonZeroU32::new(fallbacks.texture_array_size as u32).unwrap();
                    (
                        texture_2d(TextureSampleType::Float { filterable: true })
                            .count(texture_count),
                        sampler(SamplerBindingType::Filtering).count(texture_count),
                    )
                } else {
                    (
                        texture_2d(TextureSampleType::Float { filterable: true }),
                        sampler(SamplerBindingType::Filtering),
                    )
                },
            ),
        );
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
        const NO_TEXTURE_ARRAY                  = 1 << 0;
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
        let vertex_layout = VertexBufferLayout::from_vertex_formats(
            VertexStepMode::Instance,
            vec![
                // @location(0) i_model_row0: vec2<f32>,
                VertexFormat::Float32x2,
                // @location(1) i_model_row1: vec2<f32>,
                VertexFormat::Float32x2,
                // @location(2) i_model_row2: vec2<f32>,
                VertexFormat::Float32x2,
                // @location(3) i_color: vec4<f32>,
                VertexFormat::Float32x4,
                // @location(4) i_uv_offset_scale: vec4<f32>,
                VertexFormat::Float32x4,
                // @location(5) i_texture_index: u32
                VertexFormat::Uint32,
                // @location(6) i_hsv: vec3<f32>
                VertexFormat::Float32x3,
                // @location(7) i_flags: u32
                VertexFormat::Uint32,
            ],
        );

        let mut shader_defs = Vec::new();

        if key.contains(ObjectPipelineKey::NO_TEXTURE_ARRAY) {
            shader_defs.push("NO_TEXTURE_ARRAY".into());
        }

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
    transform: GlobalTransform2d,
    color: Vec4,
    hsv: Option<HsvMod>,
    blending: bool,
    /// Select an area of the texture
    rect: Rect,
    /// Asset ID of the [`Image`] of this sprite
    /// PERF: storing an `AssetId` instead of `Handle<Image>` enables some optimizations (`ExtractedSprite` becomes `Copy` and doesn't need to be dropped)
    image_handle_id: AssetId<Image>,
    // flip_x: bool,
    // flip_y: bool,
    anchor: Vec2,
    rotated: bool,
    entity: Entity,
}

#[derive(Default, Resource)]
pub(crate) struct ExtractedLayers {
    layers: Vec<(i32, SyncUnsafeCell<Vec<ExtractedObject>>)>,
    total_size: usize,
}

#[derive(Default, Resource)]
pub(crate) struct ExtractSystemStateCache {
    cached_system_state: Option<
        SystemState<(
            Res<'static, GlobalSections>,
            Query<
                'static,
                'static,
                (
                    Entity,
                    &'static GlobalTransform2d,
                    &'static Object,
                    &'static ObjectColor,
                    &'static ObjectColorCalculated,
                    &'static Handle<Image>,
                ),
            >,
        )>,
    >,
}

pub(crate) fn extract_objects(
    mut extract_system_state_cache: ResMut<ExtractSystemStateCache>,
    mut extracted_layers: ResMut<ExtractedLayers>,
    level_world: Extract<Option<Res<LevelWorld>>>,
    options: Extract<Option<Res<Options>>>,
) {
    let Some(level_world) = &*level_world else {
        return;
    };

    let Some(options) = &*options else {
        return;
    };

    let LevelWorld::World(world) = &**level_world else {
        // There's nothing to render
        return;
    };

    let system_state = match extract_system_state_cache
        .cached_system_state
        .as_mut()
        .filter(|system_state| system_state.matches_world(world.id()))
    {
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

            let system_state: SystemState<(
                Res<GlobalSections>,
                Query<(
                    Entity,
                    &GlobalTransform2d,
                    &Object,
                    &ObjectColor,
                    &ObjectColorCalculated,
                    &Handle<Image>,
                )>,
            )> = SystemState::new(world_mut);

            extract_system_state_cache.cached_system_state = Some(system_state);

            extract_system_state_cache
                .cached_system_state
                .as_mut()
                .unwrap()
        }
    };

    let (global_sections, objects) = system_state.get(world);

    for (_, extracted_layer) in &mut extracted_layers.layers {
        extracted_layer.get_mut().clear();
    }

    for section in &global_sections.sections[global_sections.visible.clone()] {
        for (entity, transform, object, object_color, object_color_calc, image_handle) in
            objects.iter_many(section)
        {
            if !object_color_calc.enabled {
                continue;
            }

            if options.hide_triggers {
                match object.id {
                    29 | 30 | 31 | 32 | 33 | 34 | 104 | 105 | 221 | 717 | 718 | 743 | 744 | 900
                    | 915 | 1006 | 1268 | 1347 | 1520 | 1585 | 1595 | 1611 | 1612 | 1613 | 1616
                    | 1811 | 1812 | 1814 | 1815 | 1817 | 1818 | 1819 | 22 | 24 | 23 | 25 | 26
                    | 27 | 28 | 55 | 56 | 57 | 58 | 59 | 1912 | 1913 | 1914 | 1916 | 1917
                    | 1931 | 1932 | 1934 | 1935 | 2015 | 2016 | 2062 | 2067 | 2068 | 2701
                    | 2702 | 1586 | 1700 | 1755 | 1813 | 1829 | 1859 | 899 | 901 | 1007 | 1049
                    | 1346 => {
                        continue;
                    }
                    _ => (),
                }
            }

            let z_layer = object.z_layer
                - if object_color_calc.blending ^ (object.z_layer % 2 == 0) {
                    1
                } else {
                    0
                };

            let extracted_layer = if let Some((_, extracted_layer)) = extracted_layers
                .layers
                .iter_mut()
                .find(|(layer_index, _)| *layer_index == z_layer)
            {
                extracted_layer
            } else {
                let layer_index = extracted_layers.layers.len();
                extracted_layers.layers.push((
                    z_layer,
                    SyncUnsafeCell::new(Vec::with_capacity(10000)),
                ));
                &mut extracted_layers.layers[layer_index].1
            };

            let hsv = match object_color.object_color_kind {
                ObjectColorKind::None | ObjectColorKind::Black => None,
                _ => object_color.hsv,
            };

            extracted_layer.get_mut().push(ExtractedObject {
                transform: *transform,
                color: object_color_calc.color,
                hsv,
                blending: object_color_calc.blending,
                rect: object.frame.rect,
                image_handle_id: image_handle.id(),
                anchor: object.frame.anchor + object.anchor,
                rotated: object.frame.rotated,
                entity,
            });
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default, Pod, Zeroable)]
struct ObjectInstance {
    i_model: [Vec2; 3],
    i_color: [f32; 4],
    i_uv_offset_scale: [f32; 4],
    i_texture_index: u32,
    i_hsv: [f32; 3],
    i_flags: u32,
}

impl ObjectInstance {
    #[inline]
    fn from(
        transform: &Affine2,
        color: Vec4,
        uv_offset_scale: &Vec4,
        texture_index: u32,
        hsv_mod: Option<HsvMod>,
        blending: bool,
    ) -> Self {
        let (i_hsv, mut i_flags) = match hsv_mod {
            Some(hsv_mod) => hsv_mod.into(),
            None => ([0.; 3], FLAGS_HSV_DISABLED),
        };

        if blending {
            i_flags |= FLAGS_BLENDING;
        }

        Self {
            i_model: [
                transform.matrix2.x_axis,
                transform.matrix2.y_axis,
                transform.translation.xy(),
            ],
            i_color: color.to_array(),
            i_uv_offset_scale: uv_offset_scale.to_array(),
            i_texture_index: texture_index,
            i_hsv,
            i_flags,
        }
    }
}

const FLAGS_BLENDING: u32 = 1 << 0;
const FLAGS_HSV_DISABLED: u32 = 1 << 1;
const FLAGS_HSV_S_ABSOLUTE: u32 = 1 << 2;
const FLAGS_HSV_V_ABSOLUTE: u32 = 1 << 3;

impl From<HsvMod> for ([f32; 3], u32) {
    fn from(hsv: HsvMod) -> Self {
        let mut flags = 0;
        if hsv.s_absolute {
            flags |= FLAGS_HSV_S_ABSOLUTE;
        }
        if hsv.v_absolute {
            flags |= FLAGS_HSV_V_ABSOLUTE;
        }
        ([hsv.h, hsv.s, hsv.v], flags)
    }
}

#[derive(Resource)]
pub struct ObjectMeta {
    view_bind_group: Option<BindGroup>,
    index_buffer: BufferVec<u32>,
    instance_buffer: BufferVec<ObjectInstance>,
}

impl Default for ObjectMeta {
    fn default() -> Self {
        Self {
            view_bind_group: None,
            index_buffer: BufferVec::<u32>::new(BufferUsages::INDEX),
            instance_buffer: BufferVec::<ObjectInstance>::new(BufferUsages::VERTEX),
        }
    }
}

#[derive(Default, Component, PartialEq, Eq, Clone)]
pub struct ObjectBatch {
    pub(crate) ranges: Vec<(usize, Range<u32>)>,
}

pub(crate) fn queue_objects(mut extracted_layers: ResMut<ExtractedLayers>) {
    let compute_task_pool = ComputeTaskPool::get();

    let mut total_size = 0;

    // Sort the layers
    compute_task_pool.scope(|scope| {
        for (layer_index, extracted_layer) in &extracted_layers.layers {
            // let a = info_span!("queue_objects: layer sort task");
            total_size += unsafe { &*extracted_layer.get() }.len();
            if (layer_index % 2).abs() == 0 {
                // Sorting additive blending sprites aren't needed
                continue;
            }
            scope.spawn(async move {
                // let _a = a.enter();
                radsort::sort_by_cached_key(
                    unsafe { &mut *extracted_layer.get() },
                    |extracted_object| {
                        (
                            extracted_object.transform.z(),
                            extracted_object.entity.index(),
                        )
                    },
                )
            });
        }
    });

    extracted_layers.total_size = total_size;
}

#[derive(Resource, Default)]
pub struct ImageBindGroups {
    values: Vec<BindGroup>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_objects(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut object_meta: ResMut<ObjectMeta>,
    view_uniforms: Res<ViewUniforms>,
    object_pipeline: Res<ObjectPipeline>,
    mut image_bind_groups: ResMut<ImageBindGroups>,
    gpu_images: Res<RenderAssets<Image>>,
    mut extracted_layers: ResMut<ExtractedLayers>,
    mut phases: Query<&mut RenderPhase<Transparent2d>>,
    fallbacks: Res<Fallbacks>,
    msaa: Res<Msaa>,
    draw_functions: Res<DrawFunctions<Transparent2d>>,
    mut pipelines: ResMut<SpecializedRenderPipelines<ObjectPipeline>>,
    pipeline_cache: Res<PipelineCache>,
) {
    image_bind_groups.values.clear();

    let Some(view_binding) = view_uniforms.uniforms.binding() else {
        return;
    };

    object_meta.view_bind_group = Some(render_device.create_bind_group(
        "object_view_bind_group",
        &object_pipeline.view_layout,
        &BindGroupEntries::single(view_binding),
    ));

    let instance_buffer_values = object_meta.instance_buffer.values_mut();

    instance_buffer_values.resize(extracted_layers.total_size, ObjectInstance::default());

    let mut index = 0;

    let mut instance_mut_ref = &mut instance_buffer_values[..];
    let layers_batches: SyncUnsafeCell<Vec<Vec<(usize, Range<u32>)>>> =
        SyncUnsafeCell::new(Vec::with_capacity(extracted_layers.layers.len()));
    let dummy_image = &object_pipeline.dummy_white_gpu_image;
    let mut images_index = AtomicUsize::new(0);
    let images = SyncUnsafeCell::new(
        [(
            AssetId::invalid(),
            dummy_image.size,
            &dummy_image.texture_view,
            &dummy_image.sampler,
        ); 16],
    );

    let compute_task_pool = ComputeTaskPool::get();

    compute_task_pool.scope(|scope| {
        let fallbacks = &fallbacks;
        let images_index = &images_index;
        let images = &images;
        let gpu_images = &gpu_images;
        // Iterate through the phase items and detect when successive sprites that can be batched.
        // Spawn an entity with a `SpriteBatch` component for each possible batch.
        // Compatible items share the same entity.

        radsort::sort_by_cached_key(&mut extracted_layers.layers, |(layer_index, _)| *layer_index);

        for (_, extracted_layer) in &mut extracted_layers.layers {
            let extracted_layer = unsafe { &*extracted_layer.get() };

            if extracted_layer.is_empty() {
                continue;
            }

            let (this_chunk, other_chunk) = instance_mut_ref.split_at_mut(extracted_layer.len());
            instance_mut_ref = other_chunk;

            let layer_batches_mut = unsafe { &mut *layers_batches.get() };
            let layer_batches_index = layer_batches_mut.len();
            layer_batches_mut.push(Vec::new());

            let layer_batches = &layers_batches;

            // let a = info_span!("prepare_objects: layer task");
            scope.spawn(async move {
                // let _a = a.enter();
                let mut previous_image_group_index = 0;
                let mut batch_ranges = Vec::new();
                let mut batch_range = index..index;
                let images = unsafe { &mut *images.get() };
                for (extracted_object, buffer_entry) in extracted_layer.iter().zip(this_chunk) {
                    let (image_group_index, texture_index, current_image_size) = match images
                        .iter()
                        .position(|(asset_id, _, _, _)| {
                            *asset_id == extracted_object.image_handle_id
                        }) {
                        Some(index) => {
                            let y = index % fallbacks.texture_array_size;
                            let x = (index - y) / fallbacks.texture_array_size;
                            (x, y, images[index].1)
                        }
                        None => {
                            let Some(gpu_image) = gpu_images.get(extracted_object.image_handle_id)
                            else {
                                // Texture isn't ready yet
                                continue;
                            };

                            let new_index = images_index.fetch_add(1, Ordering::Relaxed);

                            images[new_index] = (
                                extracted_object.image_handle_id,
                                gpu_image.size,
                                &gpu_image.texture_view,
                                &gpu_image.sampler,
                            );

                            let y = new_index % fallbacks.texture_array_size;
                            let x = (new_index - y) / fallbacks.texture_array_size;
                            (x, y, gpu_image.size)
                        }
                    };

                    if image_group_index != previous_image_group_index && !batch_range.is_empty() {
                        let new_range = batch_range.end..batch_range.end;
                        batch_ranges.push((
                            previous_image_group_index,
                            std::mem::replace(&mut batch_range, new_range),
                        ));
                    }
                    previous_image_group_index = image_group_index;

                    // Calculate vertex data for this item
                    let rect_size = extracted_object.rect.size();
                    let uv_offset_scale = Vec4::new(
                        extracted_object.rect.min.x / current_image_size.x,
                        extracted_object.rect.max.y / current_image_size.y,
                        rect_size.x / current_image_size.x,
                        -rect_size.y / current_image_size.y,
                    );
                    let mut quad_size = rect_size;

                    // if extracted_object.flip_x {
                    //     uv_offset_scale.x += uv_offset_scale.z;
                    //     uv_offset_scale.z *= -1.0;
                    // }
                    // if extracted_object.flip_y {
                    //     uv_offset_scale.y += uv_offset_scale.w;
                    //     uv_offset_scale.w *= -1.0;
                    // }

                    // Texture atlas scale factor
                    quad_size /= 4.;

                    let mut transform = extracted_object.transform.affine();

                    if extracted_object.rotated {
                        let y_axis = -transform.x_axis;
                        transform.x_axis = transform.y_axis;
                        transform.y_axis = y_axis;
                    }

                    transform *= Affine2::from_scale_angle_translation(
                        quad_size,
                        0.,
                        quad_size * (-extracted_object.anchor - Vec2::splat(0.5)),
                    );

                    // Store the vertex data and add the item to the render phase
                    *buffer_entry = ObjectInstance::from(
                        &transform,
                        extracted_object.color,
                        &uv_offset_scale,
                        texture_index as u32,
                        extracted_object.hsv,
                        extracted_object.blending,
                    );

                    batch_range.end += 1;
                }

                batch_ranges.push((previous_image_group_index, batch_range));

                let layer_batch = &mut unsafe { &mut *layer_batches.get() }[layer_batches_index];
                *layer_batch = batch_ranges;
            });

            index += extracted_layer.len() as u32;
        }
    });

    let mut ranges: Vec<(usize, Range<u32>)> = Vec::with_capacity(50);

    for mut batch in layers_batches.into_inner() {
        if ranges
            .last()
            .and_then(|last| {
                batch
                    .first()
                    .and_then(|first| if last.0 == first.0 { Some(()) } else { None })
            })
            .is_some()
        {
            ranges.last_mut().unwrap().1.end = batch.first().unwrap().1.end;
            ranges.append(&mut batch[1..].to_vec());
        } else {
            ranges.append(&mut batch);
        }
    }

    if !ranges.is_empty() {
        let batch_id = commands.spawn(ObjectBatch { ranges }).id();
        let mut view_key = ObjectPipelineKey::from_msaa_samples(msaa.samples());
        if fallbacks.texture_array_size == 1 {
            view_key |= ObjectPipelineKey::NO_TEXTURE_ARRAY;
        }

        let draw_object_function = draw_functions.read().id::<DrawObject>();

        let pipeline = pipelines.specialize(&pipeline_cache, &object_pipeline, view_key);
        for mut phase in &mut phases {
            phase.add(Transparent2d {
                draw_function: draw_object_function,
                pipeline,
                // Instead of passing an `Entity`, use this field to pass the index of this layer
                entity: batch_id,
                sort_key: FloatOrd(0.),
                batch_range: 0..1,
                dynamic_offset: None,
            });
        }
    }

    for image_chunk in
        images.into_inner()[..*images_index.get_mut()].chunks(fallbacks.texture_array_size)
    {
        image_bind_groups.values.push(create_image_bind_group(
            image_chunk,
            fallbacks.texture_array_size,
            &object_pipeline,
            &render_device,
        ))
    }

    object_meta
        .instance_buffer
        .write_buffer(&render_device, &render_queue);

    if object_meta.index_buffer.len() != 6 {
        object_meta.index_buffer.clear();

        // NOTE: This code is creating 6 indices pointing to 4 vertices.
        // The vertices form the corners of a quad based on their two least significant bits.
        // 10   11
        //
        // 00   01
        // The sprite shader can then use the two least significant bits as the vertex index.
        // The rest of the properties to transform the vertex positions and UVs (which are
        // implicit) are baked into the instance transform, and UV offset and scale.
        // See object.wgsl for the details.
        let indices = [2, 0, 1, 1, 3, 2];
        object_meta.index_buffer.values_mut().extend(indices);

        object_meta
            .index_buffer
            .write_buffer(&render_device, &render_queue);
    }
}

fn create_image_bind_group(
    image_handles: &[(AssetId<Image>, Vec2, &TextureView, &Sampler)],
    image_bind_group_size: usize,
    object_pipeline: &ObjectPipeline,
    render_device: &RenderDevice,
) -> BindGroup {
    let (mut texture_views, mut samplers): (Vec<_>, Vec<_>) = image_handles
        .iter()
        .map(|(_, _, texture_view, sampler)| (&***texture_view, &***sampler))
        .unzip();

    texture_views.resize(
        image_bind_group_size,
        &object_pipeline.dummy_white_gpu_image.texture_view,
    );
    samplers.resize(
        image_bind_group_size,
        &object_pipeline.dummy_white_gpu_image.sampler,
    );

    if image_handles.len() != 1 {
        render_device.create_bind_group(
            Some("object_material_bind_group"),
            &object_pipeline.material_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureViewArray(&texture_views),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::SamplerArray(&samplers),
                },
            ],
        )
    } else {
        render_device.create_bind_group(
            Some("object_material_bind_group"),
            &object_pipeline.material_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(texture_views.first().unwrap()),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(samplers.first().unwrap()),
                },
            ],
        )
    }
}

pub type DrawObject = (
    SetItemPipeline,
    SetObjectViewBindGroup<0>,
    DrawObjectBatch<1>,
);

pub struct SetObjectViewBindGroup<const I: usize>;

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetObjectViewBindGroup<I> {
    type Param = SRes<ObjectMeta>;
    type ViewQuery = Read<ViewUniformOffset>;
    type ItemQuery = ();

    fn render<'w>(
        _item: &P,
        view_uniform: &'_ ViewUniformOffset,
        _entity: Option<()>,
        object_meta: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        pass.set_bind_group(
            I,
            object_meta.into_inner().view_bind_group.as_ref().unwrap(),
            &[view_uniform.offset],
        );
        RenderCommandResult::Success
    }
}

pub struct DrawObjectBatch<const I: usize>;

impl<P: PhaseItem, const I: usize> RenderCommand<P> for DrawObjectBatch<I> {
    type Param = (SRes<ObjectMeta>, SRes<ImageBindGroups>);
    type ViewQuery = ();
    type ItemQuery = Read<ObjectBatch>;

    fn render<'w>(
        _item: &P,
        _view: (),
        batch: Option<&'_ ObjectBatch>,
        (object_meta, image_bind_groups): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let object_meta = object_meta.into_inner();
        let image_bind_groups = image_bind_groups.into_inner();
        let Some(batch) = batch else {
            return RenderCommandResult::Failure;
        };

        pass.set_index_buffer(
            object_meta.index_buffer.buffer().unwrap().slice(..),
            0,
            IndexFormat::Uint32,
        );
        pass.set_vertex_buffer(0, object_meta.instance_buffer.buffer().unwrap().slice(..));

        for (bind_group_index, range) in &batch.ranges {
            pass.set_bind_group(I, &image_bind_groups.values[*bind_group_index], &[]);
            pass.draw_indexed(0..6, 0, range.clone());
        }
        RenderCommandResult::Success
    }
}
