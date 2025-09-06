use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{broadcast, RwLock, mpsc};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::str::FromStr;

use bralaladex_backend::trade_streaming::{
    TradeEventStreamer, TradeEvent, TradeExecutionEvent, RoutingDecisionEvent,
    SlippageUpdateEvent, FailedTransactionEvent, TradeEventMessage, TradeStreamingConfig,
};
use bralaladex_backend::aggregator::QuoteParams;

#[tokio::test]
async fn test_trade_event_streamer_creation() {
    let config = TradeStreamingConfig::default();
    let streamer = TradeEventStreamer::new(config).await.unwrap();
    
    assert_eq!(streamer.get_active_subscriptions().await, 0);
    assert!(streamer.is_healthy().await);
}

#[tokio::test]
async fn test_live_trade_execution_notifications() {
    let config = TradeStreamingConfig::default();
    let streamer = TradeEventStreamer::new(config).await.unwrap();
    
    let user_id = Uuid::new_v4();
    let trade_id = Uuid::new_v4();
    
    // Subscribe to trade events
    let mut receiver = streamer.subscribe_to_trade_events(user_id).await.unwrap();
    
    // Simulate trade execution
    let execution_event = TradeExecutionEvent {
        trade_id,
        user_id,
        token_in: "ETH".to_string(),
        token_out: "USDC".to_string(),
        amount_in: Decimal::from(1000000000000000000u64), // 1 ETH in wei
        amount_out: Decimal::from(3400000000u64), // 3400 USDC in smallest unit
        dex_name: "Uniswap V3".to_string(),
        transaction_hash: "0x123...".to_string(),
        gas_used: 150000,
        gas_price: Decimal::from(20000000000u64), // 20 gwei
        execution_time_ms: 2500,
        status: "confirmed".to_string(),
        timestamp: Utc::now(),
    };
    
    // Emit trade execution event
    streamer.emit_trade_execution(execution_event.clone()).await.unwrap();
    
    // Verify event received
    let received_message = tokio::time::timeout(
        tokio::time::Duration::from_secs(1),
        receiver.recv()
    ).await.unwrap().unwrap();
    
    match received_message {
        TradeEventMessage::TradeExecution(event) => {
            assert_eq!(event.trade_id, trade_id);
            assert_eq!(event.user_id, user_id);
            assert_eq!(event.token_in, "ETH");
            assert_eq!(event.token_out, "USDC");
            assert_eq!(event.dex_name, "Uniswap V3");
            assert_eq!(event.status, "confirmed");
        }
        _ => panic!("Expected TradeExecution event"),
    }
}

#[tokio::test]
async fn test_dex_routing_decisions_real_time() {
    let config = TradeStreamingConfig::default();
    let streamer = TradeEventStreamer::new(config).await.unwrap();
    
    let user_id = Uuid::new_v4();
    let quote_id = Uuid::new_v4();
    
    // Subscribe to routing events
    let mut receiver = streamer.subscribe_to_routing_events(user_id).await.unwrap();
    
    // Simulate routing decision
    let routing_event = RoutingDecisionEvent {
        quote_id,
        user_id,
        token_in: "ETH".to_string(),
        token_out: "USDC".to_string(),
        amount_in: Decimal::from(1000000000000000000u64), // 1 ETH
        selected_route: vec![
            ("Uniswap V3".to_string(), Decimal::from(60)), // 60%
            ("Curve".to_string(), Decimal::from(40)),       // 40%
        ],
        alternative_routes: vec![
            vec![("Balancer".to_string(), Decimal::from(100))],
            vec![("SushiSwap".to_string(), Decimal::from(100))],
        ],
        selection_reason: "Best price with acceptable slippage".to_string(),
        expected_output: Decimal::from(3405000000u64), // 3405 USDC
        estimated_gas: 180000,
        price_impact: Decimal::from_str("0.0025").unwrap(), // 0.25%
        timestamp: Utc::now(),
    };
    
    // Emit routing decision
    streamer.emit_routing_decision(routing_event.clone()).await.unwrap();
    
    // Verify event received
    let received_message = tokio::time::timeout(
        tokio::time::Duration::from_secs(1),
        receiver.recv()
    ).await.unwrap().unwrap();
    
    match received_message {
        TradeEventMessage::RoutingDecision(event) => {
            assert_eq!(event.quote_id, quote_id);
            assert_eq!(event.selected_route.len(), 2);
            assert_eq!(event.selected_route[0].0, "Uniswap V3");
            assert_eq!(event.selected_route[1].0, "Curve");
            assert!(event.price_impact < Decimal::from_str("0.01").unwrap()); // < 1%
        }
        _ => panic!("Expected RoutingDecision event"),
    }
}

