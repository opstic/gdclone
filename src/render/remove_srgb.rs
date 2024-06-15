use bevy::app::{App, Plugin};
use bevy::asset::{load_internal_asset, Handle};
use bevy::core_pipeline::core_2d::graph::{Core2d, Node2d};
use bevy::core_pipeline::fullscreen_vertex_shader::fullscreen_shader_vertex_state;
use bevy::ecs::query::QueryItem;
use bevy::prelude::{FromWorld, Resource, Shader, World};
use bevy::render::render_graph::RenderLabel;
use bevy::render::render_resource::binding_types::{sampler, texture_2d};
use bevy::render::render_resource::BindGroupLayoutEntries;
use bevy::render::{
    render_graph::{NodeRunError, RenderGraphApp, RenderGraphContext, ViewNode, ViewNodeRunner},
    render_resource::{
        BindGroupEntries, BindGroupLayout, CachedRenderPipelineId, ColorTargetState, ColorWrites,
        FragmentState, MultisampleState, Operations, PipelineCache, PrimitiveState,
        RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor, Sampler,
        SamplerBindingType, SamplerDescriptor, ShaderStages, TextureFormat, TextureSampleType,
    },
    renderer::{RenderContext, RenderDevice},
    texture::BevyDefault,
    view::ViewTarget,
    RenderApp,
};

pub const REMOVE_SRGB_SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(57293847162839275647);

pub(crate) struct RemoveSrgbPlugin;

impl Plugin for RemoveSrgbPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            REMOVE_SRGB_SHADER_HANDLE,
            "remove_srgb.wgsl",
            Shader::from_wgsl
        );

        // We need to get the render app from the main app
        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            // Bevy's renderer uses a render graph which is a collection of nodes in a directed acyclic graph.
            // It currently runs on each view/camera and executes each node in the specified order.
            // It will make sure that any node that needs a dependency from another node
            // only runs when that dependency is done.
            //
            // Each node can execute arbitrary work, but it generally runs at least one render pass.
            // A node only has access to the render world, so if you need data from the main world
            // you need to extract it manually or with the plugin like above.
            // Add a [`Node`] to the [`RenderGraph`]
            // The Node needs to impl FromWorld
            .add_render_graph_node::<ViewNodeRunner<RemoveSrgbNode>>(
                // Specify the name of the graph, in this case we want the graph for 2d
                Core2d,
                // It also needs the name of the node
                RemoveSrgbLabel,
            )
            .add_render_graph_edges(
                Core2d,
                // Specify the node ordering.
                // This will automatically create all required node edges to enforce the given ordering.
                (
                    Node2d::Tonemapping,
                    RemoveSrgbLabel,
                    Node2d::Fxaa,
                ),
            );
    }

    fn finish(&self, app: &mut App) {
        // We need to get the render app from the main app
        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            // Initialize the pipeline
            .init_resource::<RemoveSrgbPipeline>();
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct RemoveSrgbLabel;

/// The post process node used for the render graph
#[derive(Default)]
struct RemoveSrgbNode;

impl ViewNode for RemoveSrgbNode {
    // The node needs a query to gather data from the ECS in order to do its rendering,
    // but it's not a normal system so we need to define it manually.
    //
    // This query will only run on the view entity
    type ViewQuery = &'static ViewTarget;

    fn run(
        &self,
        _: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        view_target: QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        // Get the pipeline resource that contains the global data we need
        // to create the render pipeline
        let remove_srgb_pipeline = world.resource::<RemoveSrgbPipeline>();

        // The pipeline cache is a cache of all previously created pipelines.
        // It is required to avoid creating a new pipeline each frame,
        // which is expensive due to shader compilation.
        let pipeline_cache = world.resource::<PipelineCache>();

        // Get the pipeline from the cache
        let Some(pipeline) = pipeline_cache.get_render_pipeline(remove_srgb_pipeline.pipeline_id)
        else {
            return Ok(());
        };

        // This will start a new "post process write", obtaining two texture
        // views from the view target - a `source` and a `destination`.
        // `source` is the "current" main texture and you _must_ write into
        // `destination` because calling `post_process_write()` on the
        // [`ViewTarget`] will internally flip the [`ViewTarget`]'s main
        // texture to the `destination` texture. Failing to do so will cause
        // the current main texture information to be lost.
        let post_process = view_target.post_process_write();

        // The bind_group gets created each frame.
        //
        // Normally, you would create a bind_group in the Queue set,
        // but this doesn't work with the post_process_write().
        // The reason it doesn't work is because each post_process_write will alternate the source/destination.
        // The only way to have the correct source/destination for the bind_group
        // is to make sure you get it during the node execution.
        let bind_group = render_context.render_device().create_bind_group(
            "remove_srgb_bind_group",
            &remove_srgb_pipeline.layout,
            // It's important for this to match the BindGroupLayout defined in the PostProcessPipeline
            &BindGroupEntries::sequential((
                // Make sure to use the source view
                post_process.source,
                // Use the sampler created for the pipeline
                &remove_srgb_pipeline.sampler,
            )),
        );

        // Begin the render pass
        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("remove_srgb_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                // We need to specify the post process destination view here
                // to make sure we write to the appropriate texture.
                view: post_process.destination,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // This is mostly just wgpu boilerplate for drawing a fullscreen triangle,
        // using the pipeline/bind_group created above
        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

// This contains global data used by the render pipeline. This will be created once on startup.
#[derive(Resource)]
struct RemoveSrgbPipeline {
    layout: BindGroupLayout,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

impl FromWorld for RemoveSrgbPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        // We need to define the bind group layout used for our pipeline
        let layout = render_device.create_bind_group_layout(
            "remove_srgb_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                // The layout entries will only be visible in the fragment stage
                ShaderStages::FRAGMENT,
                (
                    // The screen texture
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    // The sampler that will be used to sample the screen texture
                    sampler(SamplerBindingType::Filtering),
                    // The settings uniform that will control the effect
                ),
            ),
        );

        // We can create the sampler here since it won't change at runtime and doesn't depend on the view
        let sampler = render_device.create_sampler(&SamplerDescriptor::default());

        let pipeline_id = world
            .resource_mut::<PipelineCache>()
            // This will add the pipeline to the cache and queue it's creation
            .queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("remove_srgb_pipeline".into()),
                layout: vec![layout.clone()],
                // This will setup a fullscreen triangle for the vertex state
                vertex: fullscreen_shader_vertex_state(),
                fragment: Some(FragmentState {
                    shader: REMOVE_SRGB_SHADER_HANDLE,
                    shader_defs: vec![],
                    // Make sure this matches the entry point of your shader.
                    // It can be anything as long as it matches here and in the shader.
                    entry_point: "fragment".into(),
                    targets: vec![Some(ColorTargetState {
                        format: TextureFormat::bevy_default(),
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                // All of the following properties are not important for this effect so just use the default values.
                // This struct doesn't have the Default trait implemented because not all field can have a default value.
                primitive: PrimitiveState::default(),
                depth_stencil: None,
                multisample: MultisampleState::default(),
                push_constant_ranges: vec![],
            });

        Self {
            layout,
            sampler,
            pipeline_id,
        }
    }
}
