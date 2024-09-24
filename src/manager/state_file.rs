use std::{io::Read, marker::PhantomData, path::PathBuf};

use bevy_app::{PreStartup, Update};
use bevy_ecs::{
    system::{Res, ResMut, Resource},
    world::World,
};
use bevy_internal::prelude::DetectChanges;
use bevy_tokio_tasks::TokioTasksRuntime;
use tokio::io::AsyncWriteExt;

use crate::log;

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

        if let Ok(state) = serde_json::from_slice(&state) {
            world.insert_resource(T::build(state))
        };
    }

    fn watcher(
        maybe_this: Option<Res<T>>,
        rt: ResMut<TokioTasksRuntime>,
        state_dir: Res<StateDir>,
    ) {
        if let Some(this) = maybe_this {
            if this.is_changed() && !this.is_added() {
                let data = {
                    let mut data =
                        serde_json::to_string_pretty(&serde_json::to_value(this.save()).unwrap())
                            .unwrap();
                    data.push('\n');
                    data
                };

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
                                log::warn!("failed to write state to file, reason: {e}");
                            } else {
                                log::info!("state file '{}' updated:\n{data}", T::FILENAME)
                            }
                        }
                        Err(e) => {
                            log::warn!("failed to open state file, reason: {e}")
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
        app.add_systems(PreStartup, (StateFile::<T>::init,))
            .add_systems(Update, (StateFile::<T>::watcher,));
    }
}

pub struct Plugin {
    dirname: &'static str,
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
        app.insert_resource(StateDir(path));
    }
}

#[derive(Debug, Resource)]
struct StateDir(pub PathBuf);
