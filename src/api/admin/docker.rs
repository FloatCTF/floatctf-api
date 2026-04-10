use std::collections::HashMap;

use bollard::query_parameters::ListContainersOptionsBuilder;

use crate::{
    api::prelude::*,
    auth::SuperAdminJwtGuard,
    db::WebDocker,
    entity::{instances, sea_orm_active_enums::InstanceStatus},
};
// /**
//  *  容器名称
//  *  题目名称
//  *  比赛名称
//  *  IP 地址和 端口
//  *  状态
//  * 镜像名称
//  *  运行时长
//  *
//  */
#[derive(Debug, Serialize, Deserialize)]
pub struct FloatDockerContainer {
    pub name: String,
    pub challenge_name: String,
    pub event_name: Option<String>,
    pub net_info: String,
    pub status: String,
    pub image_name: String,
    pub uptime: String,
}
#[get("/containers")]
pub async fn get_containers(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    docker: WebDocker,
) -> UniResult<()> {
    let instances = instances::Entity::find()
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .all(db.get_ref())
        .await
        .unwrap()
        .into_iter()
        .map(|i| i.identifier)
        .collect::<Vec<String>>();
    dbg!(&instances);
    let mut f = HashMap::new();
    f.insert("name", instances);

    let lc = ListContainersOptionsBuilder::new().filters(&f).build();
    let c = docker.list_containers(Some(lc)).await.unwrap();
    UniResponse::ok_none().into()
}
pub struct FloatDockerImage {}
pub struct FloatDockerNetwork {}

// #[actix_web::test]
// pub async fn test_docker() {
//     dotenv().ok();
//     let db = init_db().await.unwrap();
//     let docker = init_docker().await.unwrap();
//     let instances = instances::Entity::find()
//         .filter(instances::Column::Status.eq(InstanceStatus::Running))
//         .all(&db)
//         .await
//         .unwrap()
//         .into_iter()
//         .map(|i| i.identifier)
//         .collect::<Vec<String>>();
//     dbg!(&instances);
//     let mut f = HashMap::new();
//     f.insert("name", instances);

//     let lc = ListContainersOptionsBuilder::new().filters(&f).build();
//     let c = docker.list_containers(Some(lc)).await.unwrap();
//     dbg!(c);
// }