#[tokio::test]
async fn test_slippage_and_price_impact_updates() {
    let config = TradeStreamingConfig::default();
    let streamer = TradeEventStreamer::new(config).await.unwrap();
    
    let user_id = Uuid::new_v4();
    let trade_id = Uuid::new_v4();
    
    // Subscribe to slippage events
    let mut receiver = streamer.subscribe_to_slippage_events(user_id).await.unwrap();
    
    // Simulate slippage update during trade
    let slippage_event = SlippageUpdateEvent {
        trade_id,
        user_id,
        token_pair: ("ETH".to_string(), "USDC".to_string()),
        expected_price: Decimal::from_str("3400.0").unwrap(),
        actual_price: Decimal::from_str("3395.5").unwrap(),
        slippage_percentage: Decimal::from_str("0.132").unwrap(), // 0.132%
        price_impact: Decimal::from_str("0.089").unwrap(), // 0.089%
        liquidity_depth: Decimal::from(15000000), // $15M
        market_conditions: "normal".to_string(),
        dex_name: "Uniswap V3".to_string(),
        timestamp: Utc::now(),
    };
    
    // Emit slippage update
    streamer.emit_slippage_update(slippage_event.clone()).await.unwrap();
    
    // Verify event received
    let received_message = tokio::time::timeout(
        tokio::time::Duration::from_secs(1),
        receiver.recv()
    ).await.unwrap().unwrap();
    
    match received_message {
        TradeEventMessage::SlippageUpdate(event) => {
            assert_eq!(event.trade_id, trade_id);
            assert_eq!(event.token_pair.0, "ETH");
            assert_eq!(event.token_pair.1, "USDC");
            assert!(event.slippage_percentage < Decimal::from_str("0.5").unwrap()); // < 0.5%
            assert!(event.price_impact < Decimal::from_str("0.1").unwrap()); // < 0.1%
        }
        _ => panic!("Expected SlippageUpdate event"),
    }
}

#[tokio::test]
async fn test_failed_transaction_alerts() {
    let config = TradeStreamingConfig::default();
    let streamer = TradeEventStreamer::new(config).await.unwrap();
    
    let user_id = Uuid::new_v4();
    let trade_id = Uuid::new_v4();
    
    // Subscribe to failure events
    let mut receiver = streamer.subscribe_to_failure_events(user_id).await.unwrap();
    
    // Simulate failed transaction
    let failure_event = FailedTransactionEvent {
        trade_id,
        user_id,
        transaction_hash: Some("0xfailed123...".to_string()),
        failure_reason: "Insufficient gas".to_string(),
        error_code: "GAS_LIMIT_EXCEEDED".to_string(),
        gas_used: 149999, // Just under limit
        gas_limit: 150000,
        gas_price: Decimal::from(25000000000u64), // 25 gwei
        token_in: "ETH".to_string(),
        token_out: "USDC".to_string(),
        amount_in: Decimal::from(1000000000000000000u64),
        dex_name: "Uniswap V3".to_string(),
        retry_possible: true,
        suggested_gas_limit: Some(200000),
        timestamp: Utc::now(),
    };
    
    // Emit failure event
    streamer.emit_transaction_failure(failure_event.clone()).await.unwrap();
    
    // Verify event received
    let received_message = tokio::time::timeout(
        tokio::time::Duration::from_secs(1),
        receiver.recv()
    ).await.unwrap().unwrap();
    
    match received_message {
        TradeEventMessage::TransactionFailure(event) => {
            assert_eq!(event.trade_id, trade_id);
            assert_eq!(event.error_code, "GAS_LIMIT_EXCEEDED");
            assert_eq!(event.failure_reason, "Insufficient gas");
            assert!(event.retry_possible);
            assert_eq!(event.suggested_gas_limit, Some(200000));
        }
        _ => panic!("Expected TransactionFailure event"),
    }
}

