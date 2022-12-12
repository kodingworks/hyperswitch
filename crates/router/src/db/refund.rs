use error_stack::Report;

use super::MockDb;
use crate::{
    core::errors::{self, CustomResult, DatabaseError, StorageError},
    types::storage::{enums, Refund, RefundNew, RefundUpdate},
};

#[async_trait::async_trait]
pub trait RefundInterface {
    async fn find_refund_by_internal_reference_id_merchant_id(
        &self,
        internal_reference_id: &str,
        merchant_id: &str,
        storage_scheme: enums::MerchantStorageScheme,
    ) -> CustomResult<Refund, errors::StorageError>;

    async fn find_refund_by_payment_id_merchant_id(
        &self,
        payment_id: &str,
        merchant_id: &str,
        storage_scheme: enums::MerchantStorageScheme,
    ) -> CustomResult<Vec<Refund>, errors::StorageError>;

    // async fn find_refund_by_payment_id_merchant_id_refund_id(
    //     &self,
    //     payment_id: &str,
    //     merchant_id: &str,
    //     refund_id: &str,
    // ) -> CustomResult<Refund, errors::StorageError>;

    async fn find_refund_by_merchant_id_refund_id(
        &self,
        merchant_id: &str,
        refund_id: &str,
        storage_scheme: enums::MerchantStorageScheme,
    ) -> CustomResult<Refund, errors::StorageError>;

    async fn update_refund(
        &self,
        this: Refund,
        refund: RefundUpdate,
        storage_scheme: enums::MerchantStorageScheme,
    ) -> CustomResult<Refund, errors::StorageError>;

    async fn find_refund_by_merchant_id_transaction_id(
        &self,
        merchant_id: &str,
        txn_id: &str,
        storage_scheme: enums::MerchantStorageScheme,
    ) -> CustomResult<Vec<Refund>, errors::StorageError>;

    async fn insert_refund(
        &self,
        new: RefundNew,
        storage_scheme: enums::MerchantStorageScheme,
    ) -> CustomResult<Refund, errors::StorageError>;
}

#[cfg(not(feature = "kv_store"))]
mod storage {
    #[async_trait::async_trait]
    impl super::RefundInterface for super::Store {
        async fn find_refund_by_internal_reference_id_merchant_id(
            &self,
            internal_reference_id: &str,
            merchant_id: &str,
            _storage_scheme: enums::MerchantStorageScheme,
        ) -> CustomResult<Refund, errors::StorageError> {
            let conn = pg_connection(&self.master_pool).await;
            Refund::find_by_internal_reference_id_merchant_id(
                &conn,
                internal_reference_id,
                merchant_id,
            )
            .await
        }

        async fn insert_refund(
            &self,
            new: RefundNew,
            _storage_scheme: enums::MerchantStorageScheme,
        ) -> CustomResult<Refund, errors::StorageError> {
            let conn = pg_connection(&self.master_pool).await;
            new.insert(&conn).await
        }

        async fn find_refund_by_merchant_id_transaction_id(
            &self,
            merchant_id: &str,
            txn_id: &str,
            _storage_scheme: enums::MerchantStorageScheme,
        ) -> CustomResult<Vec<Refund>, errors::StorageError> {
            let conn = pg_connection(&self.master_pool).await;
            Refund::find_by_merchant_id_transaction_id(&conn, merchant_id, txn_id).await
        }

        async fn update_refund(
            &self,
            this: Refund,
            refund: RefundUpdate,
            _storage_scheme: enums::MerchantStorageScheme,
        ) -> CustomResult<Refund, errors::StorageError> {
            let conn = pg_connection(&self.master_pool).await;
            this.update(&conn, refund).await
        }

        async fn find_refund_by_merchant_id_refund_id(
            &self,
            merchant_id: &str,
            refund_id: &str,
            _storage_scheme: enums::MerchantStorageScheme,
        ) -> CustomResult<Refund, errors::StorageError> {
            let conn = pg_connection(&self.master_pool).await;
            Refund::find_by_merchant_id_refund_id(&conn, merchant_id, refund_id).await
        }

