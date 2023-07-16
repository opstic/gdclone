use bevy::ecs::{
    archetype::{ArchetypeComponentId, ArchetypeGeneration, ArchetypeId},
    component::{ComponentId, Tick},
    query::{Access, FilteredAccess, QueryItem, ROQueryItem, ReadOnlyWorldQuery, WorldQuery},
    storage::TableId,
    world::{unsafe_world_cell::UnsafeWorldCell, WorldId},
};
use bevy::prelude::{Entity, Query, QueryState};
use bevy::tasks::ComputeTaskPool;
use fixedbitset::FixedBitSet;

// Sorry Bevy
pub(crate) struct LocalQuery<'world, 'state, Q: WorldQuery, F: ReadOnlyWorldQuery = ()> {
    world: UnsafeWorldCell<'world>,
    state: &'state QueryState<Q, F>,
    last_run: Tick,
    this_run: Tick,
    force_read_only_component_access: bool,
}

impl<'w, 's, Q: WorldQuery, F: ReadOnlyWorldQuery> From<&Query<'w, 's, Q, F>>
    for &LocalQuery<'w, 's, Q, F>
{
    fn from(query: &Query<Q, F>) -> Self {
        unsafe { std::mem::transmute(query) }
    }
}

impl<'w, 's, Q: WorldQuery, F: ReadOnlyWorldQuery> From<&mut Query<'w, 's, Q, F>>
    for &mut LocalQuery<'w, 's, Q, F>
{
    fn from(query: &mut Query<Q, F>) -> Self {
        unsafe { std::mem::transmute(query) }
    }
}

#[repr(C)]
pub(crate) struct LocalQueryState<Q: WorldQuery, F: ReadOnlyWorldQuery = ()> {
    world_id: WorldId,
    archetype_generation: ArchetypeGeneration,
    matched_tables: FixedBitSet,
    matched_archetypes: FixedBitSet,
    archetype_component_access: Access<ArchetypeComponentId>,
    component_access: FilteredAccess<ComponentId>,
    matched_table_ids: Vec<TableId>,
    matched_archetype_ids: Vec<ArchetypeId>,
    fetch_state: Q::State,
    filter_state: F::State,
}

impl<Q: WorldQuery, F: ReadOnlyWorldQuery> From<&QueryState<Q, F>> for &LocalQueryState<Q, F> {
    fn from(query_state: &QueryState<Q, F>) -> Self {
        unsafe { std::mem::transmute(query_state) }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub(crate) struct LocalArchetypeId(u32);

impl LocalArchetypeId {
    #[inline]
    pub(crate) fn index(self) -> usize {
        self.0 as usize
    }
}

impl From<ArchetypeId> for LocalArchetypeId {
    fn from(archetype_id: ArchetypeId) -> Self {
        unsafe { std::mem::transmute(archetype_id) }
    }
}

pub(crate) struct QueryParManyIter<'w, 's, 'l, Q: WorldQuery, F: ReadOnlyWorldQuery> {
    entity_list: &'l [Entity],
    world: UnsafeWorldCell<'w>,
    state: &'s QueryState<Q, F>,
    last_run: Tick,
    this_run: Tick,
}

impl<'w, 's, 'l, Q: ReadOnlyWorldQuery, F: ReadOnlyWorldQuery> QueryParManyIter<'w, 's, 'l, Q, F> {
    #[inline]
    pub(crate) fn for_each<FN: Fn(ROQueryItem<'w, Q>) + Send + Sync + Clone>(&self, func: FN) {
        // SAFETY: query is read only
        unsafe {
            self.for_each_unchecked(func);
        }
    }
}

impl<'w, 's, 'l, Q: WorldQuery, F: ReadOnlyWorldQuery> QueryParManyIter<'w, 's, 'l, Q, F> {
    unsafe fn new(
        world: UnsafeWorldCell<'w>,
        query_state: &'s QueryState<Q, F>,
        entity_list: &'l [Entity],
        last_run: Tick,
        this_run: Tick,
    ) -> QueryParManyIter<'w, 's, 'l, Q, F> {
        QueryParManyIter {
            state: query_state,
            world,
            last_run,
            this_run,
            entity_list,
        }
    }

