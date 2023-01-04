use bevy::app::prelude::*;
use bevy::asset::{AddAsset, AssetEvent, Assets, Handle, HandleId, HandleUntyped};
use bevy::reflect::{TypeUuid, Uuid};
use bevy::render::{
    render_phase::AddRenderCommand,
    render_resource::{Shader, SpecializedRenderPipelines},
    Extract, RenderApp, RenderStage,
};
use bevy::sprite::{
    Anchor, ColorMaterialPlugin, ExtractedSprite, ExtractedSprites, Mesh2dHandle,
    Mesh2dRenderPlugin, Sprite, SpriteAssetEvents, SpriteSystem, TextureAtlas, TextureAtlasSprite,
};
use bevy::utils::{FloatOrd, HashMap};

use std::cmp::Ordering;

use bevy::core_pipeline::{core_2d::Transparent2d, tonemapping::Tonemapping};
use bevy::ecs::{
    prelude::*,
    system::{lifetimeless::*, SystemParamItem, SystemState},
};
use bevy::log::info;
use bevy::math::{Rect, Vec2};
use bevy::render::{
    color::Color,
    render_asset::RenderAssets,
    render_phase::{
        BatchedPhaseItem, DrawFunctions, EntityRenderCommand, RenderCommand, RenderCommandResult,
        RenderPhase, SetItemPipeline, TrackedRenderPass,
    },
    render_resource::*,
    renderer::{RenderDevice, RenderQueue},
    texture::{
        BevyDefault, DefaultImageSampler, GpuImage, Image, ImageSampler, TextureFormatPixelInfo,
    },
    view::{
        ComputedVisibility, ExtractedView, Msaa, ViewTarget, ViewUniform, ViewUniformOffset,
        ViewUniforms, VisibleEntities,
    },
};
use bevy::transform::components::GlobalTransform;
use bytemuck::{Pod, Zeroable};
use fixedbitset::FixedBitSet;

#[derive(Default)]
pub struct CustomSpritePlugin;

pub const SPRITE_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 2763343953151597127);

impl Plugin for CustomSpritePlugin {
    fn build(&self, app: &mut App) {
        let mut shaders = app.world.resource_mut::<Assets<Shader>>();
        let sprite_shader = Shader::from_wgsl(include_str!("sprite.wgsl"));
        shaders.set_untracked(SPRITE_SHADER_HANDLE, sprite_shader);
        app.add_asset::<TextureAtlas>()
            .register_asset_reflect::<TextureAtlas>()
            .register_type::<Sprite>()
            .register_type::<Anchor>()
            .register_type::<Mesh2dHandle>()
            .add_plugin(Mesh2dRenderPlugin)
            .add_plugin(ColorMaterialPlugin);

        if let Ok(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<ImageBindGroups>()
                .init_resource::<SpritePipeline>()
                .init_resource::<SpecializedRenderPipelines<SpritePipeline>>()
                .init_resource::<SpriteMeta>()
                .init_resource::<ExtractedSprites>()
                .init_resource::<ExtractedBlending>()
                .init_resource::<SpriteAssetEvents>()
                .add_render_command::<Transparent2d, DrawSprite>()
                .add_system_to_stage(
                    RenderStage::Extract,
                    extract_sprites.label(SpriteSystem::ExtractSprites),
                )
                .add_system_to_stage(RenderStage::Extract, extract_sprite_events)
                .add_system_to_stage(RenderStage::Queue, queue_sprites);
        };
    }
}

#[derive(Resource)]
pub struct SpritePipeline {
    view_layout: BindGroupLayout,
    material_layout: BindGroupLayout,
    pub dummy_white_gpu_image: GpuImage,
}

impl FromWorld for SpritePipeline {
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
            label: Some("sprite_view_layout"),
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
            label: Some("sprite_material_layout"),
        });
        let dummy_white_gpu_image = {
            let image = Image::new_fill(
                Extent3d::default(),
                TextureDimension::D2,
                &[255u8; 4],
                TextureFormat::bevy_default(),
            );
            let texture = render_device.create_texture(&image.texture_descriptor);
            let sampler = match image.sampler_descriptor {
                ImageSampler::Default => (**default_sampler).clone(),
                ImageSampler::Descriptor(descriptor) => render_device.create_sampler(&descriptor),
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
                    bytes_per_row: Some(
                        std::num::NonZeroU32::new(
                            image.texture_descriptor.size.width * format_size as u32,
                        )
                        .unwrap(),
                    ),
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
                size: Vec2::new(
                    image.texture_descriptor.size.width as f32,
                    image.texture_descriptor.size.height as f32,
                ),
            }
        };

        SpritePipeline {
            view_layout,
            material_layout,
            dummy_white_gpu_image,
        }
    }
}

