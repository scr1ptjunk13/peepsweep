use crate::analytics::gas_usage_tracker::{GasUsageTracker, GasUsageRecord, GasEfficiencyMetrics};
use crate::analytics::gas_optimization_analyzer::{GasOptimizationAnalyzer, GasOptimizationInsights};
use crate::risk_management::types::{UserId, RiskError};
use chrono::{DateTime, Utc, Duration, Datelike};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Comprehensive gas usage report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasUsageReport {
    pub report_id: String,
    pub user_id: UserId,
    pub report_type: ReportType,
    pub period: ReportPeriod,
    pub generated_at: DateTime<Utc>,
    pub summary: GasUsageSummary,
    pub detailed_metrics: GasEfficiencyMetrics,
    pub optimization_insights: Option<GasOptimizationInsights>,
    pub charts_data: Vec<ChartData>,
    pub recommendations: Vec<String>,
    pub export_formats: Vec<ExportFormat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReportType {
    Daily,
    Weekly,
    Monthly,
    Custom,
    Comparative,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportPeriod {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasUsageSummary {
    pub total_transactions: u64,
    pub total_gas_spent_usd: Decimal,
    pub average_gas_per_transaction: Decimal,
    pub most_expensive_transaction: Option<TransactionSummary>,
    pub most_efficient_dex: Option<String>,
    pub efficiency_trend: TrendDirection,
    pub savings_opportunities: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSummary {
    pub transaction_hash: String,
    pub gas_cost_usd: Decimal,
    pub dex_name: String,
    pub token_pair: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    Improving,
    Declining,
    Stable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartData {
    pub chart_type: ChartType,
    pub title: String,
    pub data_points: Vec<DataPoint>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChartType {
    LineChart,
    BarChart,
    PieChart,
    ScatterPlot,
    Heatmap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub x: String, // Could be timestamp, category, etc.
    pub y: Decimal,
    pub label: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    Json,
    Csv,
    Pdf,
    Excel,
}

/// Report generation interface
#[async_trait::async_trait]
pub trait ReportGenerator: Send + Sync {
    async fn generate_daily_report(&self, user_id: UserId, date: DateTime<Utc>) -> Result<GasUsageReport, RiskError>;
    async fn generate_weekly_report(&self, user_id: UserId, week_start: DateTime<Utc>) -> Result<GasUsageReport, RiskError>;
    async fn generate_monthly_report(&self, user_id: UserId, month: DateTime<Utc>) -> Result<GasUsageReport, RiskError>;
    async fn generate_custom_report(&self, user_id: UserId, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<GasUsageReport, RiskError>;
    async fn generate_comparative_report(&self, user_ids: Vec<UserId>, period: ReportPeriod) -> Result<ComparativeGasReport, RiskError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparativeGasReport {
    pub report_id: String,
    pub user_comparisons: Vec<UserGasComparison>,
    pub period: ReportPeriod,
    pub generated_at: DateTime<Utc>,
    pub aggregate_insights: AggregateInsights,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserGasComparison {
    pub user_id: UserId,
    pub total_gas_spent: Decimal,
    pub efficiency_score: Decimal,
    pub rank: u32,
    pub percentile: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateInsights {
    pub total_users: u32,
    pub average_gas_spent: Decimal,
    pub median_gas_spent: Decimal,
    pub top_performing_strategies: Vec<String>,
    pub common_inefficiencies: Vec<String>,
}

/// Main gas reports generator
pub struct GasReportsGenerator {
    usage_tracker: Arc<GasUsageTracker>,
    optimization_analyzer: Arc<GasOptimizationAnalyzer>,
    exporter: Arc<DefaultReportExporter>,
    report_cache: Arc<RwLock<HashMap<String, GasUsageReport>>>,
}

impl GasReportsGenerator {
    pub fn new(
        usage_tracker: Arc<GasUsageTracker>,
        analyzer: Arc<GasOptimizationAnalyzer>,
        exporter: Arc<DefaultReportExporter>,
    ) -> Self {
        Self {
            usage_tracker,
            optimization_analyzer: analyzer,
            exporter,
            report_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate daily report (direct method for API compatibility)
    pub async fn generate_daily_report(&self, user_id: UserId, date: DateTime<Utc>) -> Result<GasUsageReport, RiskError> {
        let start_date = date.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let end_date = start_date + Duration::days(1);
        self.generate_custom_report_impl(user_id, start_date, end_date).await
    }

    /// Generate weekly report (direct method for API compatibility)
    pub async fn generate_weekly_report(&self, user_id: UserId, week_start: DateTime<Utc>) -> Result<GasUsageReport, RiskError> {
        let end_date = week_start + Duration::days(7);
        self.generate_custom_report_impl(user_id, week_start, end_date).await
    }

    /// Generate monthly report (direct method for API compatibility)
    pub async fn generate_monthly_report(&self, user_id: UserId, month: DateTime<Utc>) -> Result<GasUsageReport, RiskError> {
        let start_date = month.date_naive().with_day(1).unwrap().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let end_date = if month.month() == 12 {
            start_date.with_year(start_date.year() + 1).unwrap().with_month(1).unwrap()
        } else {
            start_date.with_month(start_date.month() + 1).unwrap()
        };
        self.generate_custom_report_impl(user_id, start_date, end_date).await
    }

    /// Generate custom report (direct method for API compatibility)
    pub async fn generate_custom_report(&self, user_id: UserId, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<GasUsageReport, RiskError> {
        self.generate_custom_report_impl(user_id, start, end).await
    }

    /// Internal implementation for custom report generation
    async fn generate_custom_report_impl(&self, user_id: UserId, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<GasUsageReport, RiskError> {
        // Get gas usage data for the period
        let gas_records = self.usage_tracker.get_user_gas_usage(user_id, start, end).await?;
        
        if gas_records.is_empty() {
            return Err(RiskError::InsufficientData("No gas usage data found for the specified period".to_string()));
        }

        // Calculate efficiency metrics
        let efficiency_metrics = self.usage_tracker.calculate_gas_efficiency_metrics(user_id, start, end).await?;
        
        // Generate optimization insights
        let insights = self.optimization_analyzer.generate_optimization_insights(user_id, 30).await?;
        
        // Generate chart data
        let chart_data = self.generate_gas_trend_chart(&gas_records).await;
        
        Ok(GasUsageReport {
            report_id: uuid::Uuid::new_v4().to_string(),
            user_id,
            report_type: ReportType::Custom,
            period: ReportPeriod {
                start_date: start,
                end_date: end,
                description: format!("Custom period from {} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
            },
            generated_at: chrono::Utc::now(),
            summary: GasUsageSummary {
                total_transactions: gas_records.len() as u64,
                total_gas_spent_usd: gas_records.iter().map(|r| r.gas_cost_usd).sum(),
                average_gas_per_transaction: if gas_records.is_empty() { Decimal::ZERO } else {
                    gas_records.iter().map(|r| r.gas_cost_usd).sum::<Decimal>() / Decimal::from(gas_records.len())
                },
                most_expensive_transaction: gas_records.iter().max_by_key(|r| r.gas_cost_usd).map(|r| TransactionSummary {
                    transaction_hash: r.transaction_hash.clone(),
                    gas_cost_usd: r.gas_cost_usd,
                    dex_name: r.dex_name.clone(),
                    token_pair: r.token_pair.clone(),
                    timestamp: r.timestamp,
                }),
                most_efficient_dex: efficiency_metrics.most_efficient_dex.clone(),
                efficiency_trend: TrendDirection::Stable,
                savings_opportunities: insights.potential_savings_usd,
            },
            detailed_metrics: efficiency_metrics,
            optimization_insights: Some(insights),
            charts_data: vec![chart_data],
            recommendations: vec![], // Populated from insights
            export_formats: vec![ExportFormat::Json, ExportFormat::Csv],
        })
    }

    /// Generate chart data for gas usage trends
    async fn generate_gas_trend_chart(&self, records: &[GasUsageRecord]) -> ChartData {
        let mut daily_costs: HashMap<String, Decimal> = HashMap::new();
        
        for record in records {
            let date_key = record.timestamp.format("%Y-%m-%d").to_string();
            *daily_costs.entry(date_key).or_insert(Decimal::ZERO) += record.gas_cost_usd;
        }

        let mut data_points: Vec<DataPoint> = daily_costs
            .into_iter()
            .map(|(date, cost)| DataPoint {
                x: date,
                y: cost,
                label: Some(format!("${}", cost)),
                metadata: None,
            })
            .collect();

        data_points.sort_by(|a, b| a.x.cmp(&b.x));

        ChartData {
            chart_type: ChartType::LineChart,
            title: "Daily Gas Costs".to_string(),
            data_points,
            metadata: HashMap::new(),
        }
    }

    /// Generate DEX efficiency comparison chart
    async fn generate_dex_efficiency_chart(&self, records: &[GasUsageRecord]) -> ChartData {
        let mut dex_efficiency: HashMap<String, (Decimal, u64)> = HashMap::new();
        
        for record in records {
            let entry = dex_efficiency.entry(record.dex_name.clone()).or_insert((Decimal::ZERO, 0));
            entry.0 += record.gas_efficiency;
            entry.1 += 1;
        }

        let data_points: Vec<DataPoint> = dex_efficiency
            .into_iter()
            .map(|(dex, (total_eff, count))| {
                let avg_efficiency = total_eff / Decimal::from(count);
                DataPoint {
                    x: dex,
                    y: avg_efficiency,
                    label: Some(format!("{:.4}", avg_efficiency)),
                    metadata: Some([("transaction_count".to_string(), count.to_string())].into()),
                }
            })
            .collect();

        ChartData {
            chart_type: ChartType::BarChart,
            title: "DEX Efficiency Comparison".to_string(),
            data_points,
            metadata: HashMap::new(),
        }
    }

    /// Calculate efficiency trend
    fn calculate_efficiency_trend(&self, records: &[GasUsageRecord]) -> TrendDirection {
        if records.len() < 2 {
            return TrendDirection::Stable;
        }

        let mid_point = records.len() / 2;
        let first_half: Decimal = records[..mid_point].iter().map(|r| r.gas_efficiency).sum::<Decimal>() / Decimal::from(mid_point);
        let second_half: Decimal = records[mid_point..].iter().map(|r| r.gas_efficiency).sum::<Decimal>() / Decimal::from(records.len() - mid_point);

        let change_threshold = Decimal::try_from(0.05f64).unwrap(); // 5% change threshold
        
        if second_half < first_half - change_threshold {
            TrendDirection::Improving // Lower efficiency ratio is better
        } else if second_half > first_half + change_threshold {
            TrendDirection::Declining
        } else {
            TrendDirection::Stable
        }
    }
}

#[async_trait::async_trait]
impl ReportGenerator for GasReportsGenerator {
    async fn generate_daily_report(&self, user_id: UserId, date: DateTime<Utc>) -> Result<GasUsageReport, RiskError> {
        let start_date = date.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let end_date = start_date + Duration::days(1);
        
        self.generate_custom_report(user_id, start_date, end_date).await
    }

    async fn generate_weekly_report(&self, user_id: UserId, week_start: DateTime<Utc>) -> Result<GasUsageReport, RiskError> {
        let end_date = week_start + Duration::days(7);
        self.generate_custom_report(user_id, week_start, end_date).await
    }

    async fn generate_monthly_report(&self, user_id: UserId, month: DateTime<Utc>) -> Result<GasUsageReport, RiskError> {
        let start_date = month.date_naive().with_day(1).unwrap().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let end_date = if month.month() == 12 {
            start_date.with_year(start_date.year() + 1).unwrap().with_month(1).unwrap()
        } else {
            start_date.with_month(start_date.month() + 1).unwrap()
        };
        
        self.generate_custom_report(user_id, start_date, end_date).await
    }

    async fn generate_custom_report(&self, user_id: UserId, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<GasUsageReport, RiskError> {
        let records = self.usage_tracker.get_user_gas_usage(user_id, start, end).await?;
        
        if records.is_empty() {
            return Err(RiskError::InsufficientData("No gas usage data found for the specified period".to_string()));
        }

        let metrics = self.usage_tracker.calculate_gas_efficiency_metrics(user_id, start, end).await?;
        let optimization_insights = self.optimization_analyzer.generate_optimization_insights(user_id, (end - start).num_days() as u32).await.ok();

        // Find most expensive transaction
        let most_expensive = records.iter()
            .max_by_key(|r| r.gas_cost_usd.to_string())
            .map(|r| TransactionSummary {
                transaction_hash: r.transaction_hash.clone(),
                gas_cost_usd: r.gas_cost_usd,
                dex_name: r.dex_name.clone(),
                token_pair: r.token_pair.clone(),
                timestamp: r.timestamp,
            });

        let efficiency_trend = self.calculate_efficiency_trend(&records);
        let savings_opportunities = optimization_insights.as_ref()
            .map(|insights| insights.potential_savings_usd)
            .unwrap_or(Decimal::ZERO);

        let summary = GasUsageSummary {
            total_transactions: records.len() as u64,
            total_gas_spent_usd: metrics.total_gas_spent_usd,
            average_gas_per_transaction: metrics.average_gas_cost_usd,
            most_expensive_transaction: most_expensive,
            most_efficient_dex: metrics.most_efficient_dex.clone(),
            efficiency_trend,
            savings_opportunities,
        };

        // Generate charts
        let mut charts = Vec::new();
        charts.push(self.generate_gas_trend_chart(&records).await);
        charts.push(self.generate_dex_efficiency_chart(&records).await);

        // Generate recommendations
        let recommendations = if let Some(insights) = &optimization_insights {
            insights.recommendations.iter().take(3).map(|r| r.title.clone()).collect()
        } else {
            vec!["Insufficient data for recommendations".to_string()]
        };

        Ok(GasUsageReport {
            report_id: Uuid::new_v4().to_string(),
            user_id,
            report_type: ReportType::Custom,
            period: ReportPeriod {
                start_date: start,
                end_date: end,
                description: format!("Custom period: {} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
            },
            generated_at: Utc::now(),
            summary,
            detailed_metrics: metrics,
            optimization_insights,
            charts_data: charts,
            recommendations,
            export_formats: vec![ExportFormat::Json, ExportFormat::Csv],
        })
    }

    async fn generate_comparative_report(&self, user_ids: Vec<UserId>, period: ReportPeriod) -> Result<ComparativeGasReport, RiskError> {
        let mut user_comparisons = Vec::new();
        let mut all_gas_spent = Vec::new();

        for user_id in &user_ids {
            if let Ok(metrics) = self.usage_tracker.calculate_gas_efficiency_metrics(*user_id, period.start_date, period.end_date).await {
                let efficiency_score = Decimal::from(100) - (metrics.average_efficiency_ratio * Decimal::from(1000)).min(Decimal::from(100));
                
                user_comparisons.push(UserGasComparison {
                    user_id: *user_id,
                    total_gas_spent: metrics.total_gas_spent_usd,
                    efficiency_score,
                    rank: 0, // Will be calculated after sorting
                    percentile: Decimal::ZERO, // Will be calculated after sorting
                });
                
                all_gas_spent.push(metrics.total_gas_spent_usd);
            }
        }

        // Sort by efficiency score (descending)
        user_comparisons.sort_by(|a, b| b.efficiency_score.cmp(&a.efficiency_score));
        
        // Assign ranks and percentiles
        let total_count = user_comparisons.len();
        for (index, comparison) in user_comparisons.iter_mut().enumerate() {
            comparison.rank = (index + 1) as u32;
            comparison.percentile = Decimal::from(100 - index * 100 / total_count);
        }

        let total_users = user_comparisons.len() as u32;
        let average_gas_spent = if !all_gas_spent.is_empty() {
            all_gas_spent.iter().sum::<Decimal>() / Decimal::from(all_gas_spent.len())
        } else {
            Decimal::ZERO
        };

        all_gas_spent.sort();
        let median_gas_spent = if all_gas_spent.is_empty() {
            Decimal::ZERO
        } else if all_gas_spent.len() % 2 == 0 {
            let mid = all_gas_spent.len() / 2;
            (all_gas_spent[mid - 1] + all_gas_spent[mid]) / Decimal::from(2)
        } else {
            all_gas_spent[all_gas_spent.len() / 2]
        };

        let aggregate_insights = AggregateInsights {
            total_users,
            average_gas_spent,
            median_gas_spent,
            top_performing_strategies: vec![
                "Optimal timing strategy".to_string(),
                "Route optimization".to_string(),
                "Batch transactions".to_string(),
            ],
            common_inefficiencies: vec![
                "High gas price during peak hours".to_string(),
                "Inefficient DEX selection".to_string(),
                "Failed transactions".to_string(),
            ],
        };

        Ok(ComparativeGasReport {
            report_id: Uuid::new_v4().to_string(),
            user_comparisons,
            period,
            generated_at: Utc::now(),
            aggregate_insights,
        })
    }
}

/// Export functionality for reports
#[async_trait::async_trait]
pub trait ReportExporter: Send + Sync {
    async fn export_to_json(&self, report: &GasUsageReport) -> Result<String, RiskError>;
    async fn export_to_csv(&self, report: &GasUsageReport) -> Result<String, RiskError>;
    async fn export_to_pdf(&self, report: &GasUsageReport) -> Result<Vec<u8>, RiskError>;
}

pub struct DefaultReportExporter;

#[async_trait::async_trait]
impl ReportExporter for DefaultReportExporter {
    async fn export_to_json(&self, report: &GasUsageReport) -> Result<String, RiskError> {
        serde_json::to_string_pretty(report)
            .map_err(|e| RiskError::SerializationError(format!("JSON export failed: {}", e)))
    }

    async fn export_to_csv(&self, report: &GasUsageReport) -> Result<String, RiskError> {
        let mut csv_content = String::new();
        csv_content.push_str("Metric,Value\n");
        csv_content.push_str(&format!("Report ID,{}\n", report.report_id));
        csv_content.push_str(&format!("User ID,{}\n", report.user_id));
        csv_content.push_str(&format!("Total Transactions,{}\n", report.summary.total_transactions));
        csv_content.push_str(&format!("Total Gas Spent USD,{}\n", report.summary.total_gas_spent_usd));
        csv_content.push_str(&format!("Average Gas Per Transaction,{}\n", report.summary.average_gas_per_transaction));
        csv_content.push_str(&format!("Most Efficient DEX,{}\n", report.summary.most_efficient_dex.as_deref().unwrap_or("N/A")));
        csv_content.push_str(&format!("Savings Opportunities,{}\n", report.summary.savings_opportunities));
        
        Ok(csv_content)
    }

    async fn export_to_pdf(&self, _report: &GasUsageReport) -> Result<Vec<u8>, RiskError> {
        // Mock PDF generation - in real implementation, use a PDF library
        Ok(b"Mock PDF content".to_vec())
    }
}
