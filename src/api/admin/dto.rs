use crate::api::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteItemsRequest {
    pub id_list: Vec<Uuid>,
}
