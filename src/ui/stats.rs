use crate::app::{App, HeatmapView, StatsState};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{BarChart, Block, Borders, Paragraph},
};
use std::collections::HashMap;

use super::theme;

pub fn draw(f: &mut Frame, app: &App) {
    let stats = match &app.stats_state {
        Some(s) => s,
        None => return,
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .margin(1)
        .split(f.area());

    draw_retention(f, chunks[0], stats);
    draw_heatmap(f, chunks[1], stats);
    draw_forecast(f, chunks[2], stats);

    let hint = Paragraph::new("v toggle view  Esc back  q quit").style(theme::hint());
    f.render_widget(hint, chunks[3]);
}

fn draw_retention(f: &mut Frame, area: Rect, stats: &StatsState) {
    let text = match stats.retention {
        Some(rate) => format!("retention (30d): {:.0}%", rate),
        None => "retention (30d): no reviews yet".to_string(),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(" Stats ")
        .title_style(theme::title());
    let para = Paragraph::new(text).block(block);
    f.render_widget(para, area);
}

fn draw_heatmap(f: &mut Frame, area: Rect, stats: &StatsState) {
    let title = match stats.view {
        HeatmapView::Daily => " Review Heatmap (90d) — daily ",
        HeatmapView::Weekly => " Review Heatmap (90d) — weekly ",
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(title)
        .title_style(theme::title());

    let counts = build_heatmap_counts(&stats.heatmap);
    let today = Utc::now().date_naive();
    let grid_text = match stats.view {
        HeatmapView::Daily => build_heatmap_grid_daily(&counts, today),
        HeatmapView::Weekly => build_heatmap_grid_weekly(&counts, today),
    };
    let legend = "less  ·  ▪  ▮  more";
    let content = format!("{}\n{}", grid_text, legend);

    let para = Paragraph::new(content).block(block).style(theme::dim());
    f.render_widget(para, area);
}

fn build_heatmap_counts(heatmap: &[(NaiveDate, i64)]) -> HashMap<NaiveDate, i64> {
    heatmap.iter().map(|(d, c)| (*d, *c)).collect()
}

fn build_heatmap_grid_daily(counts: &HashMap<NaiveDate, i64>, today: NaiveDate) -> String {
    // Build a 7 x 13 grid (91 days). The rightmost column is the current week.
    let total_cols = 13;
    let total_days = total_cols * 7;
    let start = today - Duration::days(total_days as i64 - 1);

    let mut lines = Vec::with_capacity(7);
    for weekday in 0..7 {
        let mut row = String::with_capacity(total_cols);
        for col in 0..total_cols {
            let offset = col * 7 + weekday;
            let day = start + Duration::days(offset as i64);
            let count = counts.get(&day).copied().unwrap_or(0);
            let ch = density_glyph_daily(count);
            row.push(ch);
        }
        lines.push(row);
    }

    lines.join("\n")
}

fn build_heatmap_grid_weekly(counts: &HashMap<NaiveDate, i64>, today: NaiveDate) -> String {
    // Build a 1 x 13 grid of weeks. The rightmost cell is the current week.
    let total_cols = 13;
    let total_days = total_cols * 7;
    let start = today - Duration::days(total_days as i64 - 1);

    let mut row = String::with_capacity(total_cols);
    for col in 0..total_cols {
        let week_start = start + Duration::days(col as i64 * 7);
        let mut sum = 0;
        for offset in 0..7 {
            let day = week_start + Duration::days(offset);
            sum += counts.get(&day).copied().unwrap_or(0);
        }
        row.push(density_glyph_weekly(sum));
    }

    row
}

fn density_glyph_daily(count: i64) -> char {
    match count {
        0 => ' ',
        1..=2 => '·',
        3..=5 => '▪',
        _ => '▮',
    }
}

fn density_glyph_weekly(count: i64) -> char {
    match count {
        0 => ' ',
        1..=14 => '·',
        15..=35 => '▪',
        _ => '▮',
    }
}

fn draw_forecast(f: &mut Frame, area: Rect, stats: &StatsState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(" Forecast (14d) ")
        .title_style(theme::title());

    let counts = build_forecast_counts(&stats.forecast);
    let labels: Vec<String> = counts
        .iter()
        .map(|(date, _)| format!("{}", date.day()))
        .collect();
    let data: Vec<(&str, u64)> = labels
        .iter()
        .zip(counts.iter())
        .map(|(label, (_, count))| (label.as_str(), *count as u64))
        .collect();
    let max = counts.iter().map(|(_, c)| *c).max().unwrap_or(0) as u64;

    let barchart = BarChart::default()
        .block(block)
        .data(&data[..])
        .bar_width(2)
        .bar_gap(1)
        .max(max.max(1));

    f.render_widget(barchart, area);
}

fn build_forecast_counts(forecast: &[(NaiveDate, i64)]) -> Vec<(NaiveDate, i64)> {
    let today = Utc::now().date_naive();
    let mut map: HashMap<NaiveDate, i64> = forecast.iter().map(|(d, c)| (*d, *c)).collect();
    let mut result = Vec::with_capacity(14);
    for offset in 0..14 {
        let day = today + Duration::days(offset);
        result.push((day, map.remove(&day).unwrap_or(0)));
    }
    result
}