bitflags::bitflags! {
    #[repr(transparent)]
    // NOTE: Apparently quadro drivers support up to 64x MSAA.
    // MSAA uses the highest 3 bits for the MSAA log2(sample count) to support up to 128x MSAA.
    pub struct SpritePipelineKey: u32 {
        const NONE                        = 0;
        const COLORED                     = (1 << 0);
        const HDR                         = (1 << 1);
        const TONEMAP_IN_SHADER           = (1 << 2);
        const DEBAND_DITHER               = (1 << 3);
        const BLENDING                    = (1 << 4);
        const MSAA_RESERVED_BITS          = Self::MSAA_MASK_BITS << Self::MSAA_SHIFT_BITS;
    }
}

impl SpritePipelineKey {
    const MSAA_MASK_BITS: u32 = 0b111;
    const MSAA_SHIFT_BITS: u32 = 32 - Self::MSAA_MASK_BITS.count_ones();

    #[inline]
    pub const fn from_msaa_samples(msaa_samples: u32) -> Self {
        let msaa_bits =
            (msaa_samples.trailing_zeros() & Self::MSAA_MASK_BITS) << Self::MSAA_SHIFT_BITS;
        Self::from_bits_truncate(msaa_bits)
    }

    #[inline]
    pub const fn msaa_samples(&self) -> u32 {
        1 << ((self.bits >> Self::MSAA_SHIFT_BITS) & Self::MSAA_MASK_BITS)
    }

    #[inline]
    pub const fn from_colored(colored: bool) -> Self {
        if colored {
            SpritePipelineKey::COLORED
        } else {
            SpritePipelineKey::NONE
        }
    }

    #[inline]
    pub const fn from_hdr(hdr: bool) -> Self {
        if hdr {
            SpritePipelineKey::HDR
        } else {
            SpritePipelineKey::NONE
        }
    }
}

impl SpecializedRenderPipeline for SpritePipeline {
    type Key = SpritePipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let mut formats = vec![
            // position
            VertexFormat::Float32x3,
            // uv
            VertexFormat::Float32x2,
        ];

        if key.contains(SpritePipelineKey::COLORED) {
            // color
            formats.push(VertexFormat::Float32x4);
        }

        let vertex_layout =
            VertexBufferLayout::from_vertex_formats(VertexStepMode::Vertex, formats);

        let mut shader_defs = Vec::new();
        if key.contains(SpritePipelineKey::COLORED) {
            shader_defs.push("COLORED".into());
        }

        if key.contains(SpritePipelineKey::TONEMAP_IN_SHADER) {
            shader_defs.push("TONEMAP_IN_SHADER".into());

            // Debanding is tied to tonemapping in the shader, cannot run without it.
            if key.contains(SpritePipelineKey::DEBAND_DITHER) {
                shader_defs.push("DEBAND_DITHER".into());
            }
        }

        let format = match key.contains(SpritePipelineKey::HDR) {
            true => ViewTarget::TEXTURE_FORMAT_HDR,
            false => TextureFormat::bevy_default(),
        };