        // async fn find_refund_by_payment_id_merchant_id_refund_id(
        //     &self,
        //     payment_id: &str,
        //     merchant_id: &str,
        //     refund_id: &str,
        // ) -> CustomResult<Refund, errors::StorageError> {
        //     let conn = pg_connection(&self.master_pool).await;
        //     Refund::find_by_payment_id_merchant_id_refund_id(&conn, payment_id, merchant_id, refund_id)
        //         .await
        // }

        async fn find_refund_by_payment_id_merchant_id(
            &self,
            payment_id: &str,
            merchant_id: &str,
            _storage_scheme: enums::MerchantStorageScheme,
        ) -> CustomResult<Vec<Refund>, errors::StorageError> {
            let conn = pg_connection(&self.master_pool).await;
            Refund::find_by_payment_id_merchant_id(&conn, payment_id, merchant_id).await
        }
    }
}

#[cfg(feature = "kv_store")]
mod storage {
    use common_utils::{date_time, ext_traits::StringExt};
    use error_stack::{IntoReport, ResultExt};
    use futures::StreamExt;
    use redis_interface::{HashesInterface, RedisEntryId};

    use crate::{
        connection::pg_connection,
        core::errors::{self, CustomResult},
        db::reverse_lookup::ReverseLookupInterface,
        services::Store,
        types::storage::{enums, Refund, RefundNew, RefundUpdate, ReverseLookupNew},
        utils::storage_partitioning::KvStorePartition,
    };
    #[async_trait::async_trait]
    impl super::RefundInterface for Store {
        async fn find_refund_by_internal_reference_id_merchant_id(
            &self,
            internal_reference_id: &str,
            merchant_id: &str,
            _storage_scheme: enums::MerchantStorageScheme,
        ) -> CustomResult<Refund, errors::StorageError> {
            let conn = pg_connection(&self.master_pool).await;
            Refund::find_by_internal_reference_id_merchant_id(
                &conn,
                internal_reference_id,
                merchant_id,
            )
            .await
        }

        async fn insert_refund(
            &self,
            new: RefundNew,
            storage_scheme: enums::MerchantStorageScheme,
        ) -> CustomResult<Refund, errors::StorageError> {
            match storage_scheme {
                enums::MerchantStorageScheme::PostgresOnly => {
                    let conn = pg_connection(&self.master_pool).await;
                    new.insert(&conn).await
                }
                enums::MerchantStorageScheme::RedisKv => {
                    let key = format!("{}_{}", new.payment_id, new.merchant_id);
                    // TODO: need to add an application generated payment attempt id to distinguish between multiple attempts for the same payment id
                    // Check for database presence as well Maybe use a read replica here ?
                    let created_refund = Refund {
                        id: 0i32,
                        refund_id: new.refund_id.clone(),
                        merchant_id: new.merchant_id.clone(),
                        internal_reference_id: new.internal_reference_id.clone(),
                        payment_id: new.payment_id.clone(),
                        transaction_id: new.transaction_id.clone(),
                        connector: new.connector.clone(),
                        pg_refund_id: new.pg_refund_id.clone(),
                        external_reference_id: new.external_reference_id.clone(),
                        refund_type: new.refund_type,
                        total_amount: new.total_amount,
                        currency: new.currency,
                        refund_amount: new.refund_amount,
                        refund_status: new.refund_status,
                        sent_to_gateway: new.sent_to_gateway,
                        refund_error_message: new.refund_error_message.clone(),
                        metadata: new.metadata.clone(),
                        refund_arn: new.refund_arn.clone(),
                        created_at: new.created_at.unwrap_or_else(date_time::now),
                        updated_at: new.created_at.unwrap_or_else(date_time::now),
                        description: new.description.clone(),
                    };
                    // TODO: Add a proper error for serialization failure
                    let redis_value = serde_json::to_string(&created_refund)
                        .into_report()
                        .change_context(errors::StorageError::KVError)?;

                    let field = format!(
                        "pa_{}_ref_{}",
                        &created_refund.payment_id, &created_refund.refund_id
                    );
                    match self
                        .redis_conn
                        .pool
                        .hsetnx::<u8, &str, &str, &str>(&key, &field, &redis_value)
                        .await
                    {
                        Ok(0) => Err(errors::StorageError::DuplicateValue(format!(
                            "Refund already exists refund_id: {}",
                            &created_refund.refund_id
                        )))
                        .into_report(),
                        Ok(1) => {
                            let conn = pg_connection(&self.master_pool).await;
                            let query = new
                                .insert_diesel_query(&conn)
                                .await
                                .change_context(errors::StorageError::KVError)?;

                            ReverseLookupNew::new(
                                created_refund.refund_id.clone(),
                                format!(
                                    "{}_{}",
                                    created_refund.refund_id, created_refund.merchant_id
                                ),
                                key.clone(),
                                "ref".to_string(),
                            )
                            .insert(&conn)
                            .await?;

                            //Reverse lookup for txn_id
                            ReverseLookupNew::new(
                                created_refund.refund_id.clone(),
                                format!(
                                    "{}_{}",
                                    created_refund.transaction_id, created_refund.merchant_id
                                ),
                                key,
                                "ref".to_string(),
                            )
                            .insert(&conn)
                            .await?;

                            let stream_name = self.drainer_stream(&Refund::shard_key(
                            crate::utils::storage_partitioning::PartitionKey::MerchantIdPaymentId {
                                merchant_id: &created_refund.merchant_id,
                                payment_id: &created_refund.payment_id,
                            },
                            self.config.drainer_num_partitions,
                        ));
                            self.redis_conn
                                .stream_append_entry(
                                    &stream_name,
                                    &RedisEntryId::AutoGeneratedID,
                                    query.to_field_value_pairs(),
                                )
                                .await
                                .change_context(errors::StorageError::KVError)?;
                            Ok(created_refund)
                        }
                        Ok(i) => Err(errors::StorageError::KVError)
                            .into_report()
                            .attach_printable_lazy(|| {
                                format!("Invalid response for HSETNX: {}", i)
                            }),
                        Err(er) => Err(er)
                            .into_report()
                            .change_context(errors::StorageError::KVError),
                    }
                }
            }
        }

