use std::{path::PathBuf, sync::Arc, time::Duration};

use bevy_ecs::system::Resource;
use serde_with::serde_as;

use crate::helper::AtomicFixedString;

use super::PersistenceType;

#[serde_with::serde_as]
#[derive(Clone, Resource, serde::Deserialize, serde::Serialize, Debug)]
pub struct ClientCreateOptions {
    pub server_uri: AtomicFixedString,
    pub client_id: AtomicFixedString,

    #[serde(default = "ClientCreateOptions::default_cache_path")]
    pub cache_dir_path: Option<PathBuf>,

    #[serde(default = "ClientCreateOptions::default_incoming_msg_buffer_size")]
    pub incoming_msg_buffer_size: Option<usize>,

    #[serde(
        alias = "restart_interval_ms",
        default = "ClientCreateOptions::default_restart_interval"
    )]
    #[serde_as(as = "Option<serde_with::DurationMilliSeconds<u64>>")]
    pub restart_interval: Option<Duration>,

    pub max_buffered_messages: Option<i32>,
    pub persistence_type: Option<PersistenceType>,
    pub send_while_disconnected: Option<bool>,
    pub allow_disconnected_send_at_anytime: Option<bool>,
    pub delete_oldest_messages: Option<bool>,
    pub restore_messages: Option<bool>,
    pub persist_qos0: Option<bool>,
}
impl ClientCreateOptions {
    fn default_cache_path() -> Option<PathBuf> {
        let mut cache_dir_path = std::env::current_dir().unwrap();
        cache_dir_path.push("data");
        cache_dir_path.push("cache");
        Some(cache_dir_path)
    }

    fn default_incoming_msg_buffer_size() -> Option<usize> {
        Some(25)
    }

    fn default_restart_interval() -> Option<Duration> {
        Some(Duration::from_secs(5))
    }
}
impl From<&ClientCreateOptions> for paho_mqtt::CreateOptions {
    fn from(value: &ClientCreateOptions) -> Self {
        let ClientCreateOptions {
            server_uri,
            client_id,
            max_buffered_messages,
            persistence_type,
            send_while_disconnected,
            allow_disconnected_send_at_anytime,
            delete_oldest_messages,
            restore_messages,
            persist_qos0,
            incoming_msg_buffer_size: _,
            cache_dir_path: _,
            restart_interval: _,
        } = value;

        let builder = {
            let server_uri: Arc<str> = server_uri.clone().into();
            let client_id: Arc<str> = client_id.clone().into();

            paho_mqtt::CreateOptionsBuilder::new()
                .server_uri(server_uri.to_string())
                .client_id(client_id.to_string())
        };

        let builder = if let Some(n) = *max_buffered_messages {
            builder.max_buffered_messages(n)
        } else {
            builder
        };

        let builder = if let Some(persist) = persistence_type {
            builder.persistence(persist)
        } else {
            builder
        };

        let builder = if let Some(on) = *send_while_disconnected {
            builder.send_while_disconnected(on)
        } else {
            builder
        };

        let builder = if let Some(allow) = *allow_disconnected_send_at_anytime {
            builder.allow_disconnected_send_at_anytime(allow)
        } else {
            builder
        };

        let builder = if let Some(delete_oldest) = *delete_oldest_messages {
            builder.delete_oldest_messages(delete_oldest)
        } else {
            builder
        };

        let builder = if let Some(restore) = *restore_messages {
            builder.restore_messages(restore)
        } else {
            builder
        };

        let builder = if let Some(persist) = *persist_qos0 {
            builder.persist_qos0(persist)
        } else {
            builder
        };

        builder.finalize()
    }
}

#[serde_with::serde_as]
#[derive(Clone, Resource, serde::Serialize, serde::Deserialize, Debug)]
pub struct ClientConnectOptions {
    pub clean_start: Option<bool>,
    pub max_inflight: Option<i32>,

    #[serde(alias = "connect_timeout_ms")]
    #[serde_as(as = "Option<serde_with::DurationMilliSeconds<u64>>")]
    pub connect_timeout: Option<Duration>,

    #[serde(alias = "keep_alive_interval_ms")]
    #[serde_as(as = "Option<serde_with::DurationMilliSeconds<u64>>")]
    pub keep_alive_interval: Option<Duration>,
}
impl From<&ClientConnectOptions> for paho_mqtt::ConnectOptions {
    fn from(value: &ClientConnectOptions) -> Self {
        let ClientConnectOptions {
            clean_start,
            connect_timeout,
            keep_alive_interval,
            max_inflight,
        } = value;

        let mut builder = paho_mqtt::ConnectOptionsBuilder::new();

        if let Some(clean) = clean_start {
            builder.clean_start(*clean);
        }

        if let Some(timeout) = connect_timeout {
            builder.connect_timeout(*timeout);
        }

        if let Some(keep_alive_interval) = keep_alive_interval {
            builder.keep_alive_interval(*keep_alive_interval);
        }

        if let Some(max_inflight) = max_inflight {
            builder.max_inflight(*max_inflight);
        }

        builder.finalize()
    }
}
