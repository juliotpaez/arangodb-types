use std::collections::HashSet;
use std::sync::Arc;

use arangors::{AqlOptions, AqlQuery};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::aql::{AqlBuilder, AqlInsert};
use crate::aql::AQL_DOCUMENT_ID;
use crate::aql::AQL_NEW_ID;
use crate::aql::AqlLet;
use crate::aql::AqlLetKind;
use crate::aql::AqlLimit;
use crate::aql::AqlResult;
use crate::aql::AqlReturn;
use crate::aql::AqlUpdate;
use crate::documents::DBDocumentField;
use crate::traits::DBDocument;
use crate::traits::utils::check_client_is_write_conflict;
use crate::types::Collection;
use crate::types::Database;
use crate::types::DBInfo;

#[async_trait]
pub trait DBCollection: Send + Sync {
    type Document: DBDocument<Collection = Self>;

    // GETTERS ----------------------------------------------------------------

    /// Gets the collection name from the configuration.
    fn name() -> &'static str;

    /// Gets the database information.
    fn db_info(&self) -> &Arc<DBInfo>;

    /// Gets the arangodb instance of this collection.
    async fn db_collection(&self) -> Result<Collection, anyhow::Error> {
        let db_info = self.db_info();
        Ok(db_info.database.collection(Self::name()).await?)
    }

    fn database(&self) -> &Database {
        &self.db_info().database
    }

    // METHODS ----------------------------------------------------------------

    /// Checks whether a document exists in the DB by its key.
    async fn exists_by_key(
        &self,
        key: &<Self::Document as DBDocument>::Key,
    ) -> Result<bool, anyhow::Error> {
        Ok(self.get_one_by_key(key, None).await?.is_some())
    }

    /// Checks whether a document exists in the DB by a single custom property.
    async fn exists_by<V: Serialize + Send + Sync>(
        &self,
        property_path: &str,
        value: &V,
    ) -> Result<bool, anyhow::Error> {
        Ok(self.get_one_by(property_path, value, None).await?.is_some())
    }

    /// Gets all documents in the collection. Useful for cache.
    async fn get_all(
        &self,
        return_fields: Option<&Self::Document>,
    ) -> Result<Vec<Self::Document>, anyhow::Error> {
        // Prepare AQL.
        // FOR i IN <collection>
        //      RETURN i
        let mut aql = AqlBuilder::new_for_in_collection(AQL_DOCUMENT_ID, Self::name());

        if let Some(fields) = return_fields {
            aql.return_step_with_fields(AQL_DOCUMENT_ID, fields);
        } else {
            aql.return_step(AqlReturn::new_document());
        }

        let aql_result = self.send_aql(&aql).await?;

        Ok(aql_result.results)
    }

    /// Gets a document from the DB by its key.
    async fn get_one_by_key(
        &self,
        key: &<<Self as DBCollection>::Document as DBDocument>::Key,
        return_fields: Option<&Self::Document>,
    ) -> Result<Option<Self::Document>, anyhow::Error> {
        let result = self
            .get_one_by(&DBDocumentField::Key.path(), &key, return_fields)
            .await?;
        Ok(result)
    }

    /// Gets a list of documents from the DB by their keys.
    async fn get_many_by_key<
        I: Iterator<Item = <<Self as DBCollection>::Document as DBDocument>::Key> + std::marker::Send,
    >(
        &self,
        iterator: I,
        return_fields: Option<&Self::Document>,
    ) -> Result<Vec<Option<Self::Document>>, anyhow::Error> {
        // Prepare AQL.
        // FOR i IN <keys>
        //     LET o = Document(<collection>, i)
        //     RETURN o
        let document_key = "o";
        let mut aql = AqlBuilder::new_for_in_iterator(AQL_DOCUMENT_ID, iterator);
        aql.let_step(AqlLet {
            variable: document_key,
            expression: AqlLetKind::Expression(
                format!("DOCUMENT({}, {})", Self::name(), AQL_DOCUMENT_ID).into(),
            ),
        });

        if let Some(fields) = return_fields {
            aql.return_step_with_fields(document_key, fields);
        } else {
            aql.return_step(AqlReturn::new_expression(document_key.into()));
        }

        let aql_result = self
            .send_generic_aql::<Option<Self::Document>>(&aql)
            .await?;

        Ok(aql_result.results)
    }