        async fn find_refund_by_merchant_id_transaction_id(
            &self,
            merchant_id: &str,
            txn_id: &str,
            storage_scheme: enums::MerchantStorageScheme,
        ) -> CustomResult<Vec<Refund>, errors::StorageError> {
            match storage_scheme {
                enums::MerchantStorageScheme::PostgresOnly => {
                    let conn = pg_connection(&self.master_pool).await;
                    Refund::find_by_merchant_id_transaction_id(&conn, merchant_id, txn_id).await
                }
                enums::MerchantStorageScheme::RedisKv => {
                    let lookup_id = format!("{}_{}", txn_id, merchant_id);
                    let lookup = self.get_lookup_by_lookup_id(&lookup_id).await?;
                    let key = &lookup.result_id;
                    let payment_id = key.split('_').next().ok_or(errors::StorageError::KVError)?;

                    let field = format!("pa_{}_ref_*", payment_id);
                    let redis_results = self
                        .redis_conn
                        .pool
                        .hscan::<&str, &str>(key, &field, None)
                        .filter_map(|value| async move {
                            match value {
                                Ok(mut v) => {
                                    let v = v.take_results()?;
                                    let v: Vec<String> =
                                        v.iter().filter_map(|(_, val)| val.as_string()).collect();
                                    Some(v)
                                }
                                Err(_) => None,
                            }
                        })
                        .collect::<Vec<_>>()
                        .await;
                    Ok(redis_results
                        .iter()
                        .flatten()
                        .filter_map(|v| {
                            let r: Refund = v.parse_struct("Refund").ok()?;
                            Some(r)
                        })
                        .collect())
                }
            }
        }

