use std::{io::Read, marker::PhantomData, path::PathBuf};

use bevy_app::{PreStartup, Update};
use bevy_ecs::{
    system::{Res, ResMut, Resource},
    world::World,
};
use bevy_internal::{prelude::DetectChanges, utils::hashbrown::HashMap};
use bevy_tokio_tasks::TokioTasksRuntime;
use tokio::io::AsyncWriteExt;

use crate::{log, AtomicFixedString};

pub trait SaveState
where
    Self: Resource,
{
    type State<'de>: serde::Serialize + serde::Deserialize<'de>;
    const FILENAME: &str;

    fn build(state: Self::State<'_>) -> Self;
    fn save<'de>(&self) -> Self::State<'de>;
}

fn state_file_path(StateDir(path): &StateDir, filename: &'static str) -> PathBuf {
    let mut path = path.clone();
    path.push(format!("{filename}.json"));
    path
}

pub struct StateFile<T>
where
    T: SaveState,
{
    _p: PhantomData<T>,
}
impl<T> StateFile<T>
where
    T: SaveState,
{
    pub fn new() -> Self {
        Self {
            _p: PhantomData::<T>,
        }
    }

    fn init(world: &mut World) {
        let state_dir = world.get_resource::<StateDir>().unwrap();
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(state_file_path(state_dir, T::FILENAME))
            .unwrap();

        let state = {
            let mut state = Vec::new();
            file.read_to_end(&mut state).unwrap();
            state
        };

        let state_info = if let Ok(state_info) =
            serde_json::from_slice::<HashMap<AtomicFixedString, serde_json::Value>>(&state)
        {
            state_info
                .into_iter()
                .map(|(k, v)| (k, v.to_string().into()))
                .collect::<HashMap<AtomicFixedString, AtomicFixedString>>()
        } else {
            HashMap::new()
        };

        if let Ok(state) = serde_json::from_slice(&state) {
            log::info!(
                "[state_file] state loaded from '{}.json': {:?} ",
                T::FILENAME,
                state_info
            );

            world.remove_resource::<T>();
            world.insert_resource(T::build(state))
        } else {
            log::debug!("[state_file] empty state file: {}.json", T::FILENAME);
        };
    }

    fn watcher(
        maybe_this: Option<Res<T>>,
        rt: ResMut<TokioTasksRuntime>,
        state_dir: Res<StateDir>,
        maybe_used: Option<Res<UseStateFile>>,
    ) {
        if maybe_used.is_none() {
            log::debug!("[state_file] plugin disabled!");
            return;
        }

        if let Some(this) = maybe_this {
            if this.is_changed() && !this.is_added() {
                let data =
                    serde_json::to_string(&serde_json::to_value(this.save()).unwrap()).unwrap();

                let file_path = state_file_path(&state_dir, T::FILENAME);

                rt.spawn_background_task(move |_| async move {
                    match tokio::fs::OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .open(file_path)
                        .await
                    {
                        Ok(mut file) => {
                            if let Err(e) = file.write_all(data.as_bytes()).await {
                                log::warn!(
                                    "[state_file] failed to write state to file, reason: {e}"
                                );
                            } else {
                                log::debug!("[state_file] '{}.json' updated -> {data}", T::FILENAME)
                            }
                        }
                        Err(e) => {
                            log::warn!("[state_file] failed to open state file, reason: {e}")
                        }
                    }
                });
            }
        }
    }
}
impl<T> bevy_app::Plugin for StateFile<T>
where
    T: SaveState,
{
    fn build(&self, app: &mut bevy_app::App) {
        if app.world().get_resource::<UseStateFile>().is_some() {
            app.add_systems(PreStartup, (StateFile::<T>::init,))
                .add_systems(Update, (StateFile::<T>::watcher,));
        }
    }
}

#[derive(Resource, Default)]
struct UseStateFile;

pub struct Plugin {
    dirname: &'static str,
}
impl Plugin {
    pub fn disable(world: &mut World) {
        world.remove_resource::<UseStateFile>();
    }
}
impl Default for Plugin {
    fn default() -> Self {
        Self { dirname: "states" }
    }
}
impl bevy_app::Plugin for Plugin {
    fn build(&self, app: &mut bevy_app::App) {
        let mut path = crate::data_directory().to_path_buf();
        path.push(self.dirname);

        std::fs::create_dir_all(&path).unwrap();
        app.init_resource::<UseStateFile>()
            .insert_resource(StateDir(path));
    }
}

#[derive(Debug, Resource)]
struct StateDir(pub PathBuf);
