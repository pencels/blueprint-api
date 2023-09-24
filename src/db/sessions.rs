use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Session {
    #[serde(rename = "_id", with = "bson::serde_helpers::hex_string_as_object_id")]
    pub id: String,
    #[serde(rename = "sessionToken")]
    pub session_token: String,
    #[serde(
        rename = "userId",
        with = "bson::serde_helpers::hex_string_as_object_id"
    )]
    pub user_id: String,
}