        async fn update_refund(
            &self,
            this: Refund,
            refund: RefundUpdate,
            storage_scheme: enums::MerchantStorageScheme,
        ) -> CustomResult<Refund, errors::StorageError> {
            match storage_scheme {
                enums::MerchantStorageScheme::PostgresOnly => {
                    let conn = pg_connection(&self.master_pool).await;
                    this.update(&conn, refund).await
                }
                enums::MerchantStorageScheme::RedisKv => {
                    let key = format!("{}_{}", this.payment_id, this.merchant_id);

                    let updated_refund = refund.clone().apply_changeset(this.clone());
                    // Check for database presence as well Maybe use a read replica here ?
                    // TODO: Add a proper error for serialization failure
                    let redis_value = serde_json::to_string(&updated_refund)
                        .into_report()
                        .change_context(errors::StorageError::KVError)?;
                    let field = format!(
                        "pa_{}_ref_{}",
                        &updated_refund.payment_id, &updated_refund.refund_id
                    );

                    let updated_refund = self
                        .redis_conn
                        .pool
                        .hset::<u8, &str, (&str, String)>(&key, (&field, redis_value))
                        .await
                        .map(|_| updated_refund)
                        .into_report()
                        .change_context(errors::StorageError::KVError)?;

                    let conn = pg_connection(&self.master_pool).await;
                    let query = this
                        .update_query(&conn, refund)
                        .await
                        .change_context(errors::StorageError::KVError)?;

                    let stream_name = self.drainer_stream(&Refund::shard_key(
                        crate::utils::storage_partitioning::PartitionKey::MerchantIdPaymentId {
                            merchant_id: &updated_refund.merchant_id,
                            payment_id: &updated_refund.payment_id,
                        },
                        self.config.drainer_num_partitions,
                    ));
                    self.redis_conn
                        .stream_append_entry(
                            &stream_name,
                            &RedisEntryId::AutoGeneratedID,
                            query.to_field_value_pairs(),
                        )
                        .await
                        .change_context(errors::StorageError::KVError)?;
                    Ok(updated_refund)
                }
            }
        }

        async fn find_refund_by_merchant_id_refund_id(
            &self,
            merchant_id: &str,
            refund_id: &str,
            storage_scheme: enums::MerchantStorageScheme,
        ) -> CustomResult<Refund, errors::StorageError> {
            match storage_scheme {
                enums::MerchantStorageScheme::PostgresOnly => {
                    let conn = pg_connection(&self.master_pool).await;
                    Refund::find_by_merchant_id_refund_id(&conn, merchant_id, refund_id).await
                }
                enums::MerchantStorageScheme::RedisKv => {
                    let lookup_id = format!("{}_{}", refund_id, merchant_id);
                    let lookup = self.get_lookup_by_lookup_id(&lookup_id).await?;
                    let key = &lookup.result_id;
                    let payment_id = key.split('_').next().ok_or(errors::StorageError::KVError)?;
                    let field = format!("pa_{}_ref_{}", payment_id, refund_id);

                    self.redis_conn
                        .pool
                        .hget::<String, &str, &str>(key, &field)
                        .await
                        .into_report()
                        .change_context(errors::StorageError::KVError)
                        .and_then(|redis_resp| {
                            serde_json::from_str::<Refund>(&redis_resp)
                                .into_report()
                                .change_context(errors::StorageError::KVError)
                        })
                }
            }
        }

        // async fn find_refund_by_payment_id_merchant_id_refund_id(
        //     &self,
        //     payment_id: &str,
        //     merchant_id: &str,
        //     refund_id: &str,
        // ) -> CustomResult<Refund, errors::StorageError> {
        //     let conn = pg_connection(&self.master_pool).await;
        //     Refund::find_by_payment_id_merchant_id_refund_id(&conn, payment_id, merchant_id, refund_id)
        //         .await
        // }

