use crate::models::DashboardStats;
use crate::services::state::AppState;
use chrono::{Datelike, Local, NaiveDate};
use tauri::State;

#[tauri::command]
pub async fn get_dashboard_stats(
    year_month: Option<String>,
    state: State<'_, AppState>,
) -> Result<DashboardStats, String> {
    let now = Local::now();
    let current_year_month = year_month.unwrap_or_else(|| format!("{}-{:02}", now.year(), now.month()));
    let current_year = &current_year_month[0..4];
    let today = now.date_naive().format("%Y-%m-%d").to_string();

    let db = state.db.lock().map_err(|_| "DB lock".to_string())?;

    let revenue_month = db
        .get_monthly_sum("revenue", &current_year_month)
        .map_err(|e| e.to_string())?;
    let payable_month = db
        .get_monthly_sum("payable", &current_year_month)
        .map_err(|e| e.to_string())?;
    let revenue_year = db
        .get_yearly_sum("revenue", current_year)
        .map_err(|e| e.to_string())?;
    let payable_year = db
        .get_yearly_sum("payable", current_year)
        .map_err(|e| e.to_string())?;
    let overdue_revenue = db
        .get_overdue_total("revenue", &today)
        .map_err(|e| e.to_string())?;
    let overdue_payables = db
        .get_overdue_total("payable", &today)
        .map_err(|e| e.to_string())?;
    let open_revenue_items = db
        .get_invoice_summaries_by_status("revenue", "open")
        .map_err(|e| e.to_string())?;
    let open_payables_items = db
        .get_invoice_summaries_by_status("payable", "open")
        .map_err(|e| e.to_string())?;
    let best_customers = db
        .get_top_counterparties_by_year("revenue", current_year, 5)
        .map_err(|e| e.to_string())?;
    let highest_costs = db
        .get_top_counterparties_by_year("payable", current_year, 5)
        .map_err(|e| e.to_string())?;

    let (chart_months, chart_revenue, chart_payables, chart_profit) = build_chart_series(&*db, &current_year_month)?;

    Ok(DashboardStats {
        revenue_month,
        revenue_year,
        payable_month,
        payable_year,
        profit_month: revenue_month - payable_month,
        profit_year: revenue_year - payable_year,
        overdue_revenue,
        overdue_payables,
        open_revenue_items,
        open_payables_items,
        best_customers,
        highest_costs,
        chart_months,
        chart_revenue,
        chart_payables,
        chart_profit,
    })
}

fn build_chart_series(
    db: &crate::db::Database,
    current_year_month: &str,
) -> Result<(Vec<String>, Vec<f64>, Vec<f64>, Vec<f64>), String> {
    let base_date = NaiveDate::parse_from_str(&format!("{}-01", current_year_month), "%Y-%m-%d")
        .map_err(|e| e.to_string())?;

    let mut months = Vec::new();
    let mut revenue = Vec::new();
    let mut payables = Vec::new();
    let mut profits = Vec::new();

    for offset in (0..12).rev() {
        let date = base_date
            .with_day(1)
            .and_then(|d| d.checked_sub_months(chrono::Months::new(offset as u32)))
            .ok_or_else(|| "Invalid date".to_string())?;
        let ym = format!("{}-{:02}", date.year(), date.month());
        let rev = db.get_monthly_sum("revenue", &ym).map_err(|e| e.to_string())?;
        let pay = db.get_monthly_sum("payable", &ym).map_err(|e| e.to_string())?;
        months.push(ym);
        revenue.push(rev);
        payables.push(pay);
        profits.push(rev - pay);
    }

    Ok((months, revenue, payables, profits))
}
