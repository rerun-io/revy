use bevy::{
    ecs::component::ComponentInfo,
    prelude::*,
    render::{mesh::PlaneMeshBuilder, primitives::Aabb},
};

use rerun::{AsComponents as _, ComponentBatch, external::nohash_hasher::IntMap};

use crate::{RerunLogger, ToRerun, compute_entity_path};

// ---

/// The default [`RerunLogger`]s that are used if no user-defined logger is specified.
///
/// See [`crate::RerunComponentLoggers`] for more information.
///
/// Public so end users can easily inspect what is configured by default.
#[derive(Resource, Deref, DerefMut, Clone, Debug)]
pub struct DefaultRerunComponentLoggers(IntMap<rerun::ComponentType, Option<RerunLogger>>);

// TODO(cmc): DataUi being typed makes aliases uninspectable :(
#[allow(clippy::too_many_lines)]
impl Default for DefaultRerunComponentLoggers {
    fn default() -> Self {
        let mut loggers = IntMap::default();

        loggers.insert(
            "bevy_transform::components::transform::Transform".into(),
            Some(RerunLogger::new_static(&bevy_transform)),
        );
        loggers.insert(
            "bevy_transform::components::global_transform::GlobalTransform".into(),
            Some(RerunLogger::new_static(&bevy_global_transform)),
        );

        loggers.insert(
            "bevy_render::mesh::components::Mesh2d".into(),
            Some(RerunLogger::new_static(&bevy_mesh2d)),
        );
        loggers.insert(
            "bevy_render::mesh::components::Mesh3d".into(),
            Some(RerunLogger::new_static(&bevy_mesh3d)),
        );

        loggers.insert(
            "bevy_render::camera::projection::Projection".into(),
            Some(RerunLogger::new_static(&bevy_projection)),
        );
        loggers.insert(
            "bevy_render::camera::projection::OrthographicProjection".into(),
            Some(RerunLogger::new_static(&bevy_projection_orthographic)),
        );
        loggers.insert(
            "bevy_render::camera::projection::PerspectiveProjection".into(),
            Some(RerunLogger::new_static(&bevy_projection_perspective)),
        );

        loggers.insert(
            "bevy_sprite::sprite::Sprite".into(),
            Some(RerunLogger::new_static(&bevy_sprite)),
        );

        loggers.insert(
            "bevy_render::primitives::Aabb".into(),
            Some(RerunLogger::new_static(&bevy_aabb)),
        );

        loggers.insert(
            "bevy_hierarchy::components::parent::Parent".into(),
            Some(RerunLogger::new_static(&bevy_parent)),
        );
        loggers.insert(
            "bevy_hierarchy::components::children::Children".into(),
            Some(RerunLogger::new_static(&bevy_children)),
        );

        loggers.insert("revy::entity_path::RerunEntityPath".into(), None);

        Self(loggers)
    }
}

// ---

// TODO(cmc): all those aliasing reshenanigans should really just be custom archetype names in
// the descriptor, but the viewer won't be ready for that in 0.22.

fn bevy_transform<'w>(
    _world: &'w World,
    _all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    _component: &'w ComponentInfo,
) -> (Option<&'static str>, Vec<rerun::SerializedComponentBatch>) {
    (
        None,
        entity
            .get::<Transform>()
            .into_iter()
            .flat_map(|transform| transform.to_rerun().as_serialized_batches())
            .collect(),
    )
}

fn bevy_global_transform<'w>(
    _world: &'w World,
    _all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    _component: &'w ComponentInfo,
) -> (Option<&'static str>, Vec<rerun::SerializedComponentBatch>) {
    let suffix = None;
    let data = entity
        .get::<GlobalTransform>()
        .into_iter()
        .flat_map(|transform| {
            transform
                .to_rerun()
                .as_serialized_batches()
                .into_iter()
                .map(|batch| {
                    let archetype_name = "bevy.GlobalTransform3D";
                    let component = batch.descriptor.component;
                    let component = component.as_str().replace("Transform3D", archetype_name);

                    let descriptor = rerun::ComponentDescriptor {
                        archetype: Some(archetype_name.into()),
                        component: rerun::ComponentIdentifier::try_new(component)
                            .expect("component name is never empty"),
                        component_type: batch.descriptor.component_type,
                    };
                    batch.with_descriptor_override(descriptor)
                })
        })
        .collect();

    (suffix, data)
}

