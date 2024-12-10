use bevy::{
    ecs::component::ComponentInfo,
    prelude::*,
    render::{mesh::PlaneMeshBuilder, primitives::Aabb},
    utils::HashMap,
};

use crate::{compute_entity_path, rerun_logger::ManyAsComponents, Aliased, RerunLogger, ToRerun};

// ---

/// The default [`RerunLogger`]s that are used if no user-defined logger is specified.
///
/// See [`crate::RerunComponentLoggers`] for more information.
///
/// Public so end users can easily inspect what is configured by default.
#[derive(Resource, Deref, DerefMut, Clone, Debug)]
pub struct DefaultRerunComponentLoggers(HashMap<rerun::ComponentName, Option<RerunLogger>>);

// TODO(cmc): DataUi being typed makes aliases uninspectable :(
#[allow(clippy::too_many_lines)]
impl Default for DefaultRerunComponentLoggers {
    fn default() -> Self {
        let mut loggers = HashMap::default();

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

fn bevy_transform<'w>(
    _world: &'w World,
    _all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    _component: &'w ComponentInfo,
) -> (Option<&'static str>, Option<Box<dyn rerun::AsComponents>>) {
    let suffix = None;

    let data = entity
        .get::<Transform>()
        .map(|transform| transform.to_rerun())
        .map(|data| Box::new(data) as _);

    (suffix, data)
}

fn bevy_global_transform<'w>(
    _world: &'w World,
    _all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    _component: &'w ComponentInfo,
) -> (Option<&'static str>, Option<Box<dyn rerun::AsComponents>>) {
    let suffix = None;

    // TODO(cmc): once again the DataUi does the wrong thing... we really need to
    // go typeless.
    let data = entity.get::<GlobalTransform>().map(|transform| {
        Box::new(ManyAsComponents(vec![
            Box::new(Aliased::<rerun::datatypes::Vec3D>::new(
                "GlobalTransform3D.translation",
                transform.translation().to_rerun(),
            )),
            Box::new(Aliased::<rerun::datatypes::Quaternion>::new(
                "GlobalTransform3D.rotation",
                transform.rotation().to_rerun(),
            )),
            Box::new(Aliased::<rerun::datatypes::Vec3D>::new(
                "GlobalTransform3D.scale",
                transform.scale().to_rerun(),
            )),
        ])) as _
    });

    (suffix, data)
}

fn bevy_mesh<'w>(
    world: &'w World,
    _all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    _component: &'w ComponentInfo,
    handle: Option<&Handle<Mesh>>,
) -> (Option<&'static str>, Option<Box<dyn rerun::AsComponents>>) {
    let suffix: Option<&str> = None;

    let data = handle
        .and_then(|handle| world.resource::<Assets<Mesh>>().get(handle))
        .and_then(ToRerun::to_rerun)
        .map(|mut mesh| {
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
                    mesh = mesh.with_albedo_texture(image_format, image_data)
                }
            }
            mesh
        })
        .map(|mesh| Box::new(mesh) as _);

    (suffix, data)
}

fn bevy_mesh2d<'w>(
    world: &'w World,
    all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    component: &'w ComponentInfo,
) -> (Option<&'static str>, Option<Box<dyn rerun::AsComponents>>) {
    let suffix = Some("mesh2d");
    let (_, data) = bevy_mesh(
        world,
        all_entities,
        entity,
        component,
        entity.get::<Mesh2d>().map(|handle| &handle.0),
    );
    (suffix, data)
}

fn bevy_mesh3d<'w>(
    world: &'w World,
    all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    component: &'w ComponentInfo,
) -> (Option<&'static str>, Option<Box<dyn rerun::AsComponents>>) {
    let suffix = Some("mesh3d");
    let (_, data) = bevy_mesh(
        world,
        all_entities,
        entity,
        component,
        entity.get::<Mesh3d>().map(|handle| &handle.0),
    );
    (suffix, data)
}

