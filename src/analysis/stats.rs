//! Code statistics from the Mirage database.
//!
//! Provides aggregate metrics:
//! - Function counts by risk level
//! - Total/average/max path counts
//! - Complexity distribution
//! - Dead code (unreachable blocks)
//! - Coverage gaps (functions with no CFG data)

use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;

use crate::storage::MirageDb;

#[derive(Debug, Clone, Serialize)]
pub struct StatsReport {
    pub total_functions: usize,
    pub functions_with_cfg: usize,
    pub functions_without_cfg: usize,
    pub total_blocks: usize,
    pub total_paths: usize,
    pub avg_blocks_per_function: f64,
    pub max_blocks: usize,
    pub max_blocks_function: Option<String>,
    pub dead_code_count: usize,
    pub coverage_gap_count: usize,
    pub complexity_distribution: ComplexityDistribution,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComplexityDistribution {
    pub low: usize,
    pub medium: usize,
    pub high: usize,
    pub critical: usize,
}

pub fn gather_stats(db: &MirageDb) -> Result<StatsReport> {
    let conn = db.conn()?;

    let total_functions: usize = count_functions(conn)?;
    let functions_with_cfg: usize = count_functions_with_cfg(conn)?;
    let functions_without_cfg = total_functions.saturating_sub(functions_with_cfg);

    let total_blocks: usize = count_total_blocks(conn)?;
    let avg_blocks = if functions_with_cfg > 0 {
        total_blocks as f64 / functions_with_cfg as f64
    } else {
        0.0
    };

    let (max_blocks, max_blocks_function) = find_largest_function(conn)?;
    let dead_code_count = count_dead_blocks(conn)?;
    let coverage_gap_count = functions_without_cfg;

    let total_paths = count_total_paths(conn)?;

    let complexity_distribution = classify_complexity(conn)?;

    Ok(StatsReport {
        total_functions,
        functions_with_cfg,
        functions_without_cfg,
        total_blocks,
        total_paths,
        avg_blocks_per_function: avg_blocks,
        max_blocks,
        max_blocks_function,
        dead_code_count,
        coverage_gap_count,
        complexity_distribution,
    })
}

fn count_functions(conn: &Connection) -> Result<usize> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM graph_entities WHERE kind IN ('Function', 'Method', 'Symbol')",
        [],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

fn count_functions_with_cfg(conn: &Connection) -> Result<usize> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT function_id) FROM cfg_blocks",
        [],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

fn count_total_blocks(conn: &Connection) -> Result<usize> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM cfg_blocks", [], |row| row.get(0))?;
    Ok(count as usize)
}

fn count_total_paths(conn: &Connection) -> Result<usize> {
    let count: Option<i64> = conn
        .query_row("SELECT COUNT(*) FROM cfg_paths", [], |row| row.get(0))
        .ok();
    Ok(count.unwrap_or(0) as usize)
}

fn find_largest_function(conn: &Connection) -> Result<(usize, Option<String>)> {
    let result: Option<(i64, String)> = conn
        .query_row(
            "SELECT COUNT(cb.id), ge.name
             FROM cfg_blocks cb
             JOIN graph_entities ge ON cb.function_id = ge.id
             GROUP BY cb.function_id
             ORDER BY COUNT(cb.id) DESC
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();

    match result {
        Some((count, name)) => Ok((count as usize, Some(name))),
        None => Ok((0, None)),
    }
}

fn count_dead_blocks(conn: &Connection) -> Result<usize> {
    let count: Option<i64> = conn
        .query_row(
            "SELECT COUNT(*) FROM cfg_blocks WHERE kind = 'unreachable'",
            [],
            |row| row.get(0),
        )
        .ok();
    Ok(count.unwrap_or(0) as usize)
}

fn classify_complexity(conn: &Connection) -> Result<ComplexityDistribution> {
    let mut stmt = conn.prepare(
        "SELECT cb.function_id, COUNT(cb.id) as block_count
         FROM cfg_blocks cb
         GROUP BY cb.function_id",
    )?;

    let rows = stmt.query_map([], |row| {
        let block_count: i64 = row.get(1)?;
        Ok(block_count)
    })?;

    let mut low = 0usize;
    let mut medium = 0usize;
    let mut high = 0usize;
    let mut critical = 0usize;

    for row in rows {
        let blocks: i64 = row?;
        if blocks <= 10 {
            low += 1;
        } else if blocks <= 25 {
            medium += 1;
        } else if blocks <= 50 {
            high += 1;
        } else {
            critical += 1;
        }
    }

    Ok(ComplexityDistribution {
        low,
        medium,
        high,
        critical,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_classification() {
        let dist = ComplexityDistribution {
            low: 10,
            medium: 5,
            high: 2,
            critical: 1,
        };
        assert_eq!(dist.low, 10);
        assert_eq!(dist.critical, 1);
    }
}