fn bevy_mesh<'w>(
    world: &'w World,
    _all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    _component: &'w ComponentInfo,
    handle: Option<&Handle<Mesh>>,
) -> (Option<&'static str>, Vec<rerun::SerializedComponentBatch>) {
    let suffix = None;
    let batches = handle
        .and_then(|handle| world.resource::<Assets<Mesh>>().get(handle))
        .and_then(ToRerun::to_rerun)
        .into_iter()
        .flat_map(|mut mesh| {
            if let Some(mat) = entity
                .get::<MeshMaterial2d<ColorMaterial>>()
                .and_then(|handle| world.resource::<Assets<ColorMaterial>>().get(handle))
            {
                mesh = mesh.with_albedo_factor(mat.color.to_rerun());
            }
            if let Some(mat) = entity
                .get::<MeshMaterial3d<StandardMaterial>>()
                .and_then(|handle| world.resource::<Assets<StandardMaterial>>().get(handle))
            {
                mesh = mesh.with_albedo_factor(mat.base_color.to_rerun());

                if let Some((image_format, image_data)) = mat
                    .base_color_texture
                    .as_ref()
                    .and_then(|handle| world.resource::<Assets<Image>>().get(handle))
                    .and_then(ToRerun::to_rerun)
                {
                    mesh = mesh.with_albedo_texture(image_format, image_data);
                }
            }
            mesh.as_serialized_batches()
        })
        .collect();
    (suffix, batches)
}

fn bevy_mesh2d<'w>(
    world: &'w World,
    all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    component: &'w ComponentInfo,
) -> (Option<&'static str>, Vec<rerun::SerializedComponentBatch>) {
    let suffix = Some("mesh2d");
    let (_, batches) = bevy_mesh(
        world,
        all_entities,
        entity,
        component,
        entity.get::<Mesh2d>().map(|handle| &handle.0),
    );
    (suffix, batches)
}

fn bevy_mesh3d<'w>(
    world: &'w World,
    all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    component: &'w ComponentInfo,
) -> (Option<&'static str>, Vec<rerun::SerializedComponentBatch>) {
    let suffix = Some("mesh3d");
    let (_, batches) = bevy_mesh(
        world,
        all_entities,
        entity,
        component,
        entity.get::<Mesh3d>().map(|handle| &handle.0),
    );
    (suffix, batches)
}

fn bevy_camera<'w, C: Component + ToRerun<rerun::Pinhole>>(
    _world: &'w World,
    _all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    _component: &'w ComponentInfo,
) -> (Option<&'static str>, Vec<rerun::SerializedComponentBatch>) {
    let suffix = Some("cam");
    let batches = entity
        .get::<C>()
        // TODO(cmc): log visible entities too?
        .into_iter()
        .flat_map(|mesh| mesh.to_rerun().as_serialized_batches())
        .collect();
    (suffix, batches)
}

fn bevy_projection<'w>(
    world: &'w World,
    all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    component: &'w ComponentInfo,
) -> (Option<&'static str>, Vec<rerun::SerializedComponentBatch>) {
    bevy_camera::<Projection>(world, all_entities, entity, component)
}

fn bevy_projection_orthographic<'w>(
    world: &'w World,
    all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    component: &'w ComponentInfo,
) -> (Option<&'static str>, Vec<rerun::SerializedComponentBatch>) {
    bevy_camera::<OrthographicProjection>(world, all_entities, entity, component)
}