#[tokio::test]
async fn test_multiple_user_subscriptions() {
    let config = TradeStreamingConfig::default();
    let streamer = TradeEventStreamer::new(config).await.unwrap();
    
    let user1 = Uuid::new_v4();
    let user2 = Uuid::new_v4();
    let trade_id = Uuid::new_v4();
    
    // Both users subscribe
    let mut receiver1 = streamer.subscribe_to_trade_events(user1).await.unwrap();
    let mut receiver2 = streamer.subscribe_to_trade_events(user2).await.unwrap();
    
    assert_eq!(streamer.get_active_subscriptions().await, 2);
    
    // Emit event for user1 only
    let execution_event = TradeExecutionEvent {
        trade_id,
        user_id: user1,
        token_in: "ETH".to_string(),
        token_out: "USDC".to_string(),
        amount_in: Decimal::from(1000000000000000000u64),
        amount_out: Decimal::from(3400000000u64),
        dex_name: "Uniswap V3".to_string(),
        transaction_hash: "0x123...".to_string(),
        gas_used: 150000,
        gas_price: Decimal::from(20000000000u64),
        execution_time_ms: 2500,
        status: "confirmed".to_string(),
        timestamp: Utc::now(),
    };
    
    streamer.emit_trade_execution(execution_event).await.unwrap();
    
    // Only user1 should receive the event
    let received1 = tokio::time::timeout(
        tokio::time::Duration::from_millis(100),
        receiver1.recv()
    ).await.unwrap().unwrap();
    
    assert!(matches!(received1, TradeEventMessage::TradeExecution(_)));
    
    // User2 should not receive anything
    let received2 = tokio::time::timeout(
        tokio::time::Duration::from_millis(100),
        receiver2.recv()
    ).await;
    
    assert!(received2.is_err()); // Timeout - no message received
}

#[tokio::test]
async fn test_trade_streaming_performance() {
    let config = TradeStreamingConfig {
        max_subscribers: 1000,
        event_buffer_size: 10000,
        cleanup_interval_secs: 30,
    };
    let streamer = TradeEventStreamer::new(config).await.unwrap();
    
    let user_id = Uuid::new_v4();
    let mut receiver = streamer.subscribe_to_trade_events(user_id).await.unwrap();
    
    // Emit 100 events rapidly
    let start_time = std::time::Instant::now();
    
    for i in 0..100 {
        let execution_event = TradeExecutionEvent {
            trade_id: Uuid::new_v4(),
            user_id,
            token_in: "ETH".to_string(),
            token_out: "USDC".to_string(),
            amount_in: Decimal::from(1000000000000000000u64),
            amount_out: Decimal::from(3400000000u64 + i), // Vary output
            dex_name: "Uniswap V3".to_string(),
            transaction_hash: format!("0x{:064x}", i),
            gas_used: 150000,
            gas_price: Decimal::from(20000000000u64),
            execution_time_ms: 2500,
            status: "confirmed".to_string(),
            timestamp: Utc::now(),
        };
        
        streamer.emit_trade_execution(execution_event).await.unwrap();
    }
    
    let emit_duration = start_time.elapsed();
    
    // Receive all events
    let mut received_count = 0;
    let receive_start = std::time::Instant::now();
    
    while received_count < 100 {
        let _message = tokio::time::timeout(
            tokio::time::Duration::from_secs(1),
            receiver.recv()
        ).await.unwrap().unwrap();
        received_count += 1;
    }
    
    let receive_duration = receive_start.elapsed();
    
    // Performance assertions
    assert!(emit_duration.as_millis() < 1000, "Emitting 100 events should take < 1s");
    assert!(receive_duration.as_millis() < 1000, "Receiving 100 events should take < 1s");
    assert_eq!(received_count, 100);
}