        let blend_state = match key.contains(SpritePipelineKey::BLENDING) {
            true => BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::SrcAlpha,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::SrcAlpha,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
            },
            false => BlendState::ALPHA_BLENDING,
            // false => BlendState {
            //     color: BlendComponent {
            //         src_factor: BlendFactor::SrcAlpha,
            //         dst_factor: BlendFactor::One,
            //         operation: BlendOperation::Add,
            //     },
            //     alpha: BlendComponent {
            //         src_factor: BlendFactor::SrcAlpha,
            //         dst_factor: BlendFactor::One,
            //         operation: BlendOperation::Add,
            //     },
            // },
        };

        RenderPipelineDescriptor {
            vertex: VertexState {
                shader: SPRITE_SHADER_HANDLE.typed::<Shader>(),
                entry_point: "vertex".into(),
                shader_defs: shader_defs.clone(),
                buffers: vec![vertex_layout],
            },
            fragment: Some(FragmentState {
                shader: SPRITE_SHADER_HANDLE.typed::<Shader>(),
                shader_defs,
                entry_point: "fragment".into(),
                targets: vec![Some(ColorTargetState {
                    format,
                    blend: Some(blend_state),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            layout: Some(vec![self.view_layout.clone(), self.material_layout.clone()]),
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
            label: Some("sprite_pipeline".into()),
        }
    }
}

#[derive(Component)]
pub struct BlendingSprite;

#[derive(Resource, Default)]
pub struct ExtractedBlending {
    pub blending: HashMap<u32, bool>,
}

pub fn extract_sprite_events(
    mut events: ResMut<SpriteAssetEvents>,
    mut image_events: Extract<EventReader<AssetEvent<Image>>>,
) {
    let SpriteAssetEvents { ref mut images } = *events;
    images.clear();

    for image in image_events.iter() {
        // AssetEvent: !Clone
        images.push(match image {
            AssetEvent::Created { handle } => AssetEvent::Created {
                handle: handle.clone_weak(),
            },
            AssetEvent::Modified { handle } => AssetEvent::Modified {
                handle: handle.clone_weak(),
            },
            AssetEvent::Removed { handle } => AssetEvent::Removed {
                handle: handle.clone_weak(),
            },
        });
    }
}

pub fn extract_sprites(
    mut extracted_sprites: ResMut<ExtractedSprites>,
    mut extracted_blending: ResMut<ExtractedBlending>,
    texture_atlases: Extract<Res<Assets<TextureAtlas>>>,
    sprite_query: Extract<
        Query<(
            Entity,
            &ComputedVisibility,
            &Sprite,
            &GlobalTransform,
            &Handle<Image>,
            Option<&BlendingSprite>,
        )>,
    >,
    atlas_query: Extract<
        Query<(
            Entity,
            &ComputedVisibility,
            &TextureAtlasSprite,
            &GlobalTransform,
            &Handle<TextureAtlas>,
            Option<&BlendingSprite>,
        )>,
    >,
) {
    extracted_sprites.sprites.clear();
    extracted_blending.blending.clear();
    for (entity, visibility, sprite, transform, handle, blending) in sprite_query.iter() {
        if !visibility.is_visible() {
            continue;
        }
        // PERF: we don't check in this function that the `Image` asset is ready, since it should be in most cases and hashing the handle is expensive
        extracted_sprites.sprites.push(ExtractedSprite {
            entity,
            color: sprite.color,
            transform: *transform,
            rect: sprite.rect,
            // Pass the custom size
            custom_size: sprite.custom_size,
            flip_x: sprite.flip_x,
            flip_y: sprite.flip_y,
            image_handle_id: handle.id(),
            anchor: sprite.anchor.as_vec(),
        });
        extracted_blending
            .blending
            .insert(entity.index(), blending.is_some());
    }
    for (entity, visibility, atlas_sprite, transform, texture_atlas_handle, blending) in
        atlas_query.iter()
    {
        if !visibility.is_visible() {
            continue;
        }
        if let Some(texture_atlas) = texture_atlases.get(texture_atlas_handle) {
            let rect = Some(texture_atlas.textures[atlas_sprite.index]);
            extracted_sprites.sprites.push(ExtractedSprite {
                entity,
                color: atlas_sprite.color,
                transform: *transform,
                // Select the area in the texture atlas
                rect,
                // Pass the custom size
                custom_size: atlas_sprite.custom_size,
                flip_x: atlas_sprite.flip_x,
                flip_y: atlas_sprite.flip_y,
                image_handle_id: texture_atlas.texture.id(),
                anchor: atlas_sprite.anchor.as_vec(),
            });
            extracted_blending
                .blending
                .insert(entity.index(), blending.is_some());
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct SpriteVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ColoredSpriteVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

#[derive(Resource)]
pub struct SpriteMeta {
    vertices: BufferVec<SpriteVertex>,
    colored_vertices: BufferVec<ColoredSpriteVertex>,
    view_bind_group: Option<BindGroup>,
}

impl Default for SpriteMeta {
    fn default() -> Self {
        Self {
            vertices: BufferVec::new(BufferUsages::VERTEX),
            colored_vertices: BufferVec::new(BufferUsages::VERTEX),
            view_bind_group: None,
        }
    }
}

const QUAD_INDICES: [usize; 6] = [0, 2, 3, 0, 1, 2];

const QUAD_VERTEX_POSITIONS: [Vec2; 4] = [
    Vec2::new(-0.5, -0.5),
    Vec2::new(0.5, -0.5),
    Vec2::new(0.5, 0.5),
    Vec2::new(-0.5, 0.5),
];

const QUAD_UVS: [Vec2; 4] = [
    Vec2::new(0., 1.),
    Vec2::new(1., 1.),
    Vec2::new(1., 0.),
    Vec2::new(0., 0.),
];

#[derive(Component, Eq, PartialEq, Copy, Clone)]
pub struct SpriteBatch {
    image_handle_id: HandleId,
    colored: bool,
    blending: bool,
}

#[derive(Resource, Default)]
pub struct ImageBindGroups {
    values: HashMap<Handle<Image>, BindGroup>,
}

#[allow(clippy::too_many_arguments)]
pub fn queue_sprites(
    mut commands: Commands,
    mut view_entities: Local<FixedBitSet>,
    draw_functions: Res<DrawFunctions<Transparent2d>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut sprite_meta: ResMut<SpriteMeta>,
    view_uniforms: Res<ViewUniforms>,
    sprite_pipeline: Res<SpritePipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<SpritePipeline>>,
    mut pipeline_cache: ResMut<PipelineCache>,
    mut image_bind_groups: ResMut<ImageBindGroups>,
    gpu_images: Res<RenderAssets<Image>>,
    msaa: Res<Msaa>,
    mut extracted: (ResMut<ExtractedSprites>, Res<ExtractedBlending>),
    mut views: Query<(
        &mut RenderPhase<Transparent2d>,
        &VisibleEntities,
        &ExtractedView,
        Option<&Tonemapping>,
    )>,
    events: Res<SpriteAssetEvents>,
) {
    let (mut extracted_sprites, extracted_blending) = extracted;

    // If an image has changed, the GpuImage has (probably) changed
    for event in &events.images {
        match event {
            AssetEvent::Created { .. } => None,
            AssetEvent::Modified { handle } | AssetEvent::Removed { handle } => {
                image_bind_groups.values.remove(handle)
            }
        };
    }

    let msaa_key = SpritePipelineKey::from_msaa_samples(msaa.samples);

    if let Some(view_binding) = view_uniforms.uniforms.binding() {
        let sprite_meta = &mut sprite_meta;

        // Clear the vertex buffers
        sprite_meta.vertices.clear();
        sprite_meta.colored_vertices.clear();

        sprite_meta.view_bind_group = Some(render_device.create_bind_group(&BindGroupDescriptor {
            entries: &[BindGroupEntry {
                binding: 0,
                resource: view_binding,
            }],
            label: Some("sprite_view_bind_group"),
            layout: &sprite_pipeline.view_layout,
        }));

        let draw_sprite_function = draw_functions.read().get_id::<DrawSprite>().unwrap();

        // Vertex buffer indices
        let mut index = 0;
        let mut colored_index = 0;

        // FIXME: VisibleEntities is ignored

        let extracted_sprites = &mut extracted_sprites.sprites;
        // Sort sprites by z for correct transparency and then by handle to improve batching
        // NOTE: This can be done independent of views by reasonably assuming that all 2D views look along the negative-z axis in world space
        extracted_sprites.sort_by(|a, b| {
            match a
                .transform
                .translation()
                .z
                .partial_cmp(&b.transform.translation().z)
            {
                Some(Ordering::Equal) | None => a.image_handle_id.cmp(&b.image_handle_id),
                Some(other) => other,
            }
        });
        let image_bind_groups = &mut *image_bind_groups;

        for (mut transparent_phase, visible_entities, view, tonemapping) in &mut views {
            let mut view_key = SpritePipelineKey::from_hdr(view.hdr) | msaa_key;
            if let Some(Tonemapping::Enabled { deband_dither }) = tonemapping {
                if !view.hdr {
                    view_key |= SpritePipelineKey::TONEMAP_IN_SHADER;

                    if *deband_dither {
                        view_key |= SpritePipelineKey::DEBAND_DITHER;
                    }
                }
            }
            let pipeline = pipelines.specialize(
                &mut pipeline_cache,
                &sprite_pipeline,
                view_key | SpritePipelineKey::from_colored(false),
            );
            let colored_pipeline = pipelines.specialize(
                &mut pipeline_cache,
                &sprite_pipeline,
                view_key | SpritePipelineKey::from_colored(true),
            );
            let blending_pipeline = pipelines.specialize(
                &mut pipeline_cache,
                &sprite_pipeline,
                view_key | SpritePipelineKey::from_colored(false) | SpritePipelineKey::BLENDING,
            );
            let colored_blending_pipeline = pipelines.specialize(
                &mut pipeline_cache,
                &sprite_pipeline,
                view_key | SpritePipelineKey::from_colored(true) | SpritePipelineKey::BLENDING,
            );

            view_entities.clear();
            view_entities.extend(visible_entities.entities.iter().map(|e| e.index() as usize));
            transparent_phase.items.reserve(extracted_sprites.len());

            // Impossible starting values that will be replaced on the first iteration
            let mut current_batch = SpriteBatch {
                image_handle_id: HandleId::Id(Uuid::nil(), u64::MAX),
                colored: false,
                blending: false,
            };
            let mut current_batch_entity = Entity::from_raw(u32::MAX);
            let mut current_image_size = Vec2::ZERO;
            // Add a phase item for each sprite, and detect when succesive items can be batched.
            // Spawn an entity with a `SpriteBatch` component for each possible batch.
            // Compatible items share the same entity.
            // Batches are merged later (in `batch_phase_system()`), so that they can be interrupted
            // by any other phase item (and they can interrupt other items from batching).
            for extracted_sprite in extracted_sprites.iter() {
                if !view_entities.contains(extracted_sprite.entity.index() as usize) {
                    continue;
                }
                let new_batch = SpriteBatch {
                    image_handle_id: extracted_sprite.image_handle_id,
                    colored: extracted_sprite.color != Color::WHITE,
                    blending: match extracted_blending
                        .blending
                        .get(&extracted_sprite.entity.index())
                    {
                        Some(blend) => *blend,
                        None => false,
                    },
                };
                if new_batch != current_batch {
                    // Set-up a new possible batch
                    if let Some(gpu_image) =
                        gpu_images.get(&Handle::weak(new_batch.image_handle_id))
                    {
                        current_batch = new_batch;
                        current_image_size = Vec2::new(gpu_image.size.x, gpu_image.size.y);
                        current_batch_entity = commands.spawn(current_batch).id();

                        image_bind_groups
                            .values
                            .entry(Handle::weak(current_batch.image_handle_id))
                            .or_insert_with(|| {
                                render_device.create_bind_group(&BindGroupDescriptor {
                                    entries: &[
                                        BindGroupEntry {
                                            binding: 0,
                                            resource: BindingResource::TextureView(
                                                &gpu_image.texture_view,
                                            ),
                                        },
                                        BindGroupEntry {
                                            binding: 1,
                                            resource: BindingResource::Sampler(&gpu_image.sampler),
                                        },
                                    ],
                                    label: Some("sprite_material_bind_group"),
                                    layout: &sprite_pipeline.material_layout,
                                })
                            });
                    } else {
                        // Skip this item if the texture is not ready
                        continue;
                    }
                }

                // Calculate vertex data for this item

                let mut uvs = QUAD_UVS;
                if extracted_sprite.flip_x {
                    uvs = [uvs[1], uvs[0], uvs[3], uvs[2]];
                }
                if extracted_sprite.flip_y {
                    uvs = [uvs[3], uvs[2], uvs[1], uvs[0]];
                }

                // By default, the size of the quad is the size of the texture
                let mut quad_size = current_image_size;

                // If a rect is specified, adjust UVs and the size of the quad
                if let Some(rect) = extracted_sprite.rect {
                    let rect_size = rect.size();
                    for uv in &mut uvs {
                        *uv = (rect.min + *uv * rect_size) / current_image_size;
                    }
                    quad_size = rect_size;
                }

                // Override the size if a custom one is specified
                if let Some(custom_size) = extracted_sprite.custom_size {
                    quad_size = custom_size;
                }

                // Apply size and global transform
                let positions = QUAD_VERTEX_POSITIONS.map(|quad_pos| {
                    extracted_sprite
                        .transform
                        .transform_point(
                            ((quad_pos - extracted_sprite.anchor) * quad_size).extend(0.),
                        )
                        .into()
                });

                // These items will be sorted by depth with other phase items
                let sort_key = FloatOrd(extracted_sprite.transform.translation().z);

                // Store the vertex data and add the item to the render phase
                if current_batch.colored {
                    for i in QUAD_INDICES {
                        sprite_meta.colored_vertices.push(ColoredSpriteVertex {
                            position: positions[i],
                            uv: uvs[i].into(),
                            color: extracted_sprite.color.as_linear_rgba_f32(),
                        });
                    }
                    let item_start = colored_index;
                    colored_index += QUAD_INDICES.len() as u32;
                    let item_end = colored_index;
                    if current_batch.blending {
                        transparent_phase.add(Transparent2d {
                            draw_function: draw_sprite_function,
                            pipeline: colored_blending_pipeline,
                            entity: current_batch_entity,
                            sort_key,
                            batch_range: Some(item_start..item_end),
                        });
                    } else {
                        transparent_phase.add(Transparent2d {
                            draw_function: draw_sprite_function,
                            pipeline: colored_pipeline,
                            entity: current_batch_entity,
                            sort_key,
                            batch_range: Some(item_start..item_end),
                        });
                    }
                } else {
                    for i in QUAD_INDICES {
                        sprite_meta.vertices.push(SpriteVertex {
                            position: positions[i],
                            uv: uvs[i].into(),
                        });
                    }
                    let item_start = index;
                    index += QUAD_INDICES.len() as u32;
                    let item_end = index;
                    if current_batch.blending {
                        transparent_phase.add(Transparent2d {
                            draw_function: draw_sprite_function,
                            pipeline: blending_pipeline,
                            entity: current_batch_entity,
                            sort_key,
                            batch_range: Some(item_start..item_end),
                        });
                    } else {
                        transparent_phase.add(Transparent2d {
                            draw_function: draw_sprite_function,
                            pipeline,
                            entity: current_batch_entity,
                            sort_key,
                            batch_range: Some(item_start..item_end),
                        });
                    }
                }
            }
        }
        sprite_meta
            .vertices
            .write_buffer(&render_device, &render_queue);
        sprite_meta
            .colored_vertices
            .write_buffer(&render_device, &render_queue);
    }
}

pub type DrawSprite = (
    SetItemPipeline,
    SetSpriteViewBindGroup<0>,
    SetSpriteTextureBindGroup<1>,
    DrawSpriteBatch,
);

pub struct SetSpriteViewBindGroup<const I: usize>;
impl<const I: usize> EntityRenderCommand for SetSpriteViewBindGroup<I> {
    type Param = (SRes<SpriteMeta>, SQuery<Read<ViewUniformOffset>>);

    fn render<'w>(
        view: Entity,
        _item: Entity,
        (sprite_meta, view_query): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let view_uniform = view_query.get(view).unwrap();
        pass.set_bind_group(
            I,
            sprite_meta.into_inner().view_bind_group.as_ref().unwrap(),
            &[view_uniform.offset],
        );
        RenderCommandResult::Success
    }
}
pub struct SetSpriteTextureBindGroup<const I: usize>;
impl<const I: usize> EntityRenderCommand for SetSpriteTextureBindGroup<I> {
    type Param = (SRes<ImageBindGroups>, SQuery<Read<SpriteBatch>>);

    fn render<'w>(
        _view: Entity,
        item: Entity,
        (image_bind_groups, query_batch): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let sprite_batch = query_batch.get(item).unwrap();
        let image_bind_groups = image_bind_groups.into_inner();

        pass.set_bind_group(
            I,
            image_bind_groups
                .values
                .get(&Handle::weak(sprite_batch.image_handle_id))
                .unwrap(),
            &[],
        );
        RenderCommandResult::Success
    }
}

pub struct DrawSpriteBatch;
impl<P: BatchedPhaseItem> RenderCommand<P> for DrawSpriteBatch {
    type Param = (SRes<SpriteMeta>, SQuery<Read<SpriteBatch>>);

    fn render<'w>(
        _view: Entity,
        item: &P,
        (sprite_meta, query_batch): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let sprite_batch = query_batch.get(item.entity()).unwrap();
        let sprite_meta = sprite_meta.into_inner();
        if sprite_batch.colored {
            pass.set_vertex_buffer(0, sprite_meta.colored_vertices.buffer().unwrap().slice(..));
        } else {
            pass.set_vertex_buffer(0, sprite_meta.vertices.buffer().unwrap().slice(..));
        }
        pass.draw(item.batch_range().as_ref().unwrap().clone(), 0..1);
        RenderCommandResult::Success
    }
}
