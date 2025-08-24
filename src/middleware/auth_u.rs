use std::future::{Ready, ready};

use actix_web::{
    Error, HttpMessage,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::Method,
};
use futures_util::future::LocalBoxFuture;

use crate::auth::{Role, validate_jwt};

// There are two steps in middleware processing.
// 1. Middleware initialization, middleware factory gets called with
//    next service in chain as parameter.
// 2. Middleware's call method gets called with normal request.
pub struct UserGuardMiddleWare;

// Middleware factory is `Transform` trait
// `S` - type of the next service
// `B` - type of response's body
impl<S, B> Transform<S, ServiceRequest> for UserGuardMiddleWare
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = JwtGuardService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(JwtGuardService { service }))
    }
}

pub struct JwtGuardService<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for JwtGuardService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        if req.method() == Method::OPTIONS {
            let fut = self.service.call(req);
            return Box::pin(async move { fut.await });
        }
        if let Some(auth_header) = req.headers().get("Authorization") {
            let token = String::from_utf8_lossy(auth_header.as_bytes());
            if token.starts_with("Bearer ") {
                let jwt = token.trim_start_matches("Bearer ").trim().to_string();

                if let Ok(claims) = validate_jwt(jwt) {
                    if claims.role == Role::User {
                        req.extensions_mut().insert(claims.sub);

                        let fut = self.service.call(req);

                        return Box::pin(async move {
                            let res = fut.await?;
                            Ok(res)
                        });
                    }
                }
            }
        }

        Box::pin(async move { Err(actix_web::error::ErrorUnauthorized("Unauthorized")) })
    }
}
