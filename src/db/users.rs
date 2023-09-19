use bson::doc;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::models::NewUser;
use crate::models::UpdateUser;
use crate::util::Result;

use super::{get_entities, DateTime};

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

impl From<crate::models::User> for User {
    fn from(user: crate::models::User) -> Self {
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

impl From<User> for crate::models::User {
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

pub async fn create_new_user(client: &mongodb::Client, new_user: NewUser) -> Result<()> {
    let user: User = new_user.into();

    client
        .default_database()
        .unwrap()
        .collection::<User>(USERS_TABLE)
        .insert_one(&user, None)
        .await?;

    Ok(())
}

pub async fn get_users(client: &mongodb::Client, page: usize) -> Result<Vec<crate::models::User>> {
    get_entities::<User, crate::models::User>(client, USERS_TABLE, page).await
}

pub async fn get_user(client: &mongodb::Client, id: &str) -> Result<Option<crate::models::User>> {
    let response = client
        .default_database()
        .unwrap()
        .collection::<User>(USERS_TABLE)
        .find_one(doc! { "_id": id }, None)
        .await?;
    Ok(response.map(|e| e.into()))
}

pub async fn update_user(client: &mongodb::Client, id: &str, update: UpdateUser) -> Result<()> {
    let modifications = doc! {
        "username": update.username,
        "display_name": update.display_name,
    };

    client
        .default_database()
        .unwrap()
        .collection::<User>(USERS_TABLE)
        .update_one(doc! { "_id": id }, modifications, None)
        .await?;

    Ok(())
}

pub async fn delete_user(client: &mongodb::Client, id: &str) -> Result<()> {
    client
        .default_database()
        .unwrap()
        .collection::<User>(USERS_TABLE)
        .delete_one(doc! { "_id": id }, None)
        .await?;
    Ok(())
}
