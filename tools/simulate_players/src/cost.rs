use std::sync::atomic::{AtomicU64, Ordering};

// ── AWS AppSync Pricing (us-east-1) ──────────────────────────────────
// https://aws.amazon.com/appsync/pricing/

/// $4.00 per million query/mutation operations
const APPSYNC_MUTATION_PRICE_PER_M: f64 = 4.00;

/// $2.00 per million real-time updates (outbound WS messages)
const APPSYNC_REALTIME_UPDATE_PRICE_PER_M: f64 = 2.00;

/// $0.08 per million connection-minutes
const APPSYNC_CONNECTION_MIN_PRICE_PER_M: f64 = 0.08;

// ── Amazon DynamoDB On-Demand Pricing (us-east-1) ────────────────────
// https://aws.amazon.com/dynamodb/pricing/on-demand/

/// $0.125 per million read request units (strongly consistent, up to 4 KB)
const DDB_READ_PRICE_PER_M: f64 = 0.1487;

/// $0.625 per million write request units (standard, up to 1 KB)
const DDB_WRITE_PRICE_PER_M: f64 = 0.7423;

/// $1.25 per million transactional write request units (2x standard)
const DDB_TX_WRITE_PRICE_PER_M: f64 = DDB_WRITE_PRICE_PER_M * 2.0;

// ── Per-Mutation DynamoDB Cost Profile ───────────────────────────────
// Derived from analyzing rust/lambdas/appsync-source/src/operations.rs

/// (standard_reads, standard_writes, transactional_reads, transactional_writes)
#[derive(Debug, Clone, Copy)]
struct DdbProfile {
    std_reads: u64,
    std_writes: u64,
    tx_writes: u64,
}

/// registerNewPlayer: PutItem(player) + UpdateItem(game_counts)
const REGISTER_NEW_PLAYER: DdbProfile = DdbProfile {
    std_reads: 0,
    std_writes: 2,
    tx_writes: 0,
};

/// buyBabyYak: 3 GetItems (player, game_status, game_counts)
///   + TransactWrite(Put yak, Update player) = 2 tx writes
///   + UpdateItem(game_counts) = 1 std write
const BUY_BABY_YAK: DdbProfile = DdbProfile {
    std_reads: 3,
    std_writes: 1,
    tx_writes: 2,
};

/// sellGrownYak: 3 GetItems (player, yak, game_counts)
///   + TransactWrite(Update player, Delete yak, Put yak) = 3 tx writes
///   + UpdateItem(game_counts) = 1 std write
const SELL_GROWN_YAK: DdbProfile = DdbProfile {
    std_reads: 2,
    std_writes: 1,
    tx_writes: 3,
};

/// buyGrownYak: 1 Query(warehouse) + 2 GetItems (player, game_counts)
///   + TransactWrite(Delete yak, Put yak, Update player) = 3 tx writes
///   + UpdateItem(game_counts) = 1 std write
const BUY_GROWN_YAK: DdbProfile = DdbProfile {
    std_reads: 4,
    std_writes: 1,
    tx_writes: 3,
};

/// sellUnshearedYak: same pattern as sellGrownYak
const SELL_UNSHEARED_YAK: DdbProfile = DdbProfile {
    std_reads: 2,
    std_writes: 1,
    tx_writes: 3,
};

/// buyUnshearedYak: same pattern as buyGrownYak (Query shearing_shed)
const BUY_UNSHEARED_YAK: DdbProfile = DdbProfile {
    std_reads: 4,
    std_writes: 1,
    tx_writes: 3,
};

/// sellShearedYak: 3 GetItems (player, yak, game_counts)
///   + TransactWrite(Update player, Delete yak) = 2 tx writes
///   + UpdateItem(game_counts) = 1 std write
const SELL_SHEARED_YAK: DdbProfile = DdbProfile {
    std_reads: 2,
    std_writes: 1,
    tx_writes: 2,
};

// ── Shared Counters ──────────────────────────────────────────────────

/// Thread-safe counters for metering AWS costs during a simulation run.
#[derive(Debug, Default)]
pub struct CostMetrics {
    // AppSync GraphQL mutations (total HTTP POST calls)
    pub graphql_mutations: AtomicU64,

    // Per-mutation counters (for DynamoDB cost breakdown)
    pub register_new_player: AtomicU64,
    pub buy_baby_yak: AtomicU64,
    pub sell_grown_yak: AtomicU64,
    pub buy_grown_yak: AtomicU64,
    pub sell_unsheared_yak: AtomicU64,
    pub buy_unsheared_yak: AtomicU64,
    pub sell_sheared_yak: AtomicU64,

    // WebSocket metrics
    /// Total outbound real-time update messages received across all bots
    pub ws_messages_received: AtomicU64,
    /// Total WebSocket connection duration in milliseconds (summed across all bots)
    pub ws_connection_ms: AtomicU64,
}

impl CostMetrics {
    pub fn new() -> Self {
        Self::default()
    }
}

// ── Cost Report ──────────────────────────────────────────────────────

fn cost(count: u64, price_per_million: f64) -> f64 {
    count as f64 * price_per_million / 1_000_000.0
}

