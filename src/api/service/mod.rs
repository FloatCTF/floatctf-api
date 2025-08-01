mod super_admin;
mod users;
use actix_web::web::{ServiceConfig, scope};

use crate::api::service::super_admin::super_admin_login;
pub fn config(cfg: &mut ServiceConfig) {
    cfg.service(super_admin_login);

    cfg.service(scope("/users").service(users::user_login));
}
