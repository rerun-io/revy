use std::sync::Arc;

use bevy::{
    ecs::component::ComponentInfo,
    prelude::*,
    reflect::{ReflectFromPtr, serde::ReflectSerializer},
    utils::HashMap,
};
use rerun::ComponentBatch;

use crate::DefaultRerunComponentLoggers;

// ---

// TODO(cmc): this should really work with component ids, although the API gotta uses names...
// but that means doing things (such as defaults) lazily since components are themselves registered
// lazily... and then it becomes a mess.

/// The callback type to create a [`RerunLogger`].
pub trait RerunLoggerFn:
    Send
    + Sync
    + for<'w> Fn(
        &'w World,
        &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
        EntityRef<'_>,
        &'w ComponentInfo,
    ) -> (Option<&'static str>, Vec<rerun::SerializedComponentBatch>)
{
}

impl<F> RerunLoggerFn for F where
    F: Send
        + Sync
        + for<'w> Fn(
            &'w World,
            &'w QueryState<(Entity, Option<&'w Parent>, Option<&'w Name>)>,
            EntityRef<'_>,
            &'w ComponentInfo,
        ) -> (Option<&'static str>, Vec<rerun::SerializedComponentBatch>)
{
}

#[derive(Clone)]
pub enum BoxedOrStaticRerunLogger {
    Boxed(Arc<dyn RerunLoggerFn>),
    Static(&'static dyn RerunLoggerFn),
}

impl std::ops::Deref for BoxedOrStaticRerunLogger {
    type Target = dyn RerunLoggerFn;

    #[inline]
    fn deref(&self) -> &Self::Target {
        match self {
            BoxedOrStaticRerunLogger::Boxed(f) => &**f,
            BoxedOrStaticRerunLogger::Static(f) => f,
        }
    }
}

/// An arbitrary callback to convert Bevy component data into Rerun component data.
#[derive(Resource, Deref, Clone)]
pub struct RerunLogger(BoxedOrStaticRerunLogger);

impl std::fmt::Debug for RerunLogger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("RerunLogger")
            .field(&format!("{:p}", &self.0) as _)
            .finish()
    }
}

impl RerunLogger {
    #[inline]
    pub fn new<F>(f: F) -> Self
    where
        F: RerunLoggerFn + 'static,
    {
        Self(BoxedOrStaticRerunLogger::Boxed(Arc::new(f) as _))
    }

    #[inline]
    pub const fn new_static(f: &'static dyn RerunLoggerFn) -> Self {
        Self(BoxedOrStaticRerunLogger::Static(f))
    }
}

// ---

/// Associate a [`RerunLogger`] with a fully-qualified component name.
///
/// E.g. log `"bevy_transform::components::transform::Transform"` as [`rerun::Transform3D`].
///
/// Use `None` to prevent the data from being logged entirely.
///
/// Don't set anything if you want to let the default logger to take over.
/// See [`crate::DefaultRerunComponentLoggers`] for more information.
///
/// If no default logger exists, the data will be logged as a [`rerun::TextDocument`].
#[derive(Resource, Deref, DerefMut, Clone)]
pub struct RerunComponentLoggers(pub HashMap<rerun::ComponentType, Option<RerunLogger>>);

impl RerunComponentLoggers {
    pub fn new(it: impl IntoIterator<Item = (rerun::ComponentType, Option<RerunLogger>)>) -> Self {
        Self(it.into_iter().collect())
    }
}

pub fn get_component_logger<'a>(
    component: &ComponentInfo,
    loggers: Option<&'a RerunComponentLoggers>,
    default_loggers: &'a DefaultRerunComponentLoggers,
) -> Option<&'a RerunLogger> {
    let component_name = rerun::ComponentType::from(component.name());

    if let Some(logger) = loggers.and_then(|loggers| {
        loggers
            .get(&component_name)
            .as_ref()
            .map(|logger| logger.as_ref())
    }) {
        return logger;
    }

    if let Some(logger) = default_loggers
        .get(&component_name)
        .as_ref()
        .map(|logger| logger.as_ref())
    {
        return logger;
    }

    #[allow(clippy::unnecessary_wraps)]
    fn log_ignored_component(
        world: &World,
        _all_entities: &QueryState<(Entity, Option<&Parent>, Option<&Name>)>,
        entity: EntityRef<'_>,
        component: &ComponentInfo,
    ) -> (Option<&'static str>, Vec<rerun::SerializedComponentBatch>) {
        let component_type_name = component.name();
        let parts = component_type_name.split("::").collect::<Vec<_>>();
        let (archetype_name, field_name): (String, String) = if parts.len() >= 2 {
            (
                parts[0..parts.len() - 1].join("."),
                parts.last().unwrap().to_string(),
            )
        } else {
            ("bevy".to_owned(), component_type_name.replace("::", "."))
        };

        let descriptor = rerun::ComponentDescriptor {
            archetype: Some(archetype_name.clone().into()),
            component: format!("{archetype_name}:{field_name}").into(),
            component_type: Some(component_type_name.into()),
        };

        let body = component_to_ron(world, entity, component)
            .unwrap_or_else(|| "<missing reflection metadata>".into());
        (
            None,
            rerun::components::Text(body.into())
                .serialized(descriptor)
                .into_iter()
                .collect(),
        )
    }

    static LOG_IGNORED_COMPONENT: RerunLogger = RerunLogger::new_static(&log_ignored_component);

    Some(&LOG_IGNORED_COMPONENT)
}

// TODO(cmc): why does this seem to fail for recursive types though? or is it something else?
fn component_to_ron(
    world: &World,
    entity: EntityRef<'_>,
    component: &ComponentInfo,
) -> Option<String> {
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

            reflected.ok().and_then(|reflected| {
                let serializer =
                    ReflectSerializer::new(reflected.as_partial_reflect(), &type_registry);
                ron::ser::to_string_pretty(&serializer, ron::ser::PrettyConfig::default()).ok()
            })
        })
}
