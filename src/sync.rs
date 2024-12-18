use std::hash::Hash;

use bevy::{
    core::FrameCount,
    ecs::{
        component::{ComponentId, ComponentInfo},
        entity::EntityHashMap,
        event::EventCursor,
    },
    prelude::*,
    reflect::{serde::ReflectSerializer, ReflectFromPtr},
    utils::{AHasher, HashMap},
};
use rerun::external::re_log::ResultExt;

use crate::{
    compute_entity_path, get_component_logger, DefaultRerunComponentLoggers, RerunComponentLoggers,
};

// ---

#[derive(Resource)]
struct RerunSyncState {
    /// Where to publish the data?
    pub rec: rerun::RecordingStream,

    /// Keeps track of alive entities so we can clear those that get despawned.
    pub entities: EntityHashMap<rerun::EntityPath>,
}

/// A plugin to sync the state of the Bevy database and the Rerun database.
pub struct RerunSyncPlugin {
    pub rec: rerun::RecordingStream,
}

impl Plugin for RerunSyncPlugin {
    fn build(&self, app: &mut App) {
        self.rec
            .log_static("world", &rerun::ViewCoordinates::RIGHT_HAND_Y_UP)
            .ok_or_log_error();

        let state = RerunSyncState {
            rec: self.rec.clone(),
            entities: Default::default(),
        };

        app.init_resource::<DefaultRerunComponentLoggers>()
            .insert_resource(state)
            .add_systems(Last, system_sync_entities);
    }
}

// ---

// TODO(cmc): some multithreading parallel iterators for the whole sync wouldn't hurt.

fn system_sync_entities(world: &mut World) {
    let _trace = info_span!("sync_entities").entered();

    let state = world.resource::<RerunSyncState>();
    let rec = state.rec.clone();

    // TODO(cmc): we should be subscribing to hierarchy event in order to clear old entity paths as
    // their hierarchy changes (and thus their path).

    let mut previous_entities = state.entities.clone();
    let mut current_entities = EntityHashMap::<rerun::EntityPath>::default();
    {
        set_recording_time(world, &rec);
        sync_components(world, &mut current_entities, &mut previous_entities, &rec);
        clear_despawned_entities(previous_entities, &rec);
    }

    let mut state = world.resource_mut::<RerunSyncState>();
    state.entities = current_entities;
}

/// Synchronize Bevy's clock with the recording's clock.
fn set_recording_time(world: &World, rec: &rerun::RecordingStream) {
    let _trace = info_span!("set_recording_time").entered();

    let time = world.resource::<Time>();
    let elapsed = time.elapsed_secs_f64();

    let tick = world.resource::<FrameCount>();
    let frame = tick.0;

    rec.set_time_seconds("sim_time", elapsed);
    // TODO(cmc): i'll log it once i can tell the blueprint to default to `sim_time`.
    // rec.set_time_sequence("sim_frame", frame);
    _ = frame;
}

// TODO(cmc): implement proper subscription model for asset dependencies
const DEPENDS_ON_IMAGES: &[&str] = &[
    "bevy_render::mesh::components::Mesh3d",
    "bevy_sprite::sprite::Sprite",
];
const DEPENDS_ON_MESHES: &[&str] = &[
    "bevy_render::mesh::components::Mesh3d", //
];
const DEPENDS_ON_STDMATS: &[&str] = &[
    "bevy_render::mesh::components::Mesh3d", //
    "bevy_render::primitives::Aabb",         //
];
const DEPENDS_ON_COLMATS: &[&str] = &[
    "bevy_render::mesh::components::Mesh3d", //
    "bevy_render::primitives::Aabb",         //
];

