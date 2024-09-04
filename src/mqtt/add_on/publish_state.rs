use std::time::Duration;

use bevy_app::{Plugin, Update};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::With,
    schedule::IntoSystemConfigs,
    system::{Commands, Query, ResMut, Resource},
    world::World,
};
use bevy_internal::{time::common_conditions::on_timer, utils::HashMap};

use crate::mqtt::component;

pub struct PublishStatePlugin {
    pub publish_interval: Duration,
}
impl Plugin for PublishStatePlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.insert_resource(StatePublishRegistry::new())
            .add_systems(
                Update,
                (
                    StatePublishRegistry::update,
                    StatePublishRegistry::publish
                        .run_if(on_timer(self.publish_interval))
                        .after(StatePublishRegistry::update),
                ),
            );
    }
}

#[derive(Component)]
pub struct UpdateState {
    pub(super) id: std::any::TypeId,
    pub(super) data: Box<dyn StatePublisher + Send + Sync + 'static>,
}
impl UpdateState {
    pub fn new<T: StatePublisher + Send + Sync + 'static>(new_state: T) -> Self {
        Self {
            id: std::any::TypeId::of::<T>(),
            data: Box::new(new_state),
        }
    }
}

pub trait StatePublisher {
    fn to_publish(&self) -> component::PublishMsg;
}

#[derive(Resource)]
struct StatePublishRegistry {
    hashmap: HashMap<std::any::TypeId, component::PublishMsg>,
}
impl StatePublishRegistry {
    fn new() -> Self {
        Self {
            hashmap: HashMap::new(),
        }
    }

    fn update(mut cmd: Commands, entt: Query<Entity, With<UpdateState>>) {
        entt.iter().for_each(|entt| {
            cmd.add(move |world: &mut World| {
                let maybe_new_state = world.entity_mut(entt).take::<UpdateState>();

                if let Some(UpdateState { id, data }) = maybe_new_state {
                    let mut registry = world.get_resource_mut::<StatePublishRegistry>().unwrap();
                    registry.hashmap.insert(id, data.to_publish());
                }
            });
        })
    }

    fn publish(mut cmd: Commands, registry: ResMut<StatePublishRegistry>) {
        registry.hashmap.iter().for_each(|(_, msg)| {
            cmd.spawn(msg.clone());
        });
    }
}
