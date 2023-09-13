use azure_data_tables::{operations::InsertEntityResponse, prelude::TableServiceClient};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::models::users::NewUser;
use crate::util::Result;

use super::DateTime;

const USERS_TABLE: &str = "users";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    #[serde(rename = "PartitionKey")]
    pub partition_key: String,
    #[serde(rename = "RowKey")]
    pub row_key: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub created: DateTime,
    #[serde(
        rename = "created@odata.type",
        serialize_with = "super::edm_datetime",
        skip_deserializing
    )]
    _created_tag: (),
}

impl From<crate::models::users::User> for User {
    fn from(user: crate::models::users::User) -> Self {
        User {
            partition_key: user.id.clone(),
            row_key: user.id,
            avatar_url: user.avatar_url,
            created: user.created.into(),
            username: user.username,
            display_name: user.display_name,
            _created_tag: (),
        }
    }
}

impl From<User> for crate::models::users::User {
    fn from(db_user: User) -> Self {
        Self {
            avatar_url: db_user.avatar_url,
            id: db_user.row_key,
            username: db_user.username,
            created: db_user.created.into(),
            display_name: db_user.display_name,
        }
    }
}

impl From<NewUser> for User {
    fn from(new_user: NewUser) -> Self {
        let id = uuid::Uuid::new_v4();
        User {
            username: new_user.username.clone(),
            display_name: new_user.username,
            avatar_url: None,
            created: OffsetDateTime::now_utc().into(),
            row_key: id.to_string(),
            partition_key: id.to_string(),
            _created_tag: (),
        }
    }
}

// User Table API Functions

pub async fn create_new_user(
    client: &TableServiceClient,
    new_user: NewUser,
) -> Result<crate::models::User> {
    let user: User = new_user.into();

    let response: InsertEntityResponse<User> = client
        .table_client(USERS_TABLE)
        .insert(&user)?
        .return_entity(true)
        .await?;

    response
        .entity_with_metadata
        .map(|e| e.entity.into())
        .ok_or("User create failed".into())
}