fn bevy_projection_perspective<'w>(
    world: &'w World,
    all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    component: &'w ComponentInfo,
) -> (Option<&'static str>, Vec<rerun::SerializedComponentBatch>) {
    bevy_camera::<PerspectiveProjection>(world, all_entities, entity, component)
}

// TODO(cmc): check if sprite has custom sizes etc
fn bevy_sprite<'w>(
    world: &'w World,
    _all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    _component: &'w ComponentInfo,
) -> (Option<&'static str>, Vec<rerun::SerializedComponentBatch>) {
    let suffix = Some("sprite");
    let batches = entity
        .get::<Sprite>()
        .and_then(|sprite| {
            world
                .resource::<Assets<Image>>()
                .get(sprite.image.id())
                .and_then(ToRerun::to_rerun)
                .and_then(|(image_format, image_data)| {
                    let mesh = PlaneMeshBuilder::default()
                        .normal(Dir3::Z)
                        .size(image_format.width as _, image_format.height as _)
                        .build();
                    mesh.to_rerun().map(|mesh| {
                        mesh.with_albedo_factor(sprite.color.to_rerun())
                            .with_albedo_texture(image_format, image_data)
                    })
                })
        })
        .into_iter()
        .flat_map(|mesh| mesh.as_serialized_batches())
        .collect();
    (suffix, batches)
}

fn bevy_aabb<'w>(
    world: &'w World,
    _all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    _component: &'w ComponentInfo,
) -> (Option<&'static str>, Vec<rerun::SerializedComponentBatch>) {
    let suffix = Some("aabb");
    let batches = entity
        .get::<Aabb>()
        .map(|aabb| {
            rerun::Boxes3D::from_centers_and_half_sizes(
                [aabb.center.to_rerun()],
                [aabb.half_extents.to_rerun()],
            )
        })
        .map(|aabb| {
            if let Some(mat) = entity
                .get::<MeshMaterial2d<ColorMaterial>>()
                .and_then(|handle| world.resource::<Assets<ColorMaterial>>().get(handle))
            {
                aabb.with_colors([mat.color.to_rerun()])
            } else if let Some(mat) = entity
                .get::<MeshMaterial3d<StandardMaterial>>()
                .and_then(|handle| world.resource::<Assets<StandardMaterial>>().get(handle))
            {
                aabb.with_colors([mat.base_color.to_rerun()])
            } else if let Some(sprite) = entity.get::<Sprite>() {
                aabb.with_colors([sprite.color.to_rerun()])
            } else {
                aabb
            }
        })
        .into_iter()
        .flat_map(|aabb| aabb.as_serialized_batches())
        .collect();
    (suffix, batches)
}

fn bevy_parent<'w>(
    world: &'w World,
    all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    _component: &'w ComponentInfo,
) -> (Option<&'static str>, Vec<rerun::SerializedComponentBatch>) {
    let suffix = None;
    let batches = entity
        .get::<Parent>()
        .and_then(|parent| {
            let parent_entity_path = compute_entity_path(world, all_entities, parent.get());
            rerun::components::EntityPath(parent_entity_path.to_string().into())
                .serialized(rerun::ComponentDescriptor::partial("Parent"))
        })
        .into_iter()
        .collect();
    (suffix, batches)
}

fn bevy_children<'w>(
    world: &'w World,
    all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    _component: &'w ComponentInfo,
) -> (Option<&'static str>, Vec<rerun::SerializedComponentBatch>) {
    let suffix = None;
    let batches = entity
        .get::<Children>()
        .and_then(|children| {
            let children = children
                .iter()
                .map(|entity_id| {
                    rerun::components::EntityPath(
                        compute_entity_path(world, all_entities, *entity_id)
                            .to_string()
                            .into(),
                    )
                })
                .collect::<Vec<_>>();
            children.serialized(rerun::ComponentDescriptor::partial("Children"))
        })
        .into_iter()
        .collect();
    (suffix, batches)
}