    #[inline]
    pub fn for_each_mut<FN: Fn(QueryItem<'w, Q>) + Send + Sync + Clone>(&mut self, func: FN) {
        // SAFETY: query has unique world access
        unsafe {
            self.for_each_unchecked(func);
        }
    }

    #[inline]
    unsafe fn for_each_unchecked<FN: Fn(Q::Item<'w>) + Send + Sync + Clone>(&self, func: FN) {
        let task_pool = ComputeTaskPool::get();
        let chunk_size = (self.entity_list.len() / task_pool.thread_num()).max(1);
        let query_state: &LocalQueryState<Q, F> = self.state.into();
        task_pool.scope(|scope| {
            for entity_chunk in self.entity_list.chunks(chunk_size) {
                let func = func.clone();
                let task = async move {
                    let mut fetch = Q::init_fetch(
                        self.world,
                        &query_state.fetch_state,
                        self.last_run,
                        self.this_run,
                    );
                    let mut filter = F::init_fetch(
                        self.world,
                        &query_state.filter_state,
                        self.last_run,
                        self.this_run,
                    );

                    let entities = self.world.entities();
                    let tables = &self.world.storages().tables;
                    let archetypes = self.world.archetypes();

                    for entity in entity_chunk {
                        let entity = *entity;

                        let location = match entities.get(entity) {
                            Some(location) => location,
                            None => continue,
                        };

                        let archetype_id: LocalArchetypeId = location.archetype_id.into();

                        if !query_state
                            .matched_archetypes
                            .contains(archetype_id.index())
                        {
                            continue;
                        }

                        let archetype = archetypes.get(location.archetype_id).unwrap_unchecked();
                        let table = tables.get(location.table_id).unwrap_unchecked();

                        Q::set_archetype(&mut fetch, &query_state.fetch_state, archetype, table);
                        F::set_archetype(&mut filter, &query_state.filter_state, archetype, table);

                        // SAFETY: set_archetype was called prior.
                        // `location.archetype_row` is an archetype index row in range of the current archetype, because if it was not, the match above would have `continue`d
                        if F::filter_fetch(&mut filter, entity, location.table_row) {
                            // SAFETY: set_archetype was called prior, `location.archetype_row` is an archetype index in range of the current archetype
                            func(Q::fetch(&mut fetch, entity, location.table_row));
                        }
                    }
                };

                let span = bevy::utils::tracing::info_span!(
                    "par_for_each_many",
                    query = std::any::type_name::<Q>(),
                    filter = std::any::type_name::<F>(),
                    count = entity_chunk.len(),
                );
                use bevy::utils::tracing::Instrument;
                let task = task.instrument(span);

                scope.spawn(task);
            }
        });
    }
}

#[inline]
pub(crate) fn par_iter_many<'w, 's, 'l, Q: WorldQuery, F: ReadOnlyWorldQuery>(
    query: &Query<'w, 's, Q, F>,
    entities: &'l [Entity],
) -> QueryParManyIter<'w, 's, 'l, Q::ReadOnly, F::ReadOnly> {
    let query: &LocalQuery<Q, F> = query.into();
    unsafe {
        QueryParManyIter::new(
            query.world,
            query.state.as_readonly(),
            entities,
            query.last_run,
            query.this_run,
        )
    }
}

#[inline]
pub(crate) fn par_iter_many_mut<'w, 's, 'l, Q: WorldQuery, F: ReadOnlyWorldQuery>(
    query: &mut Query<'w, 's, Q, F>,
    entities: &'l [Entity],
) -> QueryParManyIter<'w, 's, 'l, Q, F> {
    let query: &mut LocalQuery<Q, F> = query.into();
    unsafe {
        QueryParManyIter::new(
            query.world,
            query.state,
            entities,
            query.last_run,
            query.this_run,
        )
    }
}
