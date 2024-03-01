use bevy::prelude::*;

// ---

/// Iterates over the ancestors of `entity_id`, in ascending order (parent, grand-parent, grand-grand-parent, …).
///
/// `entities` must have been updated manually before calling this function, or the results will be
/// out-of-date.
pub fn ancestors_from_world<'w: 's, 's>(
    world: &'w World,
    entities: &'w QueryState<(Entity, Option<&'s Parent>, Option<&'s Name>)>,
    entity_id: Entity,
) -> impl Iterator<Item = Entity> + 'w {
    let mut current_entity_id = entity_id;
    std::iter::from_fn(move || {
        #[allow(clippy::collapsible_match)]
        if let Ok((_, parent, _)) = entities.get_manual(world, current_entity_id) {
            if let Some(parent) = parent {
                current_entity_id = **parent;
                return Some(**parent);
            }
        }
        None
    })
}

/// Computes the [`rerun::EntityPath`] of the specified `entity_id`.
///
/// The entity path is hierarchy dependent: if the target entity's parent change, the next call to
/// this function will yield a different result.
///
/// `entities` must have been updated manually before calling this function, or the results will be
/// out-of-date.
pub fn compute_entity_path<'w: 's, 's>(
    world: &'w World,
    entities: &'w QueryState<(Entity, Option<&'s Parent>, Option<&'s Name>)>,
    entity_id: Entity,
) -> rerun::EntityPath {
    // TODO(cmc): kinda awkward that we have to prefix `world/` everywhere or hell ensues.
    std::iter::once(rerun::EntityPathPart::new("world"))
        .chain(
            std::iter::once(entity_id)
                .chain(ancestors_from_world(world, entities, entity_id))
                .map(|entity_id| {
                    rerun::EntityPathPart::new({
                        entities
                            .get_manual(world, entity_id)
                            .ok()
                            .and_then(|(_, _, name)| name)
                            .map_or_else(
                                || format!("{entity_id:?}"),
                                |name| format!("{entity_id:?}_{name}"),
                            )
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .rev(),
        )
        .collect()
}