#[tokio::test]
async fn test_trade_streaming_integration_with_aggregator() {
    let config = TradeStreamingConfig::default();
    let streamer = TradeEventStreamer::new(config).await.unwrap();
    
    let user_id = Uuid::new_v4();
    let mut receiver = streamer.subscribe_to_all_events(user_id).await.unwrap();
    
    // Simulate complete trade flow
    let quote_params = QuoteParams {
        token_in: "ETH".to_string(),
        token_out: "USDC".to_string(),
        amount_in: "1000000000000000000".to_string(), // 1 ETH
        user_address: format!("{}", user_id),
        slippage_tolerance: Some("0.5".to_string()),
    };
    
    // 1. Routing decision
    let routing_event = RoutingDecisionEvent {
        quote_id: Uuid::new_v4(),
        user_id,
        token_in: quote_params.token_in.clone(),
        token_out: quote_params.token_out.clone(),
        amount_in: Decimal::from_str(&quote_params.amount_in).unwrap(),
        selected_route: vec![("Uniswap V3".to_string(), Decimal::from(100))],
        alternative_routes: vec![],
        selection_reason: "Best available rate".to_string(),
        expected_output: Decimal::from(3400000000u64),
        estimated_gas: 150000,
        price_impact: Decimal::from_str("0.0025").unwrap(),
        timestamp: Utc::now(),
    };
    
    streamer.emit_routing_decision(routing_event).await.unwrap();
    
    // 2. Slippage update during execution
    let slippage_event = SlippageUpdateEvent {
        trade_id: Uuid::new_v4(),
        user_id,
        token_pair: (quote_params.token_in.clone(), quote_params.token_out.clone()),
        expected_price: Decimal::from_str("3400.0").unwrap(),
        actual_price: Decimal::from_str("3398.5").unwrap(),
        slippage_percentage: Decimal::from_str("0.044").unwrap(),
        price_impact: Decimal::from_str("0.025").unwrap(),
        liquidity_depth: Decimal::from(15000000),
        market_conditions: "normal".to_string(),
        dex_name: "Uniswap V3".to_string(),
        timestamp: Utc::now(),
    };
    
    streamer.emit_slippage_update(slippage_event).await.unwrap();
    
    // 3. Successful execution
    let execution_event = TradeExecutionEvent {
        trade_id: Uuid::new_v4(),
        user_id,
        token_in: quote_params.token_in.clone(),
        token_out: quote_params.token_out.clone(),
        amount_in: Decimal::from_str(&quote_params.amount_in).unwrap(),
        amount_out: Decimal::from(3398500000u64), // Actual output with slippage
        dex_name: "Uniswap V3".to_string(),
        transaction_hash: "0xsuccess123...".to_string(),
        gas_used: 148500,
        gas_price: Decimal::from(22000000000u64),
        execution_time_ms: 3200,
        status: "confirmed".to_string(),
        timestamp: Utc::now(),
    };
    
    streamer.emit_trade_execution(execution_event).await.unwrap();
    
    // Verify all events received in order
    let mut event_types = Vec::new();
    
    for _ in 0..3 {
        let message = tokio::time::timeout(
            tokio::time::Duration::from_secs(1),
            receiver.recv()
        ).await.unwrap().unwrap();
        
        match message {
            TradeEventMessage::RoutingDecision(_) => event_types.push("routing"),
            TradeEventMessage::SlippageUpdate(_) => event_types.push("slippage"),
            TradeEventMessage::TradeExecution(_) => event_types.push("execution"),
            _ => {}
        }
    }
    
    assert_eq!(event_types, vec!["routing", "slippage", "execution"]);
}