pub fn print_cost_report(m: CostMetrics) {
    let mutations = m.graphql_mutations.load(Ordering::Relaxed);
    let ws_messages = m.ws_messages_received.load(Ordering::Relaxed);
    let ws_conn_ms = m.ws_connection_ms.load(Ordering::Relaxed);
    let ws_conn_min = ws_conn_ms as f64 / 60_000.0;

    let register = m.register_new_player.load(Ordering::Relaxed);
    let buy_baby = m.buy_baby_yak.load(Ordering::Relaxed);
    let sell_grown = m.sell_grown_yak.load(Ordering::Relaxed);
    let buy_grown = m.buy_grown_yak.load(Ordering::Relaxed);
    let sell_unsheared = m.sell_unsheared_yak.load(Ordering::Relaxed);
    let buy_unsheared = m.buy_unsheared_yak.load(Ordering::Relaxed);
    let sell_sheared = m.sell_sheared_yak.load(Ordering::Relaxed);

    // Compute DynamoDB totals from per-mutation profiles
    let mutation_counts: [(u64, DdbProfile); 7] = [
        (register, REGISTER_NEW_PLAYER),
        (buy_baby, BUY_BABY_YAK),
        (sell_grown, SELL_GROWN_YAK),
        (buy_grown, BUY_GROWN_YAK),
        (sell_unsheared, SELL_UNSHEARED_YAK),
        (buy_unsheared, BUY_UNSHEARED_YAK),
        (sell_sheared, SELL_SHEARED_YAK),
    ];

    let mut total_std_reads: u64 = 0;
    let mut total_std_writes: u64 = 0;
    let mut total_tx_writes: u64 = 0;

    for (count, profile) in mutation_counts {
        total_std_reads += count * profile.std_reads;
        total_std_writes += count * profile.std_writes;
        total_tx_writes += count * profile.tx_writes;
    }

    // Compute costs
    let appsync_mutation_cost = cost(mutations, APPSYNC_MUTATION_PRICE_PER_M);
    let appsync_realtime_cost = cost(ws_messages, APPSYNC_REALTIME_UPDATE_PRICE_PER_M);
    let appsync_connection_cost = ws_conn_min * APPSYNC_CONNECTION_MIN_PRICE_PER_M / 1_000_000.0;

    let ddb_std_read_cost = cost(total_std_reads, DDB_READ_PRICE_PER_M);
    let ddb_std_write_cost = cost(total_std_writes, DDB_WRITE_PRICE_PER_M);
    let ddb_tx_write_cost = cost(total_tx_writes, DDB_TX_WRITE_PRICE_PER_M);

    let appsync_total = appsync_mutation_cost + appsync_realtime_cost + appsync_connection_cost;
    let ddb_total = ddb_std_read_cost + ddb_std_write_cost + ddb_tx_write_cost;
    let grand_total = appsync_total + ddb_total;

    // Print report
    println!();
    println!("=== AWS Cost Estimate ===");
    println!();

    // Mutation breakdown
    println!("  Mutation Breakdown:");
    println!("    {:>24}  {:>7}", "Operation", "Count");
    if register > 0 {
        println!("    {:>24}  {:>7}", "registerNewPlayer", register);
    }
    if buy_baby > 0 {
        println!("    {:>24}  {:>7}", "buyBabyYak", buy_baby);
    }
    if sell_grown > 0 {
        println!("    {:>24}  {:>7}", "sellGrownYak", sell_grown);
    }
    if buy_grown > 0 {
        println!("    {:>24}  {:>7}", "buyGrownYak", buy_grown);
    }
    if sell_unsheared > 0 {
        println!("    {:>24}  {:>7}", "sellUnshearedYak", sell_unsheared);
    }
    if buy_unsheared > 0 {
        println!("    {:>24}  {:>7}", "buyUnshearedYak", buy_unsheared);
    }
    if sell_sheared > 0 {
        println!("    {:>24}  {:>7}", "sellShearedYak", sell_sheared);
    }
    println!("    {:>24}  {:>7}", "TOTAL mutations", mutations);
    println!();

    // AppSync costs
    println!("  AWS AppSync:");
    println!(
        "    {:>30}  {:>10}  {:>10}",
        "Dimension", "Quantity", "Cost"
    );
    println!(
        "    {:>30}  {:>10}  {:>10}",
        "GraphQL operations",
        format_qty(mutations),
        format_usd(appsync_mutation_cost)
    );
    println!(
        "    {:>30}  {:>10}  {:>10}",
        "Real-time updates",
        format_qty(ws_messages),
        format_usd(appsync_realtime_cost)
    );
    println!(
        "    {:>30}  {:>10}  {:>10}",
        "Connection-minutes",
        format!("{:.1}", ws_conn_min),
        format_usd(appsync_connection_cost)
    );
    println!(
        "    {:>30}  {:>10}  {:>10}",
        "AppSync subtotal",
        "",
        format_usd(appsync_total)
    );
    println!();

    // DynamoDB costs
    println!("  Amazon DynamoDB (on-demand):");
    println!("    {:>30}  {:>10}  {:>10}", "Dimension", "Units", "Cost");
    println!(
        "    {:>30}  {:>10}  {:>10}",
        "Standard reads (RRU)",
        format_qty(total_std_reads),
        format_usd(ddb_std_read_cost)
    );
    println!(
        "    {:>30}  {:>10}  {:>10}",
        "Standard writes (WRU)",
        format_qty(total_std_writes),
        format_usd(ddb_std_write_cost)
    );
    println!(
        "    {:>30}  {:>10}  {:>10}",
        "Transactional writes (WRU)",
        format_qty(total_tx_writes),
        format_usd(ddb_tx_write_cost)
    );
    println!(
        "    {:>30}  {:>10}  {:>10}",
        "DynamoDB subtotal",
        "",
        format_usd(ddb_total)
    );
    println!();

    // Grand total
    println!(
        "  {:>32}  {:>10}  {:>10}",
        "ESTIMATED TOTAL",
        "",
        format_usd(grand_total)
    );
    println!();
}

fn format_usd(amount: f64) -> String {
    if amount < 0.0001 && amount > 0.0 {
        format!("${:.6}", amount)
    } else {
        format!("${:.4}", amount)
    }
}

fn format_qty(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{n}")
    }
}
