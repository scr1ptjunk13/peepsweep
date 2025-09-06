use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub struct OpenApiGenerator;

impl OpenApiGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn generate_spec(&self) -> OpenApiSpec {
        generate_bridge_api_spec()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenApiSpec {
    pub openapi: String,
    pub info: ApiInfo,
    pub servers: Vec<Server>,
    pub paths: HashMap<String, PathItem>,
    pub components: Components,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiInfo {
    pub title: String,
    pub description: String,
    pub version: String,
    pub contact: Contact,
    pub license: License,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub name: String,
    pub email: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct License {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub url: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathItem {
    pub get: Option<Operation>,
    pub post: Option<Operation>,
    pub put: Option<Operation>,
    pub delete: Option<Operation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub tags: Vec<String>,
    pub summary: String,
    pub description: String,
    pub parameters: Option<Vec<Parameter>>,
    #[serde(rename = "requestBody")]
    pub request_body: Option<RequestBody>,
    pub responses: HashMap<String, Response>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "in")]
    pub location: String,
    pub required: bool,
    pub description: String,
    pub schema: Schema,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestBody {
    pub description: String,
    pub required: bool,
    pub content: HashMap<String, MediaType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub description: String,
    pub content: Option<HashMap<String, MediaType>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaType {
    pub schema: Schema,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    #[serde(rename = "type")]
    pub schema_type: Option<String>,
    pub format: Option<String>,
    pub properties: Option<HashMap<String, Schema>>,
    pub items: Option<Box<Schema>>,
    #[serde(rename = "$ref")]
    pub reference: Option<String>,
    pub required: Option<Vec<String>>,
    pub example: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Components {
    pub schemas: HashMap<String, Schema>,
}

pub fn generate_bridge_api_spec() -> OpenApiSpec {
    let mut paths = HashMap::new();
    let mut schemas = HashMap::new();

    // Define schemas
    schemas.insert("CrossChainParams".to_string(), Schema {
        schema_type: Some("object".to_string()),
        format: None,
        properties: Some({
            let mut props = HashMap::new();
            props.insert("from_chain_id".to_string(), Schema {
                schema_type: Some("integer".to_string()),
                format: Some("int64".to_string()),
                properties: None,
                items: None,
                reference: None,
                required: None,
                example: Some(serde_json::json!(1)),
            });
            props.insert("to_chain_id".to_string(), Schema {
                schema_type: Some("integer".to_string()),
                format: Some("int64".to_string()),
                properties: None,
                items: None,
                reference: None,
                required: None,
                example: Some(serde_json::json!(137)),
            });
            props.insert("token_in".to_string(), Schema {
                schema_type: Some("string".to_string()),
                format: None,
                properties: None,
                items: None,
                reference: None,
                required: None,
                example: Some(serde_json::json!("USDC")),
            });
            props.insert("token_out".to_string(), Schema {
                schema_type: Some("string".to_string()),
                format: None,
                properties: None,
                items: None,
                reference: None,
                required: None,
                example: Some(serde_json::json!("USDC")),
            });
            props.insert("amount_in".to_string(), Schema {
                schema_type: Some("string".to_string()),
                format: None,
                properties: None,
                items: None,
                reference: None,
                required: None,
                example: Some(serde_json::json!("1000000")),
            });
            props.insert("user_address".to_string(), Schema {
                schema_type: Some("string".to_string()),
                format: None,
                properties: None,
                items: None,
                reference: None,
                required: None,
                example: Some(serde_json::json!("0x742d35Cc6634C0532925a3b8D8f8b8f8b8f8b8f8")),
            });
            props.insert("slippage".to_string(), Schema {
                schema_type: Some("number".to_string()),
                format: Some("double".to_string()),
                properties: None,
                items: None,
                reference: None,
                required: None,
                example: Some(serde_json::json!(0.005)),
            });
            props
        }),
        items: None,
        reference: None,
        required: Some(vec![
            "from_chain_id".to_string(),
            "to_chain_id".to_string(),
            "token_in".to_string(),
            "token_out".to_string(),
            "amount_in".to_string(),
            "user_address".to_string(),
            "slippage".to_string(),
        ]),
        example: None,
    });

    schemas.insert("BridgeQuote".to_string(), Schema {
        schema_type: Some("object".to_string()),
        format: None,
        properties: Some({
            let mut props = HashMap::new();
            props.insert("bridge_name".to_string(), Schema {
                schema_type: Some("string".to_string()),
                format: None,
                properties: None,
                items: None,
                reference: None,
                required: None,
                example: Some(serde_json::json!("Hop Protocol")),
            });
            props.insert("amount_out".to_string(), Schema {
                schema_type: Some("string".to_string()),
                format: None,
                properties: None,
                items: None,
                reference: None,
                required: None,
                example: Some(serde_json::json!("999000")),
            });
            props.insert("fee".to_string(), Schema {
                schema_type: Some("string".to_string()),
                format: None,
                properties: None,
                items: None,
                reference: None,
                required: None,
                example: Some(serde_json::json!("1000")),
            });
            props.insert("estimated_time".to_string(), Schema {
                schema_type: Some("integer".to_string()),
                format: Some("int64".to_string()),
                properties: None,
                items: None,
                reference: None,
                required: None,
                example: Some(serde_json::json!(900)),
            });
            props.insert("confidence_score".to_string(), Schema {
                schema_type: Some("number".to_string()),
                format: Some("double".to_string()),
                properties: None,
                items: None,
                reference: None,
                required: None,
                example: Some(serde_json::json!(0.85)),
            });
            props
        }),
        items: None,
        reference: None,
        required: Some(vec![
            "bridge_name".to_string(),
            "amount_out".to_string(),
            "fee".to_string(),
            "estimated_time".to_string(),
            "confidence_score".to_string(),
        ]),
        example: None,
    });

    schemas.insert("QuoteResponse".to_string(), Schema {
        schema_type: Some("object".to_string()),
        format: None,
        properties: Some({
            let mut props = HashMap::new();
            props.insert("quotes".to_string(), Schema {
                schema_type: Some("array".to_string()),
                format: None,
                properties: None,
                items: Some(Box::new(Schema {
                    schema_type: None,
                    format: None,
                    properties: None,
                    items: None,
                    reference: Some("#/components/schemas/BridgeQuote".to_string()),
                    required: None,
                    example: None,
                })),
                reference: None,
                required: None,
                example: None,
            });
            props.insert("best_quote".to_string(), Schema {
                schema_type: None,
                format: None,
                properties: None,
                items: None,
                reference: Some("#/components/schemas/BridgeQuote".to_string()),
                required: None,
                example: None,
            });
            props.insert("supported_routes".to_string(), Schema {
                schema_type: Some("array".to_string()),
                format: None,
                properties: None,
                items: Some(Box::new(Schema {
                    schema_type: Some("array".to_string()),
                    format: None,
                    properties: None,
                    items: Some(Box::new(Schema {
                        schema_type: Some("integer".to_string()),
                        format: Some("int64".to_string()),
                        properties: None,
                        items: None,
                        reference: None,
                        required: None,
                        example: None,
                    })),
                    reference: None,
                    required: None,
                    example: None,
                })),
                reference: None,
                required: None,
                example: None,
            });
            props
        }),
        items: None,
        reference: None,
        required: Some(vec![
            "quotes".to_string(),
            "supported_routes".to_string(),
        ]),
        example: None,
    });

    schemas.insert("BridgeResponse".to_string(), Schema {
        schema_type: Some("object".to_string()),
        format: None,
        properties: Some({
            let mut props = HashMap::new();
            props.insert("transaction_hash".to_string(), Schema {
                schema_type: Some("string".to_string()),
                format: None,
                properties: None,
                items: None,
                reference: None,
                required: None,
                example: Some(serde_json::json!("0x1234567890abcdef...")),
            });
            props.insert("bridge_id".to_string(), Schema {
                schema_type: Some("string".to_string()),
                format: None,
                properties: None,
                items: None,
                reference: None,
                required: None,
                example: Some(serde_json::json!("hop_1234567890")),
            });
            props.insert("status".to_string(), Schema {
                schema_type: Some("string".to_string()),
                format: None,
                properties: None,
                items: None,
                reference: None,
                required: None,
                example: Some(serde_json::json!("Pending")),
            });
            props.insert("estimated_completion".to_string(), Schema {
                schema_type: Some("integer".to_string()),
                format: Some("int64".to_string()),
                properties: None,
                items: None,
                reference: None,
                required: None,
                example: Some(serde_json::json!(1640995200)),
            });
            props
        }),
        items: None,
        reference: None,
        required: Some(vec![
            "transaction_hash".to_string(),
            "bridge_id".to_string(),
            "status".to_string(),
        ]),
        example: None,
    });

    schemas.insert("HealthResponse".to_string(), Schema {
        schema_type: Some("object".to_string()),
        format: None,
        properties: Some({
            let mut props = HashMap::new();
            props.insert("status".to_string(), Schema {
                schema_type: Some("string".to_string()),
                format: None,
                properties: None,
                items: None,
                reference: None,
                required: None,
                example: Some(serde_json::json!("healthy")),
            });
            props.insert("bridges".to_string(), Schema {
                schema_type: Some("object".to_string()),
                format: None,
                properties: None,
                items: None,
                reference: None,
                required: None,
                example: Some(serde_json::json!({
                    "Hop Protocol": true,
                    "Across Protocol": true,
                    "Stargate Finance": true
                })),
            });
            props.insert("supported_chains".to_string(), Schema {
                schema_type: Some("array".to_string()),
                format: None,
                properties: None,
                items: Some(Box::new(Schema {
                    schema_type: Some("integer".to_string()),
                    format: Some("int64".to_string()),
                    properties: None,
                    items: None,
                    reference: None,
                    required: None,
                    example: None,
                })),
                reference: None,
                required: None,
                example: None,
            });
            props.insert("total_routes".to_string(), Schema {
                schema_type: Some("integer".to_string()),
                format: None,
                properties: None,
                items: None,
                reference: None,
                required: None,
                example: Some(serde_json::json!(42)),
            });
            props
        }),
        items: None,
        reference: None,
        required: Some(vec![
            "status".to_string(),
            "bridges".to_string(),
            "supported_chains".to_string(),
            "total_routes".to_string(),
        ]),
        example: None,
    });

    // Define paths
    paths.insert("/bridge/quote".to_string(), PathItem {
        get: Some(Operation {
            tags: vec!["Bridge".to_string()],
            summary: "Get cross-chain bridge quotes".to_string(),
            description: "Retrieve quotes from multiple bridge providers for a cross-chain transfer".to_string(),
            parameters: Some(vec![
                Parameter {
                    name: "from_chain_id".to_string(),
                    location: "query".to_string(),
                    required: true,
                    description: "Source chain ID".to_string(),
                    schema: Schema {
                        schema_type: Some("integer".to_string()),
                        format: Some("int64".to_string()),
                        properties: None,
                        items: None,
                        reference: None,
                        required: None,
                        example: Some(serde_json::json!(1)),
                    },
                },
                Parameter {
                    name: "to_chain_id".to_string(),
                    location: "query".to_string(),
                    required: true,
                    description: "Destination chain ID".to_string(),
                    schema: Schema {
                        schema_type: Some("integer".to_string()),
                        format: Some("int64".to_string()),
                        properties: None,
                        items: None,
                        reference: None,
                        required: None,
                        example: Some(serde_json::json!(137)),
                    },
                },
                Parameter {
                    name: "token_in".to_string(),
                    location: "query".to_string(),
                    required: true,
                    description: "Input token symbol".to_string(),
                    schema: Schema {
                        schema_type: Some("string".to_string()),
                        format: None,
                        properties: None,
                        items: None,
                        reference: None,
                        required: None,
                        example: Some(serde_json::json!("USDC")),
                    },
                },
                Parameter {
                    name: "token_out".to_string(),
                    location: "query".to_string(),
                    required: true,
                    description: "Output token symbol".to_string(),
                    schema: Schema {
                        schema_type: Some("string".to_string()),
                        format: None,
                        properties: None,
                        items: None,
                        reference: None,
                        required: None,
                        example: Some(serde_json::json!("USDC")),
                    },
                },
                Parameter {
                    name: "amount_in".to_string(),
                    location: "query".to_string(),
                    required: true,
                    description: "Input amount in token's smallest unit".to_string(),
                    schema: Schema {
                        schema_type: Some("string".to_string()),
                        format: None,
                        properties: None,
                        items: None,
                        reference: None,
                        required: None,
                        example: Some(serde_json::json!("1000000")),
                    },
                },
                Parameter {
                    name: "user_address".to_string(),
                    location: "query".to_string(),
                    required: true,
                    description: "User's wallet address".to_string(),
                    schema: Schema {
                        schema_type: Some("string".to_string()),
                        format: None,
                        properties: None,
                        items: None,
                        reference: None,
                        required: None,
                        example: Some(serde_json::json!("0x742d35Cc6634C0532925a3b8D8f8b8f8b8f8b8f8")),
                    },
                },
                Parameter {
                    name: "slippage".to_string(),
                    location: "query".to_string(),
                    required: false,
                    description: "Maximum slippage tolerance (default: 0.005)".to_string(),
                    schema: Schema {
                        schema_type: Some("number".to_string()),
                        format: Some("double".to_string()),
                        properties: None,
                        items: None,
                        reference: None,
                        required: None,
                        example: Some(serde_json::json!(0.005)),
                    },
                },
            ]),
            request_body: None,
            responses: {
                let mut responses = HashMap::new();
                responses.insert("200".to_string(), Response {
                    description: "Successful quote response".to_string(),
                    content: Some({
                        let mut content = HashMap::new();
                        content.insert("application/json".to_string(), MediaType {
                            schema: Schema {
                                schema_type: None,
                                format: None,
                                properties: None,
                                items: None,
                                reference: Some("#/components/schemas/QuoteResponse".to_string()),
                                required: None,
                                example: None,
                            },
                        });
                        content
                    }),
                });
                responses.insert("400".to_string(), Response {
                    description: "Bad request - invalid parameters".to_string(),
                    content: None,
                });
                responses.insert("500".to_string(), Response {
                    description: "Internal server error".to_string(),
                    content: None,
                });
                responses
            },
        }),
        post: None,
        put: None,
        delete: None,
    });

    paths.insert("/bridge/execute".to_string(), PathItem {
        get: None,
        post: Some(Operation {
            tags: vec!["Bridge".to_string()],
            summary: "Execute cross-chain bridge transfer".to_string(),
            description: "Execute a cross-chain transfer using the best available bridge".to_string(),
            parameters: None,
            request_body: Some(RequestBody {
                description: "Cross-chain transfer parameters".to_string(),
                required: true,
                content: {
                    let mut content = HashMap::new();
                    content.insert("application/json".to_string(), MediaType {
                        schema: Schema {
                            schema_type: None,
                            format: None,
                            properties: None,
                            items: None,
                            reference: Some("#/components/schemas/CrossChainParams".to_string()),
                            required: None,
                            example: None,
                        },
                    });
                    content
                },
            }),
            responses: {
                let mut responses = HashMap::new();
                responses.insert("200".to_string(), Response {
                    description: "Successful execution response".to_string(),
                    content: Some({
                        let mut content = HashMap::new();
                        content.insert("application/json".to_string(), MediaType {
                            schema: Schema {
                                schema_type: None,
                                format: None,
                                properties: None,
                                items: None,
                                reference: Some("#/components/schemas/BridgeResponse".to_string()),
                                required: None,
                                example: None,
                            },
                        });
                        content
                    }),
                });
                responses.insert("400".to_string(), Response {
                    description: "Bad request - invalid parameters".to_string(),
                    content: None,
                });
                responses.insert("500".to_string(), Response {
                    description: "Internal server error".to_string(),
                    content: None,
                });
                responses
            },
        }),
        put: None,
        delete: None,
    });

    paths.insert("/bridge/health".to_string(), PathItem {
        get: Some(Operation {
            tags: vec!["Health".to_string()],
            summary: "Get bridge system health status".to_string(),
            description: "Check the health and availability of all bridge integrations".to_string(),
            parameters: None,
            request_body: None,
            responses: {
                let mut responses = HashMap::new();
                responses.insert("200".to_string(), Response {
                    description: "Health status response".to_string(),
                    content: Some({
                        let mut content = HashMap::new();
                        content.insert("application/json".to_string(), MediaType {
                            schema: Schema {
                                schema_type: None,
                                format: None,
                                properties: None,
                                items: None,
                                reference: Some("#/components/schemas/HealthResponse".to_string()),
                                required: None,
                                example: None,
                            },
                        });
                        content
                    }),
                });
                responses
            },
        }),
        post: None,
        put: None,
        delete: None,
    });

    OpenApiSpec {
        openapi: "3.0.3".to_string(),
        info: ApiInfo {
            title: "Bridge Aggregator API".to_string(),
            description: "Cross-chain bridge aggregation service providing quotes and execution across multiple bridge providers".to_string(),
            version: "1.0.0".to_string(),
            contact: Contact {
                name: "Bridge Team".to_string(),
                email: "bridges@bralaladex.com".to_string(),
                url: "https://bralaladex.com".to_string(),
            },
            license: License {
                name: "MIT".to_string(),
                url: "https://opensource.org/licenses/MIT".to_string(),
            },
        },
        servers: vec![
            Server {
                url: "http://localhost:3001".to_string(),
                description: "Development server".to_string(),
            },
            Server {
                url: "https://api.bralaladex.com".to_string(),
                description: "Production server".to_string(),
            },
        ],
        paths,
        components: Components { schemas },
    }
}

pub fn save_openapi_spec(spec: &OpenApiSpec, file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(spec)?;
    std::fs::write(file_path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_spec_generation() {
        let spec = generate_bridge_api_spec();
        assert_eq!(spec.openapi, "3.0.3");
        assert_eq!(spec.info.title, "Bridge Aggregator API");
        assert!(spec.paths.contains_key("/bridge/quote"));
        assert!(spec.paths.contains_key("/bridge/execute"));
        assert!(spec.paths.contains_key("/bridge/health"));
    }

    #[test]
    fn test_openapi_serialization() {
        let spec = generate_bridge_api_spec();
        let json = serde_json::to_string(&spec).unwrap();
        assert!(json.contains("Bridge Aggregator API"));
        assert!(json.contains("/bridge/quote"));
    }
}
