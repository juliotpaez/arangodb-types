use arangors::ClientError;
use std::fmt::Debug;
use std::hash::Hash;

use arangors::document::options::{InsertOptions, OverwriteMode, RemoveOptions, UpdateOptions};
use arangors::document::response::DocumentResponse;
use arcstr::ArcStr;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::traits::utils::check_client_is_write_conflict;
use crate::traits::AQLMapping;
use crate::traits::DBCollection;
use crate::types::DBId;

#[async_trait]
pub trait DBDocument:
    Send + Sync + Clone + Serialize + for<'de> Deserialize<'de> + AQLMapping
{
    type Key: Debug
        + ToString
        + Eq
        + PartialEq
        + Clone
        + Hash
        + Send
        + Sync
        + Serialize
        + for<'de> Deserialize<'de>;
    type CollectionType: ToString + Send + Sync;
    type Collection: DBCollection<Document = Self>;

    // GETTERS ----------------------------------------------------------------

    fn db_id(&self) -> Option<DBId<Self::Key, Self::CollectionType>>;

    fn db_key(&self) -> &Option<Self::Key>;

    fn db_rev(&self) -> &Option<ArcStr>;

    /// Whether all the fields are missing or not.
    fn is_all_missing(&self) -> bool;

    /// Whether all the fields are null or not.
    fn is_all_null(&self) -> bool;

    /// Whether all the fields are null or missing or not.
    fn is_all_null_or_missing(&self) -> bool;

    // SETTERS ----------------------------------------------------------------

    fn set_db_key(&mut self, value: Option<Self::Key>);

    // METHODS ----------------------------------------------------------------

    /// Maps all fields that contain a value into a null.
    fn map_values_to_null(&mut self);

    /// Inserts a new document.
    ///
    /// WARN: returns the whole document.
    async fn insert(
        mut self,
        overwrite: bool,
        collection: &Self::Collection,
    ) -> Result<Self, anyhow::Error> {
        let db_collection = collection.db_collection().await?;

        loop {
            let response = if overwrite {
                db_collection
                    .create_document(
                        self.clone(),
                        InsertOptions::builder()
                            .return_new(true)
                            .return_old(false)
                            .keep_null(false)
                            .overwrite(true)
                            .overwrite_mode(OverwriteMode::Replace)
                            .build(),
                    )
                    .await
            } else {
                db_collection
                    .create_document(
                        self.clone(),
                        InsertOptions::builder()
                            .return_new(true)
                            .return_old(false)
                            .keep_null(false)
                            .overwrite(false)
                            .build(),
                    )
                    .await
            };

            match response {
                Ok(v) => match v {
                    DocumentResponse::Silent => unreachable!("Not silent insert!"),
                    DocumentResponse::Response { new, .. } => return Ok(new.unwrap()),
                },
                Err(e) => {
                    check_client_is_write_conflict(e)?;
                }
            }
        }
    }

    /// Inserts a new document ignoring the result.
    async fn insert_and_ignore(
        mut self,
        overwrite: bool,
        collection: &Self::Collection,
    ) -> Result<Self::Key, anyhow::Error> {
        let db_collection = collection.db_collection().await?;

        loop {
            let response = if overwrite {
                db_collection
                    .create_document(
                        self.clone(),
                        InsertOptions::builder()
                            .return_new(false)
                            .return_old(false)
                            .keep_null(false)
                            .overwrite(true)
                            .overwrite_mode(OverwriteMode::Replace)
                            .build(),
                    )
                    .await
            } else {
                db_collection
                    .create_document(
                        self.clone(),
                        InsertOptions::builder()
                            .return_new(false)
                            .return_old(false)
                            .keep_null(false)
                            .overwrite(false)
                            .build(),
                    )
                    .await
            };

            match response {
                Ok(_) => return Ok(self.db_key().clone().unwrap()),
                Err(error) => {
                    check_client_is_write_conflict(error)?;
                }
            }
        }
    }

    /// Updates the element and returns its updated value.
    ///
    /// WARN: returns the whole document.
    async fn update(
        &self,
        merge_objects: bool,
        collection: &Self::Collection,
    ) -> Result<Self, anyhow::Error> {
        let db_collection = collection.db_collection().await?;

        let ignore_rev = self.db_rev().is_none();

        let key = self
            .db_key()
            .as_ref()
            .unwrap_or_else(|| {
                panic!(
                    "You forgot to include the key property in the {} document",
                    Self::Collection::name()
                )
            })
            .to_string();
        let key = urlencoding::encode(key.as_str());

        loop {
            let response = db_collection
                .update_document(
                    &key,
                    self.clone(),
                    UpdateOptions::builder()
                        .merge_objects(merge_objects)
                        .keep_null(false)
                        .return_new(true)
                        .ignore_revs(ignore_rev)
                        .build(),
                )
                .await;

            match response {
                Ok(v) => match v {
                    DocumentResponse::Silent => unreachable!("This update is not silent"),
                    DocumentResponse::Response { new, .. } => return Ok(new.unwrap()),
                },
                Err(e) => {
                    check_client_is_write_conflict(e)?;
                }
            }
        }
    }

    /// Updates the element ignoring the result.
    async fn update_and_ignore(
        &self,
        merge_objects: bool,
        collection: &Self::Collection,
    ) -> Result<(), anyhow::Error> {
        let db_collection = collection.db_collection().await?;

        let ignore_rev = self.db_rev().is_none();

        let key = self
            .db_key()
            .as_ref()
            .unwrap_or_else(|| {
                panic!(
                    "You forgot to include the key property in the {} document",
                    Self::Collection::name()
                )
            })
            .to_string();
        let key = urlencoding::encode(key.as_str());

        loop {
            let response = db_collection
                .update_document(
                    &key,
                    self.clone(),
                    UpdateOptions::builder()
                        .merge_objects(merge_objects)
                        .keep_null(false)
                        .ignore_revs(ignore_rev)
                        .silent(true)
                        .build(),
                )
                .await;

            match response {
                Ok(_) => return Ok(()),
                Err(e) => {
                    check_client_is_write_conflict(e)?;
                }
            }
        }
    }

    /// Inserts a new document or updates it if it already exists.
    ///
    /// WARN: returns the whole document.
    async fn insert_or_update(
        mut self,
        merge_objects: bool,
        collection: &Self::Collection,
    ) -> Result<Self, anyhow::Error> {
        let db_collection = collection.db_collection().await?;

        loop {
            let response = db_collection
                .create_document(
                    self.clone(),
                    InsertOptions::builder()
                        .return_new(true)
                        .return_old(false)
                        .overwrite(true)
                        .overwrite_mode(OverwriteMode::Update)
                        .keep_null(false)
                        .merge_objects(merge_objects)
                        .build(),
                )
                .await;

            match response {
                Ok(v) => match v {
                    DocumentResponse::Silent => unreachable!("Not silent insert!"),
                    DocumentResponse::Response { new, .. } => return Ok(new.unwrap()),
                },
                Err(e) => {
                    check_client_is_write_conflict(e)?;
                }
            }
        }
    }

    /// Inserts a new document or updates it if it already exists, ignoring the result.
    async fn insert_or_update_and_ignore(
        mut self,
        merge_objects: bool,
        collection: &Self::Collection,
    ) -> Result<Self::Key, anyhow::Error> {
        let db_collection = collection.db_collection().await?;

        loop {
            let response = db_collection
                .create_document(
                    self.clone(),
                    InsertOptions::builder()
                        .return_new(false)
                        .return_old(false)
                        .overwrite(true)
                        .overwrite_mode(OverwriteMode::Update)
                        .keep_null(false)
                        .merge_objects(merge_objects)
                        .silent(true)
                        .build(),
                )
                .await;

            match response {
                Ok(_) => return Ok(self.db_key().clone().unwrap()),
                Err(error) => {
                    check_client_is_write_conflict(error)?;
                }
            }
        }
    }

    /// Removes the element returning the old value.
    async fn remove(
        &self,
        error_if_not_found: bool,
        collection: &Self::Collection,
    ) -> Result<Option<Self>, anyhow::Error> {
        let db_collection = collection.db_collection().await?;

        let key = self
            .db_key()
            .as_ref()
            .unwrap_or_else(|| {
                panic!(
                    "You forgot to include the key property in the {} document",
                    Self::Collection::name()
                )
            })
            .to_string();
        let key = urlencoding::encode(key.as_str());

        loop {
            let response = db_collection
                .remove_document(
                    &key,
                    RemoveOptions::builder()
                        .return_old(true)
                        .silent(false)
                        .build(),
                    None,
                )
                .await;

            match response {
                Ok(v) => match v {
                    DocumentResponse::Silent => unreachable!("This remove is not silent"),
                    DocumentResponse::Response { old, .. } => return Ok(Some(old.unwrap())),
                },
                Err(e) => {
                    if !error_if_not_found {
                        if let ClientError::Arango(e) = &e {
                            if e.error_num() == 1202 {
                                return Ok(None);
                            }
                        }
                    }

                    check_client_is_write_conflict(e)?;
                }
            }
        }
    }

    /// Removes the element ignoring the result.
    async fn remove_and_ignore(
        &self,
        error_if_not_found: bool,
        collection: &Self::Collection,
    ) -> Result<(), anyhow::Error> {
        let db_collection = collection.db_collection().await?;

        let key = self
            .db_key()
            .as_ref()
            .unwrap_or_else(|| {
                panic!(
                    "You forgot to include the key property in the {} document",
                    Self::Collection::name()
                )
            })
            .to_string();
        let key = urlencoding::encode(key.as_str());

        loop {
            let response = db_collection
                .remove_document::<()>(
                    &key,
                    RemoveOptions::builder()
                        .return_old(false)
                        .silent(true)
                        .build(),
                    None,
                )
                .await;

            match response {
                Ok(_) => return Ok(()),
                Err(e) => {
                    if !error_if_not_found {
                        if let ClientError::Arango(e) = &e {
                            if e.error_num() == 1202 {
                                return Ok(());
                            }
                        }
                    }

                    check_client_is_write_conflict(e)?;
                }
            }
        }
    }
}
