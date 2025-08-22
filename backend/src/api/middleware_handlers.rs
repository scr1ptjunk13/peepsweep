use axum::{
    extract::Request,
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use crate::ApiError;

// Middleware functions are defined in the parent module (mod.rs)
// This file provides additional middleware handlers

// Additional middleware handlers can be added here
pub async fn cors_middleware(request: Request, next: Next) -> Response {
    let response = next.run(request).await;
    // CORS headers are handled by tower_http::cors::CorsLayer
    response
}

pub async fn security_headers_middleware(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    
    let headers = response.headers_mut();
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());
    
    response
}
