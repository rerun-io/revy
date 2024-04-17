use std::sync::Arc;

use bevy::{
    ecs::component::ComponentInfo,
    prelude::*,
    reflect::{serde::ReflectSerializer, ReflectFromPtr},
    utils::HashMap,
};

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
    ) -> (Option<&'static str>, Option<Box<dyn rerun::AsComponents>>)
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
        ) -> (Option<&'static str>, Option<Box<dyn rerun::AsComponents>>)
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
/// See [`DefaultComponentLoggers`] for more information.
///
/// If no default logger exists, the data will be logged as a [`rerun::TextDocument`].
#[derive(Resource, Deref, DerefMut, Clone)]
pub struct RerunComponentLoggers(pub HashMap<rerun::ComponentName, Option<RerunLogger>>);

impl RerunComponentLoggers {
    pub fn new(it: impl IntoIterator<Item = (rerun::ComponentName, Option<RerunLogger>)>) -> Self {
        Self(it.into_iter().collect())
    }
}

pub fn get_component_logger<'a>(
    component: &ComponentInfo,
    loggers: Option<&'a RerunComponentLoggers>,
    default_loggers: &'a DefaultRerunComponentLoggers,
) -> Option<&'a RerunLogger> {
    let component_name = rerun::ComponentName::from(component.name());

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
    ) -> (Option<&'static str>, Option<Box<dyn rerun::AsComponents>>) {
        let name = component.name();
        let body = component_to_ron(world, entity, component)
            .unwrap_or_else(|| "<missing reflection metadata>".into());
        let reflected = Aliased::<rerun::components::Text>::new(name.replace("::", "."), body);

        (None, Some(Box::new(reflected) as _))
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

            reflected.and_then(|reflected| {
                let serializer = ReflectSerializer::new(reflected, &type_registry);
                ron::ser::to_string_pretty(&serializer, ron::ser::PrettyConfig::default()).ok()
            })
        })
}

// ---

// TODO(cmc): Rerun should provide tools for this.
// TODO(cmc): All this traits are very messy... CompomnentName vs. DatatypeName in particular is
// very annoying. Actually just Component vs. Datatype being different types in general is very
// annoying.
// TODO(cmc): the whole Loggable vs. LoggableBatch is also so messy

use rerun::external::{arrow2, re_types_core};

/// Helper to log any [`rerun::LoggableBatch`] as a [`rerun::Component`] with the specified name.
#[derive(Debug)]
pub struct Aliased<C: rerun::LoggableBatch> {
    name: rerun::ComponentName,
    data: C,

    field: arrow2::datatypes::Field,
}

impl<C: rerun::LoggableBatch> Aliased<C> {
    pub fn new(name: impl Into<rerun::ComponentName>, data: impl Into<C>) -> Self {
        let name = name.into();
        let data = data.into();

        // TODO(cmc): this horror should at least be cached... which is true of the code we
        // generate in the real SDK too
        let mut field = data.arrow_field();
        field.name = name.to_string();
        field.data_type = arrow2::datatypes::DataType::Extension(
            name.to_string(),
            Arc::new(field.data_type.to_logical_type().clone()),
            None,
        );

        Self { name, data, field }
    }
}

impl<C: rerun::LoggableBatch> rerun::AsComponents for Aliased<C> {
    #[inline]
    fn as_component_batches(&self) -> Vec<rerun::MaybeOwnedComponentBatch<'_>> {
        vec![rerun::MaybeOwnedComponentBatch::Ref(self)]
    }
}

impl<C: rerun::LoggableBatch> rerun::LoggableBatch for Aliased<C> {
    type Name = rerun::ComponentName;

    #[inline]
    fn name(&self) -> Self::Name {
        self.name
    }

    #[inline]
    fn num_instances(&self) -> usize {
        1
    }

    #[inline]
    fn arrow_field(&self) -> arrow2::datatypes::Field {
        self.field.clone()
    }

    #[inline]
    fn to_arrow(&self) -> re_types_core::SerializationResult<Box<dyn arrow2::array::Array>> {
        self.data.to_arrow()
    }
}

impl<C: rerun::LoggableBatch> rerun::ComponentBatch for Aliased<C> {}

// ---

// TODO(cmc): Rerun should provide tools for this.

/// Helper to merge any number of [`rerun::AsComponents`].
pub struct ManyAsComponents(Vec<Box<dyn rerun::AsComponents>>);

impl rerun::AsComponents for ManyAsComponents {
    #[inline]
    fn as_component_batches(&self) -> Vec<rerun::MaybeOwnedComponentBatch<'_>> {
        self.0
            .iter()
            .flat_map(|data| data.as_component_batches())
            .collect()
    }
}

/// Helper to merge two [`rerun::AsComponents`] without extra allocations.
pub struct TwoAsComponents<T1, T2>(T1, T2);

impl<T1, T2> rerun::AsComponents for TwoAsComponents<T1, T2>
where
    T1: rerun::AsComponents,
    T2: rerun::AsComponents,
{
    #[inline]
    fn as_component_batches(&self) -> Vec<rerun::MaybeOwnedComponentBatch<'_>> {
        self.0
            .as_component_batches()
            .into_iter()
            .chain(self.1.as_component_batches())
            .collect()
    }
}
