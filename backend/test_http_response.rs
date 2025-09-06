use axum::{
    response::Json,
    routing::get,
    Router,
};
use serde_json::json;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Simple test server to isolate HTTP response issue
    let app = Router::new()
        .route("/test", get(test_handler))
        .route("/test-json", get(test_json_handler))
        .route("/test-string", get(test_string_handler));

    println!("🧪 Starting test HTTP server on 127.0.0.1:3001");
    
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3001").await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn test_handler() -> &'static str {
    println!("Test handler called");
    "Hello World"
}

async fn test_json_handler() -> Json<serde_json::Value> {
    println!("JSON test handler called");
    Json(json!({
        "status": "success",
        "message": "JSON response working",
        "data": {
            "test": true,
            "number": 42
        }
    }))
}

async fn test_string_handler() -> String {
    println!("String test handler called");
    "String response working".to_string()
}
