use bevy::app::prelude::*;
use bevy::asset::{AddAsset, AssetEvent, Assets, Handle, HandleId, HandleUntyped};
use bevy::reflect::{TypeUuid, Uuid};
use bevy::render::{
    render_phase::AddRenderCommand,
    render_resource::{Shader, SpecializedRenderPipelines},
    Extract, ExtractSchedule, RenderApp, RenderSet,
};
use bevy::sprite::{
    queue_material2d_meshes, Anchor, ColorMaterial, ColorMaterialPlugin, ExtractedSprite,
    ExtractedSprites, Mesh2dHandle, Mesh2dRenderPlugin, Sprite, SpriteAssetEvents, SpriteSystem,
    TextureAtlas, TextureAtlasSprite,
};
use bevy::utils::{FloatOrd, HashMap, HashSet};
use std::cmp::Ordering;

use crate::level::color::ColorChannels;
use crate::level::object::Object;
use crate::level::Groups;
use bevy::core_pipeline::tonemapping::DebandDither;
use bevy::core_pipeline::{core_2d::Transparent2d, tonemapping::Tonemapping};
use bevy::ecs::{
    prelude::*,
    system::{lifetimeless::*, SystemParamItem, SystemState},
};
use bevy::log::info;
use bevy::math::{Vec2, Vec3, Vec4, Vec4Swizzles};
use bevy::render::{
    render_asset::RenderAssets,
    render_phase::{
        BatchedPhaseItem, DrawFunctions, PhaseItem, RenderCommand, RenderCommandResult,
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
                .init_resource::<ExtractedObjects>()
                .init_resource::<SpriteAssetEvents>()
                .init_resource::<PremultipliedImages>()
                .add_render_command::<Transparent2d, DrawSprite>()
                .add_systems(
                    (
                        extract_sprites.in_set(SpriteSystem::ExtractSprites),
                        extract_sprite_events,
                    )
                        .in_schedule(ExtractSchedule),
                )
                .add_system(prepare_premultiplied_images.in_set(RenderSet::Prepare))
                .add_system(
                    queue_sprites
                        .in_set(RenderSet::Queue)
                        .ambiguous_with(queue_material2d_meshes::<ColorMaterial>),
                );
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
            let image = Image::default();
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
                mip_level_count: image.texture_descriptor.mip_level_count,
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
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    #[repr(transparent)]
    // NOTE: Apparently quadro drivers support up to 64x MSAA.
    // MSAA uses the highest 3 bits for the MSAA log2(sample count) to support up to 128x MSAA.
    pub struct SpritePipelineKey: u32 {
        const NONE                              = 0;
        const COLORED                           = (1 << 0);
        const HDR                               = (1 << 1);
        const TONEMAP_IN_SHADER                 = (1 << 2);
        const DEBAND_DITHER                     = (1 << 3);
        const MSAA_RESERVED_BITS                = Self::MSAA_MASK_BITS << Self::MSAA_SHIFT_BITS;
        const TONEMAP_METHOD_RESERVED_BITS      = Self::TONEMAP_METHOD_MASK_BITS << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_NONE               = 0 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_REINHARD           = 1 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_REINHARD_LUMINANCE = 2 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_ACES_FITTED        = 3 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_AGX                = 4 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM = 5 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_TONY_MC_MAPFACE    = 6 << Self::TONEMAP_METHOD_SHIFT_BITS;
        const TONEMAP_METHOD_BLENDER_FILMIC     = 7 << Self::TONEMAP_METHOD_SHIFT_BITS;
    }
}

impl SpritePipelineKey {
    const MSAA_MASK_BITS: u32 = 0b111;
    const MSAA_SHIFT_BITS: u32 = 32 - Self::MSAA_MASK_BITS.count_ones();
    const TONEMAP_METHOD_MASK_BITS: u32 = 0b111;
    const TONEMAP_METHOD_SHIFT_BITS: u32 =
        Self::MSAA_SHIFT_BITS - Self::TONEMAP_METHOD_MASK_BITS.count_ones();

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
        let formats = vec![
            // center
            VertexFormat::Float32x2,
            // half_extents
            VertexFormat::Float32x2,
            // uv
            VertexFormat::Float32x4,
            // transform_x
            VertexFormat::Float32x3,
            // transform_y
            VertexFormat::Float32x3,
            // transform_z
            VertexFormat::Float32x3,
            // transform_w
            VertexFormat::Float32x3,
            // color
            VertexFormat::Float32x4,
        ];

        let vertex_layout =
            VertexBufferLayout::from_vertex_formats(VertexStepMode::Instance, formats);

        let mut shader_defs = Vec::new();

        if key.contains(SpritePipelineKey::TONEMAP_IN_SHADER) {
            shader_defs.push("TONEMAP_IN_SHADER".into());

            let method = key.intersection(SpritePipelineKey::TONEMAP_METHOD_RESERVED_BITS);

            if method == SpritePipelineKey::TONEMAP_METHOD_NONE {
                shader_defs.push("TONEMAP_METHOD_NONE".into());
            } else if method == SpritePipelineKey::TONEMAP_METHOD_REINHARD {
                shader_defs.push("TONEMAP_METHOD_REINHARD".into());
            } else if method == SpritePipelineKey::TONEMAP_METHOD_REINHARD_LUMINANCE {
                shader_defs.push("TONEMAP_METHOD_REINHARD_LUMINANCE".into());
            } else if method == SpritePipelineKey::TONEMAP_METHOD_ACES_FITTED {
                shader_defs.push("TONEMAP_METHOD_ACES_FITTED".into());
            } else if method == SpritePipelineKey::TONEMAP_METHOD_AGX {
                shader_defs.push("TONEMAP_METHOD_AGX".into());
            } else if method == SpritePipelineKey::TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM
            {
                shader_defs.push("TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM".into());
            } else if method == SpritePipelineKey::TONEMAP_METHOD_BLENDER_FILMIC {
                shader_defs.push("TONEMAP_METHOD_BLENDER_FILMIC".into());
            } else if method == SpritePipelineKey::TONEMAP_METHOD_TONY_MC_MAPFACE {
                shader_defs.push("TONEMAP_METHOD_TONY_MC_MAPFACE".into());
            }

            // Debanding is tied to tonemapping in the shader, cannot run without it.
            if key.contains(SpritePipelineKey::DEBAND_DITHER) {
                shader_defs.push("DEBAND_DITHER".into());
            }
        }

        let format = match key.contains(SpritePipelineKey::HDR) {
            true => ViewTarget::TEXTURE_FORMAT_HDR,
            false => TextureFormat::bevy_default(),
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
            label: Some("sprite_pipeline".into()),
            push_constant_ranges: Vec::new(),
        }
    }
}