        async fn find_refund_by_payment_id_merchant_id(
            &self,
            payment_id: &str,
            merchant_id: &str,
            storage_scheme: enums::MerchantStorageScheme,
        ) -> CustomResult<Vec<Refund>, errors::StorageError> {
            match storage_scheme {
                enums::MerchantStorageScheme::PostgresOnly => {
                    let conn = pg_connection(&self.master_pool).await;
                    Refund::find_by_payment_id_merchant_id(&conn, payment_id, merchant_id).await
                }
                enums::MerchantStorageScheme::RedisKv => {
                    let lookup_id = format!("{}_{}", payment_id, merchant_id);
                    let lookup = self.get_lookup_by_lookup_id(&lookup_id).await?;
                    let key = &lookup.result_id;
                    let payment_id = key.split('_').next().ok_or(errors::StorageError::KVError)?;

                    let field = format!("pa_{}_ref_*", payment_id);

                    let redis_results = self
                        .redis_conn
                        .pool
                        .hscan::<&str, &str>(key, &field, None)
                        .filter_map(|value| async move {
                            match value {
                                Ok(mut v) => {
                                    let v = v.take_results()?;
                                    let v: Vec<String> =
                                        v.iter().filter_map(|(_, val)| val.as_string()).collect();
                                    Some(v)
                                }
                                Err(_) => None,
                            }
                        })
                        .collect::<Vec<_>>()
                        .await;
                    Ok(redis_results
                        .iter()
                        .flatten()
                        .filter_map(|v| {
                            let r: Refund = v.parse_struct("Refund").ok()?;
                            Some(r)
                        })
                        .collect())
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl RefundInterface for MockDb {
    async fn find_refund_by_internal_reference_id_merchant_id(
        &self,
        _internal_reference_id: &str,
        _merchant_id: &str,
        _storage_scheme: enums::MerchantStorageScheme,
    ) -> CustomResult<Refund, errors::StorageError> {
        todo!()
    }

    async fn insert_refund(
        &self,
        new: RefundNew,
        _storage_scheme: enums::MerchantStorageScheme,
    ) -> CustomResult<Refund, errors::StorageError> {
        let mut refunds = self.refunds.lock().await;
        let current_time = common_utils::date_time::now();

        let refund = Refund {
            id: refunds.len() as i32,
            internal_reference_id: new.internal_reference_id,
            refund_id: new.refund_id,
            payment_id: new.payment_id,
            merchant_id: new.merchant_id,
            transaction_id: new.transaction_id,
            connector: new.connector,
            pg_refund_id: new.pg_refund_id,
            external_reference_id: new.external_reference_id,
            refund_type: new.refund_type,
            total_amount: new.total_amount,
            currency: new.currency,
            refund_amount: new.refund_amount,
            refund_status: new.refund_status,
            sent_to_gateway: new.sent_to_gateway,
            refund_error_message: new.refund_error_message,
            metadata: new.metadata,
            refund_arn: new.refund_arn.clone(),
            created_at: new.created_at.unwrap_or(current_time),
            updated_at: current_time,
            description: new.description,
        };
        refunds.push(refund.clone());
        Ok(refund)
    }
    async fn find_refund_by_merchant_id_transaction_id(
        &self,
        merchant_id: &str,
        txn_id: &str,
        _storage_scheme: enums::MerchantStorageScheme,
    ) -> CustomResult<Vec<Refund>, errors::StorageError> {
        let refunds = self.refunds.lock().await;

        Ok(refunds
            .iter()
            .take_while(|refund| {
                refund.merchant_id == merchant_id && refund.transaction_id == txn_id
            })
            .cloned()
            .collect::<Vec<_>>())
    }

    async fn update_refund(
        &self,
        _this: Refund,
        _refund: RefundUpdate,
        _storage_scheme: enums::MerchantStorageScheme,
    ) -> CustomResult<Refund, errors::StorageError> {
        todo!()
    }

    async fn find_refund_by_merchant_id_refund_id(
        &self,
        merchant_id: &str,
        refund_id: &str,
        _storage_scheme: enums::MerchantStorageScheme,
    ) -> CustomResult<Refund, errors::StorageError> {
        let refunds = self.refunds.lock().await;

        refunds
            .iter()
            .find(|refund| refund.merchant_id == merchant_id && refund.refund_id == refund_id)
            .cloned()
            .ok_or_else(|| Report::from(StorageError::DatabaseError(DatabaseError::NotFound)))
    }

    async fn find_refund_by_payment_id_merchant_id(
        &self,
        _payment_id: &str,
        _merchant_id: &str,
        _storage_scheme: enums::MerchantStorageScheme,
    ) -> CustomResult<Vec<Refund>, errors::StorageError> {
        todo!()
    }
}