fn bevy_camera<'w, C: Component + ToRerun<rerun::Pinhole>>(
    _world: &'w World,
    _all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    _component: &'w ComponentInfo,
) -> (Option<&'static str>, Option<Box<dyn rerun::AsComponents>>) {
    let suffix = Some("cam");
    let data = entity
        .get::<C>()
        // TODO(cmc): log visible entities too?
        .map(ToRerun::to_rerun)
        .map(|mesh| Box::new(mesh) as _);
    (suffix, data)
}

fn bevy_projection<'w>(
    world: &'w World,
    all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    component: &'w ComponentInfo,
) -> (Option<&'static str>, Option<Box<dyn rerun::AsComponents>>) {
    bevy_camera::<Projection>(world, all_entities, entity, component)
}

fn bevy_projection_orthographic<'w>(
    world: &'w World,
    all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    component: &'w ComponentInfo,
) -> (Option<&'static str>, Option<Box<dyn rerun::AsComponents>>) {
    bevy_camera::<OrthographicProjection>(world, all_entities, entity, component)
}

fn bevy_projection_perspective<'w>(
    world: &'w World,
    all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    component: &'w ComponentInfo,
) -> (Option<&'static str>, Option<Box<dyn rerun::AsComponents>>) {
    bevy_camera::<PerspectiveProjection>(world, all_entities, entity, component)
}

// TODO(cmc): check if sprite has custom sizes etc
fn bevy_sprite<'w>(
    world: &'w World,
    _all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    _component: &'w ComponentInfo,
) -> (Option<&'static str>, Option<Box<dyn rerun::AsComponents>>) {
    let suffix = Some("sprite");

    let data = entity
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
        .map(|data| Box::new(data) as _);

    (suffix, data)
}

fn bevy_aabb<'w>(
    world: &'w World,
    _all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    _component: &'w ComponentInfo,
) -> (Option<&'static str>, Option<Box<dyn rerun::AsComponents>>) {
    let suffix = Some("aabb");
    let data = entity
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
        .map(|data| Box::new(data) as _);

    (suffix, data)
}

fn bevy_parent<'w>(
    world: &'w World,
    all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    _component: &'w ComponentInfo,
) -> (Option<&'static str>, Option<Box<dyn rerun::AsComponents>>) {
    let suffix = None;
    let data = entity
        .get::<Parent>()
        .map(|parent| {
            let parent_entity_path = compute_entity_path(world, all_entities, parent.get());
            Aliased::<rerun::datatypes::EntityPath>::new(
                "Parent",
                rerun::datatypes::EntityPath(parent_entity_path.to_string().into()),
            )
        })
        .map(|data| Box::new(data) as _);
    (suffix, data)
}

fn bevy_children<'w>(
    world: &'w World,
    all_entities: &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
    entity: EntityRef<'_>,
    _component: &'w ComponentInfo,
) -> (Option<&'static str>, Option<Box<dyn rerun::AsComponents>>) {
    let suffix = None;

    // TODO(cmc): it is once again super annoying that number of instances gets resolved at logging
    // time... we need those clamp-to-edge semantics asap.
    // let data = entity
    //     .get::<Children>()
    //     .map(|children| {
    //         let children = children
    //             .iter()
    //             .map(|entity_id| {
    //                 rerun::datatypes::EntityPath(
    //                     compute_entity_path(world, all_entities, *entity_id)
    //                         .to_string()
    //                         .into(),
    //                 )
    //             })
    //             .collect::<Vec<_>>();
    //         Aliased::<Vec<rerun::datatypes::EntityPath>>::new(
    //             "RawChildren",
    //             children,
    //         )
    //     })
    //     .map(|data| Box::new(data) as _);

    let data = entity
        .get::<Children>()
        .map(|children| {
            let children = children
                .iter()
                .map(|entity_id| compute_entity_path(world, all_entities, *entity_id).to_string())
                .collect::<Vec<_>>();
            Aliased::<rerun::components::Text>::new(
                "RawChildren",
                rerun::components::Text(children.join("\n").into()),
            )
        })
        .map(|data| Box::new(data) as _);
    (suffix, data)
}
