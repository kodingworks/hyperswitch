use error_stack::ResultExt;

use super::Store;
use crate::core::errors::{self, CustomResult};

pub async fn get_or_populate_cache<T, F, Fut>(
    store: &Store,
    key: &str,
    fun: F,
) -> CustomResult<T, errors::StorageError>
where
    T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
    F: FnOnce() -> Fut + Send,
    Fut: futures::Future<Output = CustomResult<T, errors::StorageError>> + Send,
{
    let redis = &store.redis_conn;
    let redis_val = redis
        .get_and_deserialize_key::<T>(key, std::any::type_name::<T>())
        .await;
    match redis_val {
        Err(err) => match err.current_context() {
            errors::RedisError::NotFound => {
                let data = fun().await?;
                redis
                    .serialize_and_set_key(key, &data)
                    .await
                    .change_context(errors::StorageError::KVError)?;
                Ok(data)
            }
            _ => Err(err
                .change_context(errors::StorageError::KVError)
                .attach_printable("Error while fetching config")),
        },
        Ok(val) => Ok(val),
    }
}

pub async fn redact_cache<T, F, Fut>(
    store: &Store,
    key: &str,
    fun: F,
) -> CustomResult<T, errors::StorageError>
where
    T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
    F: FnOnce() -> Fut + Send,
    Fut: futures::Future<Output = CustomResult<T, errors::StorageError>> + Send,
{
    let data = fun().await?;
    store
        .redis_conn
        .delete_key(key)
        .await
        .change_context(errors::StorageError::KVError)?;
    Ok(data)
}