    /// Gets a list of documents from the DB by their keys filtering those that cannot be found.
    async fn get_many_by_key_flattened<
        I: Iterator<Item = <<Self as DBCollection>::Document as DBDocument>::Key> + std::marker::Send,
    >(
        &self,
        iterator: I,
        return_fields: Option<&Self::Document>,
    ) -> Result<Vec<Self::Document>, anyhow::Error> {
        // Prepare AQL.
        // FOR i IN <keys>
        //     LET o = Document(<collection>, i)
        //     FILTER o != null
        //     RETURN o
        let document_key = "o";
        let mut aql = AqlBuilder::new_for_in_iterator(AQL_DOCUMENT_ID, iterator);
        aql.let_step(AqlLet {
            variable: document_key,
            expression: AqlLetKind::Expression(
                format!("DOCUMENT({}, {})", Self::name(), AQL_DOCUMENT_ID).into(),
            ),
        });
        aql.filter_step(format!("{} != null", document_key).into());

        if let Some(fields) = return_fields {
            aql.return_step_with_fields(document_key, fields);
        } else {
            aql.return_step(AqlReturn::new_expression(document_key.into()));
        }

        let aql_result = self.send_aql(&aql).await?;

        Ok(aql_result.results)
    }

    /// Gets a document from the DB by a single custom property.
    ///
    /// `filter_output`: ignores nested values, i.e. A.B or A.B.C.D
    async fn get_one_by<V: Serialize + Send + Sync>(
        &self,
        property_path: &str,
        value: &V,
        return_fields: Option<&Self::Document>,
    ) -> Result<Option<Self::Document>, anyhow::Error> {
        let mut result = self
            .get_many_by(property_path, value, Some(1), return_fields)
            .await?;
        Ok(result.pop())
    }

    /// Gets many documents from the DB by a single custom property.
    ///
    /// `filter_output`: ignores nested values, i.e. A.B or A.B.C.D
    async fn get_many_by<V: Serialize + Send + Sync>(
        &self,
        property_path: &str,
        value: &V,
        limit: Option<u64>,
        return_fields: Option<&Self::Document>,
    ) -> Result<Vec<Self::Document>, anyhow::Error> {
        // Prepare AQL.
        // FOR i IN <collection>
        //      FILTER i.<property> == <value>
        //      LIMIT <limit>
        //      RETURN <return_fields>
        let mut aql = AqlBuilder::new_for_in_collection(AQL_DOCUMENT_ID, Self::name());

        aql.filter_step(
            format!(
                "{}.{} == {}",
                AQL_DOCUMENT_ID,
                property_path,
                serde_json::to_string(value).unwrap()
            )
            .into(),
        );

        if let Some(limit) = limit {
            aql.limit_step(AqlLimit {
                offset: None,
                count: limit,
            });
        }

        if let Some(return_fields) = return_fields {
            aql.return_step_with_fields(AQL_DOCUMENT_ID, return_fields);
        } else {
            aql.return_step(AqlReturn::new_document());
        }

        let result = self.send_aql(&aql).await?;

        Ok(result.results)
    }

    /// Update a list with retries.
    async fn update_list_with_retries(
        &self,
        documents: &[Self::Document],
    ) -> Result<(), anyhow::Error> {
        // FOR i IN <documents>
        //      UPDATE i WITH i IN <collection> OPTIONS { ignoreErrors: true }
        //      RETURN NEW._key
        let collection = self;
        let mut aql = AqlBuilder::new_for_in_list(AQL_DOCUMENT_ID, documents);

        aql.update_step(
            AqlUpdate::new_document(Self::name(), AQL_DOCUMENT_ID.into()).apply_ignore_errors(true),
        );
        aql.return_step(AqlReturn::new_expression(
            format!("{}.{}", AQL_NEW_ID, DBDocumentField::Key.path()).into(),
        ));

        let mut active_keys: HashSet<_> = documents
            .iter()
            .map(|v| v.db_key().clone().unwrap())
            .collect();

        collection
            .send_generic_aql_with_manual_retries(
                &mut aql,
                |aql_result: AqlResult<<<Self as crate::traits::collection::DBCollection>::Document as DBDocument>::Key>, aql| {
                    for key in aql_result.results {
                        active_keys.remove(&key);
                    }

                    if active_keys.is_empty() {
                        true
                    } else {
                        aql.set_list_from_iterator(
                            documents
                                .iter()
                                .filter(|v| active_keys.contains(v.db_key().as_ref().unwrap())),
                        );
                        false
                    }
                },
            )
            .await
    }

