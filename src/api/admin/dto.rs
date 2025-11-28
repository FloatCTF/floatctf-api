use crate::api::preclude::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteItemsRequest {
    pub id_list: Vec<Uuid>,
}