#[derive(Resource)]
pub struct ExtractedSpriteImages {
    extracted: Vec<(Handle<Image>, Image)>,
    removed: Vec<Handle<Image>>,
}

pub fn extract_sprite_events(
    mut commands: Commands,
    mut events: ResMut<SpriteAssetEvents>,
    mut image_events: Extract<EventReader<AssetEvent<Image>>>,
    asset_images: Extract<Res<Assets<Image>>>,
) {
    let SpriteAssetEvents { ref mut images } = *events;
    images.clear();

    let mut changed_images = HashSet::default();
    let mut removed = Vec::new();

    for image in image_events.iter() {
        // AssetEvent: !Clone
        match image {
            AssetEvent::Created { handle } => {
                images.push(AssetEvent::Created {
                    handle: handle.clone_weak(),
                });
                changed_images.insert(handle.clone_weak());
            }
            AssetEvent::Modified { handle } => {
                images.push(AssetEvent::Modified {
                    handle: handle.clone_weak(),
                });
                changed_images.insert(handle.clone_weak());
            }
            AssetEvent::Removed { handle } => {
                images.push(AssetEvent::Removed {
                    handle: handle.clone_weak(),
                });
                changed_images.remove(handle);
                removed.push(handle.clone_weak());
            }
        }
    }

    let mut extracted_images = Vec::new();
    for handle in changed_images.drain() {
        if let Some(image) = asset_images.get(&handle) {
            extracted_images.push((handle, image.clone()));
        }
    }

    commands.insert_resource(ExtractedSpriteImages {
        extracted: extracted_images,
        removed,
    })
}

