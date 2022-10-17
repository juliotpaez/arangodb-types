use std::borrow::Cow;
use std::collections::HashMap;

use arangors::uclient::reqwest::ReqwestClient;
use arangors::{ClientError, GenericConnection};
use serde::Deserialize;
use serde::Serialize;

use crate::traits::utils::check_client_is_write_conflict;

pub type Database = arangors::Database<ReqwestClient>;
pub type Collection = arangors::Collection<ReqwestClient>;

/// The database information.
#[derive(Debug)]
pub struct DBInfo {
    pub username: Cow<'static, str>,
    pub password: Cow<'static, str>,
    pub connection: GenericConnection<ReqwestClient>,
    pub database: Database,
}

impl DBInfo {
    // CONSTRUCTORS -----------------------------------------------------------

    pub async fn connect(
        url: Cow<'static, str>,
        database: Cow<'static, str>,
        username: Cow<'static, str>,
        password: Cow<'static, str>,
    ) -> Result<DBInfo, anyhow::Error> {
        let connection = GenericConnection::establish_jwt(&url, &username, &password).await?;

        let database = match connection.create_database(&database).await {
            Ok(v) => v,
            Err(_) => connection.db(&database).await?,
        };

        Ok(DBInfo {
            username,
            password,
            connection,
            database,
        })
    }

    pub fn new(
        username: Cow<'static, str>,
        password: Cow<'static, str>,
        connection: GenericConnection<ReqwestClient>,
        database: Database,
    ) -> DBInfo {
        Self {
            username,
            password,
            connection,
            database,
        }
    }

    // METHODS ----------------------------------------------------------------

    pub async fn send_aql_with_retries<T: for<'de> Deserialize<'de>>(
        &self,
        query: &str,
        bind_vars: HashMap<&str, serde_json::Value>,
    ) -> Result<Vec<T>, ClientError> {
        loop {
            match self.database.aql_bind_vars(query, bind_vars.clone()).await {
                Ok(v) => return Ok(v),
                Err(e) => check_client_is_write_conflict(e)?,
            };
        }
    }

    pub async fn add_aql_function(
        &self,
        name: &str,
        code: &str,
        is_deterministic: bool,
    ) -> Result<(), anyhow::Error> {
        let client = self.connection.session();
        let response = client
            .client
            .post(format!("{}_api/aqlfunction", self.database.url().as_str()))
            .basic_auth(&self.username, Some(&self.password))
            .json(&AddFunctionRequest {
                name,
                code,
                is_deterministic,
            })
            .send()
            .await?;

        match response.status().as_u16() {
            200 | 201 => Ok(()),
            _ => {
                let text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "<undefined>".to_string());
                Err(anyhow::anyhow!(text))
            }
        }
    }

    pub async fn remove_all_aql_function(&self, namespace: &str) -> Result<(), anyhow::Error> {
        let client = self.connection.session();
        let response = client
            .client
            .delete(format!(
                "{}_api/aqlfunction/{}?group=true",
                self.database.url().as_str(),
                namespace,
            ))
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?;

        match response.status().as_u16() {
            200 => Ok(()),
            _ => {
                let text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "<undefined>".to_string());
                Err(anyhow::anyhow!(text))
            }
        }
    }
}

// ----------------------------------------------------------------------------
// ----------------------------------------------------------------------------
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AddFunctionRequest<'a> {
    name: &'a str,
    code: &'a str,
    is_deterministic: bool,
}