/// Synchronize the Bevy and Rerun database by logging all components appropriately.
//
// TODO(cmc): obviously, iterating the world (literally, btw) is not a viable strategy.
fn sync_components(
    world: &mut World,
    current_entities: &mut EntityHashMap<rerun::EntityPath>,
    previous_entities: &mut EntityHashMap<rerun::EntityPath>,
    rec: &rerun::RecordingStream,
) {
    let now = std::time::Instant::now();

    let _trace = info_span!("sync_components").entered();

    let mut all_entities = world.query::<(Entity, Option<&Parent>, Option<&Name>)>();
    all_entities.update_archetypes(world);

    // TODO(cmc): do this the smart way
    fn collect_events<A: Asset>(world: &mut World) -> Vec<AssetEvent<A>> {
        let events = world.resource_mut::<Events<AssetEvent<A>>>();
        let mut cursor = EventCursor::<AssetEvent<A>>::default();
        cursor.read(&events).copied().collect()
    }
    let image_events = collect_events::<Image>(world);
    let mesh_events = collect_events::<Mesh>(world);
    let stdmat_events = collect_events::<StandardMaterial>(world);
    let colmat_events = collect_events::<ColorMaterial>(world);

    // TODO(cmc): no good reason to clone this every time
    let loggers = world.get_resource::<RerunComponentLoggers>().cloned();
    let default_loggers = world.resource::<DefaultRerunComponentLoggers>().clone();

    let mut deferred_hash_updates = Vec::new();

    let mut entities = world.query::<Entity>();
    for entity_id in entities.iter(world) {
        // TODO(cmc): should cache this and deal with `HierarchyEvent` accordingly.
        let entity_path = compute_entity_path(world, &all_entities, entity_id);

        current_entities.insert(entity_id, entity_path.clone());
        previous_entities.remove(&entity_id);

        let entity = world.entity(entity_id);

        let change_tick = world.read_change_tick();
        let last_change_tick = world.last_change_tick();

        let mut current_hashes = CurrentHashes::default();
        let empty_hashes = CurrentHashes::default();
        let last_hashes = world
            .entity(entity_id)
            .get::<CurrentHashes>()
            .unwrap_or(&empty_hashes);

        let mut as_components: HashMap<Option<&'static str>, Vec<Box<dyn rerun::AsComponents>>> =
            Default::default();
        for component in world.inspect_entity(entity_id) {
            let mut has_changed = entity
                .get_change_ticks_by_id(component.id())
                .map_or(false, |changes| {
                    changes.is_changed(last_change_tick, change_tick)
                });

            // TODO(cmc): implement proper subscription model for asset dependencies
            has_changed |=
                !image_events.is_empty() && DEPENDS_ON_IMAGES.contains(&component.name());
            has_changed |= !mesh_events.is_empty() && DEPENDS_ON_MESHES.contains(&component.name());
            has_changed |=
                !stdmat_events.is_empty() && DEPENDS_ON_STDMATS.contains(&component.name());
            has_changed |=
                !colmat_events.is_empty() && DEPENDS_ON_COLMATS.contains(&component.name());

            if !has_changed {
                continue;
            }

            {
                // NOTE: Default the hash to 0, that way `<missing reflection data>` will be mapped
                // to 0 and will be logged only once rather than every frame.
                let component_hash = component_to_hash(world, entity, component).unwrap_or(0u64);
                current_hashes.insert(component.id(), component_hash);
                if last_hashes.get(&component.id()) == Some(&component_hash) {
                    continue;
                }
            }

            if let Some(logger) =
                get_component_logger(component, loggers.as_ref(), &default_loggers)
            {
                let (suffix, data) = logger(world, &all_entities, entity, component);
                as_components.entry(suffix).or_default().extend(data);
            }
        }

        if !current_hashes.is_empty() {
            deferred_hash_updates.push((entity_id, current_hashes));
        }

        let mut current_components = HashMap::default();

        // TODO(cmc): lots of inneficiencies and awkward collections that are forced upon us
        // because of how the RecordingStream API is designed.
        // After quite a bit of juggling it's not too bad though.
        for (suffix, as_components) in as_components {
            let component_batches = as_components
                .iter()
                .map(|data| data.as_component_batches())
                .collect::<Vec<_>>();

            let entity_path: rerun::EntityPath = suffix.map_or_else(
                || entity_path.clone(),
                // NOTE(cmc): The extra `comps/` is crucial so that we can easily clear everything
                // (we need a recursive clear but not really)
                |suffix| entity_path.join(&"comps".into()).join(&suffix.into()),
            );

            for batches in &component_batches {
                for batch in batches {
                    current_components.insert(
                        rerun::ComponentBatch::descriptor(batch).into_owned(),
                        entity_path.clone(),
                    );
                }
            }

            rec.log_component_batches(
                entity_path,
                false,
                component_batches
                    .iter()
                    .flatten()
                    .map(|batch| batch as &dyn rerun::ComponentBatch),
            )
            .ok_or_log_error();
        }

        let empty_components = CurrentComponents::default();
        let last_components = entity
            .get::<CurrentComponents>()
            .unwrap_or(&empty_components);

        for (component_desc, entity_path) in last_components.iter() {
            if !current_components.contains_key(component_desc) {
                rec.log(entity_path.clone(), &rerun::Clear::flat())
                    .ok_or_log_error();
            }
        }
    }

    for (entity_id, hashes) in deferred_hash_updates {
        world.entity_mut(entity_id).insert(hashes);
    }

    trace!(elapsed=?now.elapsed(), "component sync done");
}

fn clear_despawned_entities(
    previous_entities: EntityHashMap<rerun::EntityPath>,
    rec: &rerun::RecordingStream,
) {
    let _trace = info_span!("clear_despawned_entities").entered();

    for (_entity, entity_path) in previous_entities {
        rec.log(
            entity_path.join(&"comps".into()),
            &rerun::Clear::recursive(),
        )
        .ok_or_log_error();

        rec.log(entity_path, &rerun::Clear::flat())
            .ok_or_log_error();
    }
}

// ---

fn component_to_hash(
    world: &World,
    entity: EntityRef<'_>,
    component: &ComponentInfo,
) -> Option<u64> {
    let type_registry = world.resource::<AppTypeRegistry>();
    let type_registry = type_registry.read();

    component
        .type_id()
        .and_then(|tid| type_registry.get(tid))
        .and_then(|ty| ty.data::<ReflectFromPtr>())
        .and_then(|reflect_from_ptr| {
            #[allow(unsafe_code)]
            let reflected = entity
                .get_by_id(component.id())
                // Safety: the type registry cannot be wrong, surely
                .map(|ptr| unsafe { reflect_from_ptr.as_reflect(ptr) });

            // TODO(cmc): `Reflect::reflect_hash` is basically never available so we go the long way
            // instead... this is likely waaay too costly in practice :)
            reflected.ok().and_then(|reflected| {
                let serializer =
                    ReflectSerializer::new(reflected.as_partial_reflect(), &type_registry);
                let mut bytes = Vec::<u8>::new();
                ron::ser::to_writer(&mut bytes, &serializer).ok()?;

                use std::hash::Hasher;
                let mut hasher = AHasher::default();
                bytes.hash(&mut hasher);
                Some(hasher.finish())
            })
        })
}

/// Used to deduplicate changes to components that don't actually change anything.
//
// TODO(cmc): we desperately need to be able to filter noise in the timeline panel.
#[derive(Component, Debug, Clone, Default, Deref, DerefMut)]
struct CurrentHashes(HashMap<ComponentId, u64>);

/// Keeps track of all components on an entity in order to `Clear` removed ones.
#[derive(Component, Debug, Clone, Default, Deref, DerefMut)]
struct CurrentComponents(HashMap<rerun::ComponentDescriptor, rerun::EntityPath>);