#[derive(Resource, Default)]
pub struct ExtractedObjects {
    objects: HashMap<Entity, ExtractedObject>,
}

pub struct ExtractedObject {
    rotated: bool,
    z_layer: i8,
    blending: bool,
}

fn extract_sprites(
    mut extracted_sprites: ResMut<ExtractedSprites>,
    mut extracted_objects: ResMut<ExtractedObjects>,
    texture_atlases: Extract<Res<Assets<TextureAtlas>>>,
    color_channels: Extract<Res<ColorChannels>>,
    groups: Extract<Res<Groups>>,
    sprite_query: Extract<
        Query<
            (
                Entity,
                &ComputedVisibility,
                &Sprite,
                &GlobalTransform,
                &Handle<Image>,
            ),
            Without<Object>,
        >,
    >,
    atlas_query: Extract<
        Query<
            (
                Entity,
                &ComputedVisibility,
                &TextureAtlasSprite,
                &GlobalTransform,
                &Handle<TextureAtlas>,
            ),
            Without<Object>,
        >,
    >,
    object_query: Extract<
        Query<(
            Entity,
            &TextureAtlasSprite,
            &GlobalTransform,
            &Handle<TextureAtlas>,
            &Object,
        )>,
    >,
    camera_query: Extract<Query<&VisibleEntities>>,
) {
    extracted_sprites.sprites.clear();
    extracted_objects.objects.clear();

    for visible_entities in &camera_query {
        'outer: for entity in &visible_entities.entities {
            if let Ok((entity, atlas_sprite, transform, atlas_handle, object)) =
                object_query.get(*entity)
            {
                if let Some(texture_atlas) = texture_atlases.get(atlas_handle) {
                    let mut opacity = 1.;
                    for group_id in &object.groups {
                        if let Some(group) = groups.0.get(group_id) {
                            if !group.activated {
                                continue 'outer;
                            }
                            opacity *= group.opacity;
                        }
                    }
                    let (mut color, blending) = color_channels.get_color(&object.color_channel);
                    if let Some(hsv) = &object.hsv {
                        hsv.apply(color);
                    }
                    color.set_a(color.a() * opacity);
                    if blending {
                        color.set_a(color.a().powf(4.475));
                    }

                    let rect = Some(texture_atlas.textures[atlas_sprite.index]);

                    extracted_objects.objects.insert(
                        entity,
                        ExtractedObject {
                            rotated: object.rotated,
                            z_layer: if blending {
                                object.z_layer - 1
                            } else {
                                object.z_layer
                            },
                            blending,
                        },
                    );
                    extracted_sprites.sprites.push(ExtractedSprite {
                        entity,
                        color,
                        transform: *transform,
                        rect,
                        // Pass the custom size
                        custom_size: atlas_sprite.custom_size,
                        flip_x: atlas_sprite.flip_x,
                        flip_y: atlas_sprite.flip_y,
                        image_handle_id: texture_atlas.texture.id(),
                        anchor: atlas_sprite.anchor.as_vec(),
                    });
                }
            }
        }
    }

    for (entity, visibility, sprite, transform, handle) in sprite_query.iter() {
        if !visibility.is_visible() {
            continue;
        }
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
    }
    for (entity, visibility, atlas_sprite, transform, texture_atlas_handle) in atlas_query.iter() {
        if !visibility.is_visible() {
            return;
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
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct SpriteInstance {
    pub anchor: [f32; 2],
    pub half_extents: [f32; 2],
    pub uv: [f32; 4],
    pub transform_1: [f32; 3],
    pub transform_2: [f32; 3],
    pub transform_3: [f32; 3],
    pub transform_4: [f32; 3],
    pub color: [f32; 4],
}

#[derive(Resource)]
pub struct SpriteMeta {
    instances: BufferVec<SpriteInstance>,
    view_bind_group: Option<BindGroup>,
}

impl Default for SpriteMeta {
    fn default() -> Self {
        Self {
            instances: BufferVec::new(BufferUsages::VERTEX),
            view_bind_group: None,
        }
    }
}

#[derive(Component, Eq, PartialEq, Copy, Clone)]
pub struct SpriteBatch {
    image_handle_id: HandleId,
}

#[derive(Resource, Default)]
pub struct ImageBindGroups {
    values: HashMap<Handle<Image>, BindGroup>,
}

#[derive(Resource, Default)]
pub struct PremultipliedImages {
    values: HashMap<Handle<Image>, GpuImage>,
}

pub fn prepare_premultiplied_images(
    mut extracted_images: ResMut<ExtractedSpriteImages>,
    mut premultiplied_images: ResMut<PremultipliedImages>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    default_sampler: Res<DefaultImageSampler>,
) {
    for removed in std::mem::take(&mut extracted_images.removed) {
        premultiplied_images.values.remove(&removed);
    }

    for (handle, image) in std::mem::take(&mut extracted_images.extracted) {
        let premultiplied_data = image
            .data
            .chunks_exact(4)
            .map(|pixel| {
                let alpha = pixel[3] as f32 / u8::MAX as f32;
                // Premultiply
                [
                    (pixel[0] as f32 * alpha) as u8,
                    (pixel[1] as f32 * alpha) as u8,
                    (pixel[2] as f32 * alpha) as u8,
                    pixel[3],
                ]
            })
            .collect::<Vec<[u8; 4]>>()
            .concat();

        let texture = render_device.create_texture_with_data(
            &render_queue,
            &image.texture_descriptor,
            &premultiplied_data,
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
            ImageSampler::Default => (**default_sampler).clone(),
            ImageSampler::Descriptor(descriptor) => render_device.create_sampler(&descriptor),
        };

        premultiplied_images.values.insert(
            handle,
            GpuImage {
                texture,
                texture_view,
                texture_format: image.texture_descriptor.format,
                sampler,
                size,
                mip_level_count: image.texture_descriptor.mip_level_count,
            },
        );
    }
}

const QUAD_UV: Vec4 = Vec4::new(1., 0., -1., 1.);

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
    pipeline_cache: Res<PipelineCache>,
    mut image_bind_groups: ResMut<ImageBindGroups>,
    premultiplied_images: Res<PremultipliedImages>,
    msaa: Res<Msaa>,
    extracted: (ResMut<ExtractedSprites>, Res<ExtractedObjects>),
    mut views: Query<(
        &mut RenderPhase<Transparent2d>,
        &VisibleEntities,
        &ExtractedView,
        Option<&Tonemapping>,
        Option<&DebandDither>,
    )>,
    events: Res<SpriteAssetEvents>,
) {
    let (mut extracted_sprites, extracted_objects) = extracted;

    // If an image has changed, the GpuImage has (probably) changed
    for event in &events.images {
        match event {
            AssetEvent::Created { .. } => None,
            AssetEvent::Modified { handle } | AssetEvent::Removed { handle } => {
                image_bind_groups.values.remove(handle)
            }
        };
    }

    let msaa_key = SpritePipelineKey::from_msaa_samples(msaa.samples());

    if let Some(view_binding) = view_uniforms.uniforms.binding() {
        let sprite_meta = &mut sprite_meta;

        // Clear the vertex buffer
        sprite_meta.instances.clear();

        sprite_meta.view_bind_group = Some(render_device.create_bind_group(&BindGroupDescriptor {
            entries: &[BindGroupEntry {
                binding: 0,
                resource: view_binding,
            }],
            label: Some("sprite_view_bind_group"),
            layout: &sprite_pipeline.view_layout,
        }));

        let draw_sprite_function = draw_functions.read().id::<DrawSprite>();

        // Vertex buffer indices
        let mut index = 0;

        // FIXME: VisibleEntities is ignored

        let extracted_sprites = &mut extracted_sprites.sprites;
        // Sort sprites by z for correct transparency and then by handle to improve batching
        // NOTE: This can be done independent of views by reasonably assuming that all 2D views look along the negative-z axis in world space
        extracted_sprites.sort_unstable_by(|a, b| {
            if let Some(object_a) = extracted_objects.objects.get(&a.entity) {
                if let Some(object_b) = extracted_objects.objects.get(&b.entity) {
                    match object_a.z_layer.partial_cmp(&object_b.z_layer) {
                        Some(Ordering::Equal) | None => (),
                        Some(other) => {
                            return other;
                        }
                    };
                }
            }
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

        for (mut transparent_phase, visible_entities, view, tonemapping, dither) in &mut views {
            let mut view_key = SpritePipelineKey::from_hdr(view.hdr) | msaa_key;

            if !view.hdr {
                if let Some(tonemapping) = tonemapping {
                    view_key |= SpritePipelineKey::TONEMAP_IN_SHADER;
                    view_key |= match tonemapping {
                        Tonemapping::None => SpritePipelineKey::TONEMAP_METHOD_NONE,
                        Tonemapping::Reinhard => SpritePipelineKey::TONEMAP_METHOD_REINHARD,
                        Tonemapping::ReinhardLuminance => {
                            SpritePipelineKey::TONEMAP_METHOD_REINHARD_LUMINANCE
                        }
                        Tonemapping::AcesFitted => SpritePipelineKey::TONEMAP_METHOD_ACES_FITTED,
                        Tonemapping::AgX => SpritePipelineKey::TONEMAP_METHOD_AGX,
                        Tonemapping::SomewhatBoringDisplayTransform => {
                            SpritePipelineKey::TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM
                        }
                        Tonemapping::TonyMcMapface => {
                            SpritePipelineKey::TONEMAP_METHOD_TONY_MC_MAPFACE
                        }
                        Tonemapping::BlenderFilmic => {
                            SpritePipelineKey::TONEMAP_METHOD_BLENDER_FILMIC
                        }
                    };
                }
                if let Some(DebandDither::Enabled) = dither {
                    view_key |= SpritePipelineKey::DEBAND_DITHER;
                }
            }

            let pipeline = pipelines.specialize(&pipeline_cache, &sprite_pipeline, view_key);

            view_entities.clear();
            view_entities.extend(visible_entities.entities.iter().map(|e| e.index() as usize));
            transparent_phase.items.reserve(extracted_sprites.len());

            // Impossible starting values that will be replaced on the first iteration
            let mut current_batch = SpriteBatch {
                image_handle_id: HandleId::Id(Uuid::nil(), u64::MAX),
            };
            let mut current_batch_entity = Entity::PLACEHOLDER;
            let mut current_image_size = Vec2::ZERO;
            // Add a phase item for each sprite, and detect when successive items can be batched.
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
                };
                if new_batch != current_batch {
                    // Set-up a new possible batch
                    if let Some(premultiplied_image) = premultiplied_images
                        .values
                        .get(&Handle::weak(new_batch.image_handle_id))
                    {
                        current_batch = new_batch;
                        current_image_size =
                            Vec2::new(premultiplied_image.size.x, premultiplied_image.size.y);
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
                                                &premultiplied_image.texture_view,
                                            ),
                                        },
                                        BindGroupEntry {
                                            binding: 1,
                                            resource: BindingResource::Sampler(
                                                &premultiplied_image.sampler,
                                            ),
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

                let mut uv = QUAD_UV;

                // By default, the size of the quad is the size of the texture
                let mut quad_size = current_image_size;

                // If a rect is specified, adjust UVs and the size of the quad
                if let Some(rect) = extracted_sprite.rect {
                    let rect_size = rect.size();
                    let uv_min = rect.min / current_image_size;
                    let uv_max = rect.max / current_image_size;
                    let uv_size = rect_size / current_image_size;
                    uv = Vec4::new(uv_max.x, uv_min.y, -uv_size.x, uv_size.y);
                    quad_size = rect_size;
                }

                if extracted_sprite.flip_x {
                    uv.x += uv.z;
                    uv.z = -uv.z;
                }
                if extracted_sprite.flip_y {
                    uv.y += uv.w;
                    uv.w = -uv.w;
                }

                // Override the size if a custom one is specified
                if let Some(custom_size) = extracted_sprite.custom_size {
                    quad_size = custom_size;
                }

                let depth =
                    if let Some(object) = extracted_objects.objects.get(&extracted_sprite.entity) {
                        extracted_sprite.transform.translation().z / 16.
                            + (object.z_layer + 4) as f32 * 1000. / 16.
                    } else {
                        extracted_sprite.transform.translation().z / 16.
                    };

                // These items will be sorted by depth with other phase items
                let sort_key = FloatOrd(depth);

                // Compute the transformation matrix of the item
                let transform_matrix = extracted_sprite.transform.compute_matrix();

                // Store the vertex data and add the item to the render phase
                let mut instance_color = extracted_sprite.color.as_linear_rgba_f32();

                // Premultiply color
                instance_color = [
                    instance_color[0] * instance_color[3],
                    instance_color[1] * instance_color[3],
                    instance_color[2] * instance_color[3],
                    instance_color[3],
                ];

                // Additive blending
                if let Some(extracted_object) =
                    extracted_objects.objects.get(&extracted_sprite.entity)
                {
                    if extracted_object.blending {
                        instance_color[3] = 0.;
                    }
                }

                sprite_meta.instances.push(SpriteInstance {
                    anchor: extracted_sprite.anchor.to_array(),
                    half_extents: (quad_size / 2.).to_array(),
                    uv: uv.to_array(),
                    transform_1: transform_matrix.x_axis.xyz().to_array(),
                    transform_2: transform_matrix.y_axis.xyz().to_array(),
                    transform_3: transform_matrix.z_axis.xyz().to_array(),
                    transform_4: transform_matrix.w_axis.xyz().to_array(),
                    color: instance_color,
                });

                let item_start = index;
                index += 1;
                let item_end = index;

                transparent_phase.add(Transparent2d {
                    draw_function: draw_sprite_function,
                    pipeline,
                    entity: current_batch_entity,
                    sort_key,
                    batch_range: Some(item_start..item_end),
                });
            }
        }
        sprite_meta
            .instances
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
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetSpriteViewBindGroup<I> {
    type Param = SRes<SpriteMeta>;
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
pub struct SetSpriteTextureBindGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetSpriteTextureBindGroup<I> {
    type Param = SRes<ImageBindGroups>;
    type ViewWorldQuery = ();
    type ItemWorldQuery = Read<SpriteBatch>;

    fn render<'w>(
        _item: &P,
        _view: (),
        sprite_batch: &'_ SpriteBatch,
        image_bind_groups: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
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
    type Param = SRes<SpriteMeta>;
    type ViewWorldQuery = ();
    type ItemWorldQuery = ();

    fn render<'w>(
        item: &P,
        _view: (),
        _entity: (),
        sprite_meta: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let sprite_meta = sprite_meta.into_inner();
        pass.set_vertex_buffer(0, sprite_meta.instances.buffer().unwrap().slice(..));
        pass.draw(1..7, item.batch_range().as_ref().unwrap().clone());
        RenderCommandResult::Success
    }
}