    /// Insert many documents.
    async fn insert_many(&self, documents: &[Self::Document]) -> Result<(), anyhow::Error> {
        // FOR i IN <documents>
        //      INSERT i INTO <collection>
        let collection = self;
        let mut aql = AqlBuilder::new_for_in_list(AQL_DOCUMENT_ID, documents);

        aql.insert_step(AqlInsert::new_document(Self::name()));

        collection.send_aql(&aql).await?;

        Ok(())
    }

    /// Sends an AQL command returning current collection's documents.
    async fn send_aql<'a>(
        &self,
        aql: &AqlBuilder<'a>,
    ) -> Result<AqlResult<Self::Document>, anyhow::Error> {
        self.send_generic_aql(aql).await
    }

    /// Sends an AQL command.
    async fn send_generic_aql<'a, R: Send + Sync + for<'de> Deserialize<'de>>(
        &self,
        aql: &AqlBuilder<'a>,
    ) -> Result<AqlResult<R>, anyhow::Error> {
        let db_info = self.db_info();

        let batch_size = aql.batch_size();
        let full_count = aql.full_count();
        let global_limit = aql.global_limit();
        let handle_write_conflicts = aql.handle_write_conflicts();

        let query = aql.build_query();

        'outer: loop {
            let aql_query = AqlQuery::builder()
                .query(&query)
                .bind_vars(aql.vars.clone())
                .options(AqlOptions::builder().full_count(full_count).build());

            let aql_query = if let Some(batch_size) = batch_size {
                aql_query.batch_size(batch_size).build()
            } else {
                aql_query.build()
            };

            let mut response_cursor = match db_info.database.aql_query_batch(aql_query).await {
                Ok(v) => v,
                Err(e) => {
                    if handle_write_conflicts {
                        check_client_is_write_conflict(e)?;
                        continue 'outer;
                    } else {
                        return Err(e.into());
                    }
                }
            };

            if response_cursor.more {
                let mut results: Vec<R> = Vec::with_capacity(global_limit as usize);
                loop {
                    results.extend(response_cursor.result.into_iter());

                    if response_cursor.more {
                        let id = response_cursor.id.as_ref().unwrap();
                        response_cursor = match db_info.database.aql_next_batch(id.as_str()).await {
                            Ok(v) => v,
                            Err(e) => {
                                if handle_write_conflicts {
                                    check_client_is_write_conflict(e)?;
                                    continue 'outer;
                                } else {
                                    return Err(e.into());
                                }
                            }
                        };
                    } else {
                        break;
                    }
                }

                response_cursor.result = results;
            }

            return Ok(response_cursor.into());
        }
    }

    /// Sends an AQL command applying manual retries and returning current collection's documents.
    async fn send_aql_with_manual_retries<'a, F>(
        &self,
        aql: &mut AqlBuilder<'a>,
        checker: F,
    ) -> Result<(), anyhow::Error>
    where
        F: FnMut(AqlResult<Self::Document>, &mut AqlBuilder<'a>) -> bool + Send,
    {
        self.send_generic_aql_with_manual_retries(aql, checker)
            .await
    }

    /// Sends an AQL command applying manual retries.
    async fn send_generic_aql_with_manual_retries<
        'a,
        F,
        R: Send + Sync + for<'de> Deserialize<'de>,
    >(
        &self,
        aql: &mut AqlBuilder<'a>,
        mut checker: F,
    ) -> Result<(), anyhow::Error>
    where
        F: FnMut(AqlResult<R>, &mut AqlBuilder<'a>) -> bool + Send,
    {
        loop {
            let results = self.send_generic_aql(aql).await?;

            if checker(results, aql) {
                return Ok(());
            }
        }
    }

    /// Removes all documents from the collection.
    async fn truncate(&self) -> Result<(), anyhow::Error> {
        let db_info = self.db_collection().await?;
        db_info.truncate().await?;
        Ok(())
    }

    /// Drops the collection.
    async fn drop_collection(&self) -> Result<(), anyhow::Error> {
        let db_info = self.db_collection().await?;
        db_info.drop().await?;
        Ok(())
    }
}
