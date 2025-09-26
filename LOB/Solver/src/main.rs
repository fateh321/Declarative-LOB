use essential_types::{convert::{word_4_from_u8_32, words_from_hex_str}, Key, Word, solution::{Solution, SolutionSet, Mutation}, contract::Contract, Program, PredicateAddress, ContentAddress};
use hex::decode;
use std::convert::TryInto;
use essential_app_utils::compile::compile_pint_project;
use regex::Regex;
use std::process::Stdio;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command as TokioCommand},
};
use std::time::{Instant, Duration};
use essential_node_types::BigBang;
use essential_app_utils as utils;
use tracing_subscriber;
use essential_hash;
use array_init::array_init;
mod abi;
use crate::abi::{deposit, withdraw, addLimitOrderBid, addLimitOrderAsk, removeLimitOrderBid, removeLimitOrderAsk, settle, settleMarketOrders, storage};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use sha3::{Digest, Keccak256};
use std::collections::{BTreeMap, VecDeque, HashMap};
use tokio::fs::File;
use std::env;
use std::ops::Bound::*;

fn main() {
    println!("Hello, world!");
    }

    // Define the LimitOrder struct with proper naming convention
    #[derive(Copy, Clone, Debug)]
    struct LimitOrder {
        max_amnt: i64,
        price: i64, //assume price is whole num for now (will have to be fixed point eventually)
        is_bid: bool,
        addr: [Word; 4],
        auth: [Word; 4],
        next_key: i64,
    }

    // Below is implementation of the Deposit predicate solution
    fn produce_solution_deposit(
        amount_0_delta: i64,
        amount_0_final: i64,
        amount_1_delta: i64,
        amount_1_final: i64,
        addr_word: [Word; 4],
        key_word: [Word; 4],
        auth_word: [Word; 4]
    ) -> Solution {
        let depoit_data = deposit::Vars{
            amount0: amount_0_delta,
            amount1: amount_1_delta,
            addr: addr_word,
            key: key_word,
            auth: auth_word,
        };
        let deposit_state_mutations: Vec<Mutation> = storage::mutations()
        .balances_0(|map| map.entry(addr_word, amount_0_final))
        .balances_1(|map| map.entry(addr_word, amount_1_final))
        .into();

        Solution {
            predicate_to_solve: deposit::ADDRESS,
            predicate_data: depoit_data.into(),
            state_mutations: deposit_state_mutations.into(),
        }
    }

    // Below is implementation of the Withdraw predicate solution
    fn produce_solution_withdraw(
        amount_0_delta: i64,
        amount_0_final: i64,
        amount_1_delta: i64,
        amount_1_final: i64,
        addr_word: [Word; 4],
        key_word: [Word; 4],
        auth_word: [Word; 4]
    ) -> Solution {
        let withdraw_data = withdraw::Vars{
            amount0: amount_0_delta,
            amount1: amount_1_delta,
            addr: addr_word,
            key: key_word,
            auth: auth_word,
        };
        let withdraw_state_mutations: Vec<Mutation> = storage::mutations()
        .balances_0(|map| map.entry(addr_word, amount_0_final))
        .balances_1(|map| map.entry(addr_word, amount_1_final))
        .into();

        Solution {
            predicate_to_solve: withdraw::ADDRESS,
            predicate_data: withdraw_data.into(),
            state_mutations: withdraw_state_mutations.into(),
        }
    }

    //Below is implementation of the AddLimitOrder predicate solution
    /*
    Notes: 
    - the linked list points from leading_key -> new_index -> trailing_key
    - index 0 is assigned for nil orders. Therefore, the last order's next_key is 0
    - if the leading_key is 0, then the new order is the first order in the orderbook
    - if the leading_key is not 0, then the new order is inserted in the orderbook
    */
    fn produce_solution_add_limit_order_bid(
        leading_key: i64,
        trailing_key: i64,
        new_order: LimitOrder,
        new_index: i64,
        leading_order_next: i64,
        first_order_index: i64,
    ) -> Solution {
        // Convert the LimitOrder struct to the expected tuple format
        let limit_order_tuple: (i64, i64, bool, [Word; 4], [Word; 4], i64) = (new_order.max_amnt, new_order.price, new_order.is_bid, new_order.addr, new_order.auth, new_order.next_key);
        let add_limit_order_data = addLimitOrderBid::Vars{
            leading_key: leading_key,
            trailing_key: trailing_key,
            new_order: limit_order_tuple,
            new_index: new_index,
        };
        let mut mutations = storage::mutations()
        .bid_orders(|map| 
            map.entry(new_index, |tup| 
                tup.max_amnt(new_order.max_amnt)
                .price(new_order.price)
                .isBid(new_order.is_bid)
                .addr(new_order.addr)
                .auth(new_order.auth)
                .next_key(new_order.next_key)
            )
        )
        .bid_orders(|map| 
            map.entry(leading_key, |tup| 
                tup.next_key(leading_order_next)
            )
        )
        .first_bid_order(first_order_index);

        let add_limit_order_state_mutations: Vec<Mutation> = mutations.into();
        
        Solution {
            predicate_to_solve: addLimitOrderBid::ADDRESS,
            predicate_data: add_limit_order_data.into(),
            state_mutations: add_limit_order_state_mutations.into(),
        }
    }

    fn produce_solution_remove_limit_order_bid(
        leading_key: i64,
        trailing_key: i64,
        middle_index: i64,
        leading_order_next: i64,
        first_order_index: i64,
    ) -> Solution {
        let remove_limit_order_data = removeLimitOrderBid::Vars{
            leading_key: leading_key,
            trailing_key: trailing_key,
            middle_index: middle_index,
        };
        let mut mutations = storage::mutations()
        .bid_orders(|map| 
            map.entry(middle_index, |tup| 
                tup.max_amnt(0)
                .price(0)
                .isBid(false)
                .addr([0,0,0,0])
                .auth([0,0,0,0])
                .next_key(0)
            )
        )
        .bid_orders(|map| 
            map.entry(leading_key, |tup| 
                tup.next_key(leading_order_next)
            )
        )
        .first_bid_order(first_order_index);
        let remove_limit_order_state_mutations: Vec<Mutation> = mutations.into();
        
        Solution {
            predicate_to_solve: removeLimitOrderBid::ADDRESS,
            predicate_data: remove_limit_order_data.into(),
            state_mutations: remove_limit_order_state_mutations.into(),
        }
    }

    fn produce_solution_add_limit_order_ask(
        leading_key: i64,
        trailing_key: i64,
        new_order: LimitOrder,
        new_index: i64,
        leading_order_next: i64,
        first_order_index: i64,
    ) -> Solution {
        // Convert the LimitOrder struct to the expected tuple format
        let limit_order_tuple: (i64, i64, bool, [Word; 4], [Word; 4], i64) = (new_order.max_amnt, new_order.price, new_order.is_bid, new_order.addr, new_order.auth, new_order.next_key);
        let add_limit_order_data = addLimitOrderAsk::Vars{
            leading_key: leading_key,
            trailing_key: trailing_key,
            new_order: limit_order_tuple,
            new_index: new_index,
        };
        let mut mutations = storage::mutations()
        .ask_orders(|map| 
            map.entry(new_index, |tup| 
                tup.max_amnt(new_order.max_amnt)
                .price(new_order.price)
                .isBid(new_order.is_bid)
                .addr(new_order.addr)
                .auth(new_order.auth)
                .next_key(new_order.next_key)
            )
        )
        .ask_orders(|map| 
            map.entry(leading_key, |tup| 
                tup.next_key(leading_order_next)
            )
        )
        .first_ask_order(first_order_index);

        let add_limit_order_state_mutations: Vec<Mutation> = mutations.into();
        
        Solution {
            predicate_to_solve: addLimitOrderAsk::ADDRESS,
            predicate_data: add_limit_order_data.into(),
            state_mutations: add_limit_order_state_mutations.into(),
        }
    }
    fn produce_solution_remove_limit_order_ask(
        leading_key: i64,
        trailing_key: i64,
        middle_index: i64,
        leading_order_next: i64,
        first_order_index: i64,
    ) -> Solution {
        let remove_limit_order_data = removeLimitOrderAsk::Vars{
            leading_key: leading_key,
            trailing_key: trailing_key,
            middle_index: middle_index,
        };
        let mut mutations = storage::mutations()
        .ask_orders(|map| 
            map.entry(middle_index, |tup| 
                tup.max_amnt(0)
                .price(0)
                .isBid(false)
                .addr([0,0,0,0])
                .auth([0,0,0,0])
                .next_key(0)
            )
        )
        .ask_orders(|map| 
            map.entry(leading_key, |tup| 
                tup.next_key(leading_order_next)
            )
        )
        .first_ask_order(first_order_index);
        let remove_limit_order_state_mutations: Vec<Mutation> = mutations.into();

        Solution {
            predicate_to_solve: removeLimitOrderAsk::ADDRESS,
            predicate_data: remove_limit_order_data.into(),
            state_mutations: remove_limit_order_state_mutations.into(),
        }
    }

    #[derive(Copy, Clone, Debug)]
    struct settle_order {
        index: i64,
        auth: [Word; 4],
    }
    //Below is implementation of the settle predicate solution
    fn produce_solution_settle(
        partial_amount_bid: i64,
        partial_amount_ask: i64,
        partial_bid_index: i64,
        partial_ask_index: i64,
        bid_orders: [settle_order; 10],
        ask_orders: [settle_order; 10],
        solver_orders: [LimitOrder; 2],
        address_list_bid: [[Word; 4]; 11], // the last index is the solver address
        address_list_ask: [[Word; 4]; 11], // the last index is the solver address
        amount_0_final_bid: [i64; 11],
        amount_1_final_bid: [i64; 11],
        amount_0_final_ask: [i64; 11],
        amount_1_final_ask: [i64; 11],
        first_bid_order: i64,
        first_ask_order: i64,
        final_bid_order: [LimitOrder; 10],
        final_ask_order: [LimitOrder; 10],
    ) -> Solution {
        // Convert the SettleOrder struct to the expected tuple format   
        let bid_orders_tuple: [(i64, [Word; 4]); 10] = array_init(|i| {
            let o = &bid_orders[i];
            (o.index, o.auth)
        });
        let ask_orders_tuple: [(i64, [Word; 4]); 10] = array_init(|i| {
            let o = &ask_orders[i];
            (o.index, o.auth)
        });
        let solver_orders_tuple: [(i64, i64, bool, [Word; 4], [Word; 4], i64); 2] = array_init(|i| {
            let o = &solver_orders[i];
            (o.max_amnt, o.price, o.is_bid, o.addr, o.auth, o.next_key)
        });
        let settle_data = settle::Vars{
            partial_amount_bid: partial_amount_bid,
            partial_amount_ask: partial_amount_ask,
            partial_bid_index: partial_bid_index,
            partial_ask_index: partial_ask_index,
            bid_orders: bid_orders_tuple,
            ask_orders: ask_orders_tuple,
            solver_orders: solver_orders_tuple,
        };

        let mut mutations = storage::mutations();
        for i in 0..11 {
            mutations = mutations.balances_0(|map| map.entry(address_list_bid[i], amount_0_final_bid[i]));
            mutations = mutations.balances_1(|map| map.entry(address_list_bid[i], amount_1_final_bid[i]));
            mutations = mutations.balances_0(|map| map.entry(address_list_ask[i], amount_0_final_ask[i]));
            mutations = mutations.balances_1(|map| map.entry(address_list_ask[i], amount_1_final_ask[i]));
        }
        mutations = mutations.first_bid_order(first_bid_order);
        mutations = mutations.first_ask_order(first_ask_order);
        for i in 0..10 {
            mutations = mutations.bid_orders(|map| map.entry(bid_orders[i].index, |tup| 
                tup.max_amnt(final_bid_order[i].max_amnt)
                .price(final_bid_order[i].price)
                .isBid(final_bid_order[i].is_bid)
                .addr(final_bid_order[i].addr)
                .auth(final_bid_order[i].auth)
                .next_key(final_bid_order[i].next_key)
            ));
        }
        for i in 0..10 {
            mutations = mutations.ask_orders(|map| map.entry(ask_orders[i].index, |tup| 
                tup.max_amnt(final_ask_order[i].max_amnt)
                .price(final_ask_order[i].price)
                .isBid(final_ask_order[i].is_bid)
                .addr(final_ask_order[i].addr)
                .auth(final_ask_order[i].auth)
                .next_key(final_ask_order[i].next_key)
            ));
        }
        let settle_state_mutations: Vec<Mutation> = mutations.into();

        Solution {
            predicate_to_solve: settle::ADDRESS,
            predicate_data: settle_data.into(),
            state_mutations: settle_state_mutations.into(),
        }
    }
    #[derive(Copy, Clone)]
    struct market_order {
        amount: i64,
        addr: [Word; 4],
        auth: [Word; 4],
    }
    fn produce_solution_market_order(
        partial_amount_bid: i64,
        partial_amount_ask: i64,
        partial_bid_index: i64,
        partial_ask_index: i64,
        bid_orders: [settle_order; 10],
        ask_orders: [settle_order; 10],
        bid_market_orders: [market_order; 10],
        ask_market_orders: [market_order; 10],
        average_price_bids: i64,
        average_price_asks: i64,
        solver_orders: [LimitOrder; 2],
        address_list_bid: [[Word; 4]; 11], // the last index is the solver address
        address_list_ask: [[Word; 4]; 11], // the last index is the solver address
        address_list_bid_market: [[Word; 4]; 10],
        address_list_ask_market: [[Word; 4]; 10],
        amount_0_final_bid: [i64; 11],
        amount_1_final_bid: [i64; 11],
        amount_0_final_ask: [i64; 11],
        amount_1_final_ask: [i64; 11],
        amount_0_final_bid_market: [i64; 10],
        amount_1_final_bid_market: [i64; 10],
        amount_0_final_ask_market: [i64; 10],
        amount_1_final_ask_market: [i64; 10],
        first_bid_order: i64,
        first_ask_order: i64,
        final_bid_order: [LimitOrder; 10],
        final_ask_order: [LimitOrder; 10],
    ) -> Solution {
        let mut bid_orders_tuple: [(i64, [Word; 4]); 10] = array_init(|i| {
            let o = &bid_orders[i];
            (o.index, o.auth)
        });
        let mut ask_orders_tuple: [(i64, [Word; 4]); 10] = array_init(|i| {
            let o = &ask_orders[i];
            (o.index, o.auth)
        });
        let mut bid_market_orders_tuple: [(i64, [Word; 4], [Word; 4]); 10] = array_init(|i| {
            let o = &bid_market_orders[i];
            (o.amount, o.addr, o.auth)
        });
        let mut ask_market_orders_tuple: [(i64, [Word; 4], [Word; 4]); 10] = array_init(|i| {
            let o = &ask_market_orders[i];
            (o.amount, o.addr, o.auth)
        });
        let mut solver_orders_tuple: [(i64, i64, bool, [Word; 4], [Word; 4], i64); 2] = array_init(|i| {
            let o = &solver_orders[i];
            (o.max_amnt, o.price, o.is_bid, o.addr, o.auth, o.next_key)
        });
        let market_order_data = settleMarketOrders::Vars{
            partial_amount_bid: partial_amount_bid,
            partial_amount_ask: partial_amount_ask,
            partial_bid_index: partial_bid_index,
            partial_ask_index: partial_ask_index,
            bid_market_orders: bid_market_orders_tuple,
            ask_market_orders: ask_market_orders_tuple,
            bid_limit_orders: bid_orders_tuple,
            ask_limit_orders: ask_orders_tuple,
            average_price_bids: average_price_bids,
            average_price_asks: average_price_asks,
            solver_orders: solver_orders_tuple,
        };
        let mut mutations = storage::mutations();
        // TODO: ideally, the below for loop should go from 0 to 11. But since solver is not mutable, we need to skip it
        for i in 0..10 {
            mutations = mutations.balances_0(|map| map.entry(address_list_bid[i], amount_0_final_bid[i]));
            mutations = mutations.balances_1(|map| map.entry(address_list_bid[i], amount_1_final_bid[i]));
            mutations = mutations.balances_0(|map| map.entry(address_list_ask[i], amount_0_final_ask[i]));
            mutations = mutations.balances_1(|map| map.entry(address_list_ask[i], amount_1_final_ask[i]));
        }
        for i in 0..10 {
            mutations = mutations.balances_0(|map| map.entry(address_list_bid_market[i], amount_0_final_bid_market[i]));
            mutations = mutations.balances_1(|map| map.entry(address_list_bid_market[i], amount_1_final_bid_market[i]));
            mutations = mutations.balances_0(|map| map.entry(address_list_ask_market[i], amount_0_final_ask_market[i]));
            mutations = mutations.balances_1(|map| map.entry(address_list_ask_market[i], amount_1_final_ask_market[i]));
        }
        mutations = mutations.first_bid_order(first_bid_order);
        mutations = mutations.first_ask_order(first_ask_order);
        for i in 0..10 {
            mutations = mutations.bid_orders(|map| map.entry(bid_orders[i].index, |tup| 
                tup.max_amnt(final_bid_order[i].max_amnt)
                .price(final_bid_order[i].price)
                .isBid(final_bid_order[i].is_bid)
                .addr(final_bid_order[i].addr)
                .auth(final_bid_order[i].auth)
                .next_key(final_bid_order[i].next_key)
            ));
        }
        for i in 0..10 {
            mutations = mutations.ask_orders(|map| map.entry(ask_orders[i].index, |tup| 
                tup.max_amnt(final_ask_order[i].max_amnt)
                .price(final_ask_order[i].price)
                .isBid(final_ask_order[i].is_bid)
                .addr(final_ask_order[i].addr)
                .auth(final_ask_order[i].auth)
                .next_key(final_ask_order[i].next_key)
            ));
        }
        let market_order_state_mutations: Vec<Mutation> = mutations.into();
        
        Solution {
            predicate_to_solve: settleMarketOrders::ADDRESS,
            predicate_data: market_order_data.into(),
            state_mutations: market_order_state_mutations.into(),
        }
    }
    
    // Function to convert a hex string to [i64;4]
    fn hex_to_i64_array(hex_str: &str) -> [i64; 4] {
        // Remove "0x" prefix if present
        let clean_hex = hex_str.trim_start_matches("0x");
        // println!("clean_hex: {:?}", clean_hex);
        
        // Convert hex string to bytes
        let mut bytes = [0u8; 32]; // 256 bits = 32 bytes
        let decoded = decode(clean_hex).expect("Invalid hex string");
        // println!("decoded: {:?}", decoded);
        
        // Copy decoded bytes to our fixed-size array, handling the case where input might be too short
        let copy_len = std::cmp::min(decoded.len(), 32);
        bytes[..copy_len].copy_from_slice(&decoded[..copy_len]);
        
        // Use the imported word_4_from_u8_32 function to convert bytes to [Word; 4]
        let word_array: [Word; 4] = word_4_from_u8_32(bytes);
        // println!("word_array: {:?}", word_array);
        word_array

    }

    fn hex_to_word_array(hex_str: &str) -> [Word; 4] {
        let clean_hex = hex_str.trim_start_matches("0x");
        let word_vec = words_from_hex_str(clean_hex);
        println!("word_vec: {:?}", word_vec);
        word_vec.expect("Error while converting result")
        .try_into()
        .expect("Vector length is not 4")
    }

    pub fn balances_0_key(address: [Word; 4]) -> Key {
        pint_abi::gen_from_file! {
            abi: "../PintLOB/orderbook/out/debug/orderbook-abi.json",
            contract: "../PintLOB/orderbook/out/debug/orderbook.json",
        }
        let balance: Vec<_> = storage::keys::keys()
            .balances_0(|e| e.entry(address))
            .into();
        balance.into_iter().next().expect("Must be a key")
    }

    pub fn balances_1_key(address: [Word; 4]) -> Key {
        pint_abi::gen_from_file! {
            abi: "../PintLOB/orderbook/out/debug/orderbook-abi.json",
            contract: "../PintLOB/orderbook/out/debug/orderbook.json",
        }
        let balance: Vec<_> = storage::keys::keys()
            .balances_1(|e| e.entry(address))
            .into();
        balance.into_iter().next().expect("Must be a key")
    }

    pub fn fetch_bid_order_keys(index: i64) -> Key {
        pint_abi::gen_from_file! {
            abi: "../PintLOB/orderbook/out/debug/orderbook-abi.json",
            contract: "../PintLOB/orderbook/out/debug/orderbook.json",
        }
        let keys: Vec<Key> = storage::keys()
        .bid_orders(|map| map.entry(index, |tup| 
        tup.max_amnt()
        .price()
        .isBid()
        .addr()
        .auth()
        .next_key()
        )
        )
        .into();
        keys[0].clone()
    }

fn generate_random_hash(rng: &mut StdRng) -> String {
    let mut random_bytes = [0u8; 32];
    rng.fill(&mut random_bytes);
    let hash = Keccak256::digest(&random_bytes);
    format!("0x{}", hex::encode(hash))
}
fn generate_index(rng: &mut StdRng) -> i64 {
    rng.gen_range(1..=u32::MAX) as i64   // excludes 0
}

#[derive(Debug, Clone)]
struct Order {
    index: i64,
    max_amnt: i64,
    price: i64,
    is_bid: bool,
    addr: [Word; 4],
    auth: [Word; 4],
}

type PriceLevel = VecDeque<Order>; // earliest order at the front

#[derive(Debug)]
struct OrderBook {
    bids: BTreeMap<u64, PriceLevel>, // descending when matching
    asks: BTreeMap<u64, PriceLevel>, // ascending when matching
}

#[derive(Debug)]
struct OrderBookParse {
    bids: BTreeMap<u64, i64>,
    asks: BTreeMap<u64, i64>,
    middle_price: i64,
}

async fn parse_orderbook_file(file_path: &str) -> (Vec<OrderBookParse>, Vec<i64>) {
    let file = File::open(file_path).await.expect("Failed to open file");
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let mut orderbooks = Vec::new();
    let mut price_seq = Vec::new();
    let mut current_price: Option<i64> = None;
    let mut current = OrderBookParse {
        bids: BTreeMap::new(),
        asks: BTreeMap::new(),
        middle_price: 0,
    };

    let re_bids = Regex::new(r"^delta in bids: \[(.*)\]").unwrap();
    let re_asks = Regex::new(r"^delta in asks: \[(.*)\]").unwrap();
    let re_price = Regex::new(r"^price: ([\d\.]+)").unwrap();
    let tuple_re = Regex::new(r"\(([\d.]+), ([\d.]+)\)").unwrap();

    while let Some(line) = lines.next_line().await.unwrap() {
        let line = line.trim().to_string();

        if line.starts_with("step:") {
            if !current.bids.is_empty() || !current.asks.is_empty() {
                if let Some(p) = current_price {
                    current.middle_price = p;
                    price_seq.push(p);
                }
                orderbooks.push(current);
                current = OrderBookParse {
                    bids: BTreeMap::new(),
                    asks: BTreeMap::new(),
                    middle_price: 0,
                };
                current_price = None;
            }
        } else if let Some(caps) = re_price.captures(&line) {
            let price: f64 = caps[1].parse().unwrap();
            current_price = Some((price * 10.0).round() as i64);
        } else if let Some(caps) = re_bids.captures(&line) {
            let content = &caps[1];
            for tup in tuple_re.captures_iter(content) {
                let price: f64 = tup[1].parse().unwrap();
                let qty: f64 = tup[2].parse().unwrap();
                current.bids.insert((price * 10.0).round() as u64, (qty * 10.0).round() as i64);
            }
        } else if let Some(caps) = re_asks.captures(&line) {
            let content = &caps[1];
            for tup in tuple_re.captures_iter(content) {
                let price: f64 = tup[1].parse().unwrap();
                let qty: f64 = tup[2].parse().unwrap();
                current.asks.insert((price * 10.0).round() as u64, (qty * 10.0).round() as i64);
            }
        }
    }

    if !current.bids.is_empty() || !current.asks.is_empty() {
        if let Some(p) = current_price {
            current.middle_price = p;
            price_seq.push(p);
        }
        orderbooks.push(current);
    }

    (orderbooks, price_seq)
}

// Moving the test outside of the main function so it can be properly discovered
#[cfg(test)]
mod tests {
    use super::*;

     //Next, let's start the essential-builder
    //  #[tokio::test]
    #[tokio::test(flavor = "multi_thread")]
     async fn test_add_limit_order() {
            // Convert the addresses for our order
        let _addr_zero_i64 = hex_to_i64_array("0x0000000000000000000000000000000000000000000000000000000000000000");
         let _addr0_i64 = hex_to_i64_array("0x5B5F934E382FDC4AD1C4AB2448B32BD66B5C53D5A3D5166A9EF48CB6DB3B2B95");
         let _addr1_i64 = hex_to_i64_array("0x7AE73AE363588924F50D5B87F807642B7193D2A0265B451000FAE4318007CD86");
         let _addr2_i64 = hex_to_i64_array("0x5F9C2BD1A47E8039D1A3B687DCE92F33A187E904B61D2A3C9F82C0EF99B72D41");
         let _auth_i64 = hex_to_i64_array("0x7AE73AE363588924F50D5B87F807642B7193D2A0265B451000FAE4318007CD86");
         let _addr_word = hex_to_word_array("0x5B5F934E382FDC4AD1C4AB2448B32BD66B5C53D5A3D5166A9EF48CB6DB3B2B95");
         let _auth_word = hex_to_word_array("0x7AE73AE363588924F50D5B87F807642B7193D2A0265B451000FAE4318007CD86");
    
         // Load the contract bytecode
         tracing_subscriber::fmt::init();
         let contract_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../PintLOB/orderbook").into();
         let (orderbook, programs): (Contract, Vec<Program>) =
             compile_pint_project(contract_path).await.unwrap();
     
         let contract_address = essential_hash::contract_addr::from_contract(&orderbook);
         let predicate_address = essential_hash::content_addr(&orderbook.predicates[2]);
         println!("predicate_address: {:?}", predicate_address);
         let predicate_address = PredicateAddress {
             contract: contract_address,
             predicate: predicate_address,
         };
 
        //  println!("orderbook: {:?}", orderbook);
        //  println!("programs: {:?}", programs);
         
        // Initialize the database
         
         let dbs = utils::db::new_dbs().await;
 
         // Load the node types
         let big_bang = BigBang::default();
         // Deploy the contract
 
         let contract_registry = big_bang.contract_registry;
         let program_registry = big_bang.program_registry;
         essential_app_utils::deploy::register_contract_and_programs(
             &dbs.builder,
             &contract_registry,
             &program_registry,
             &orderbook,
             programs,
         )
         .await
         .unwrap();

        // Next, let's submit a solution
        let solution0 = produce_solution_deposit(
            10000,         //amount_0_delta: i64,
            10000,         //amount_0_final: i64,
            100,         //amount_1_delta: i64,
            100,         //amount_1_final: i64,
            _addr0_i64,   //addr_word: [Word; 4],
            _addr0_i64,   //key_word: [Word; 4],
            _addr0_i64    //auth_word: [Word; 4]
            );

        let solution1 = produce_solution_deposit(
            10000,         //amount_0_delta: i64,
            10000,         //amount_0_final: i64,
            100,         //amount_1_delta: i64,
            100,         //amount_1_final: i64,
            _addr1_i64,   //addr_word: [Word; 4],
            _addr1_i64,   //key_word: [Word; 4],
            _addr1_i64    //auth_word: [Word; 4]
        );

        let solution_set = SolutionSet {
            solutions: vec![solution0, solution1],
        };
        // println!("solution_set: {:?}", solution_set);
    
        utils::builder::submit(&dbs.builder, solution_set.clone())
        .await
        .unwrap();
        let t1 = Instant::now();
        // validate the solution
        let result = utils::node::validate_solution(&dbs.node, solution_set.clone())
        .await
        .unwrap();
        println!("result: {:?}", result);
        println!("⏱️ validate_solution took: {:?}", t1.elapsed());

        // Build a block
        let t0 = Instant::now();
        let o = utils::builder::build_default(&dbs).await.unwrap();
        println!("o: {:?}", o);
        println!("⏱️ build_default took: {:?}", t0.elapsed());
        assert!(o.failed.is_empty(), "{:?}", o.failed);

        let solutionAddBid = produce_solution_add_limit_order_bid(
            0, // leading_key
            0, // trailing_key
            LimitOrder {
                max_amnt: 100,
                price: 100,
                is_bid: true,
                addr: _addr0_i64,
                auth: _auth_i64,
                next_key: 0,
            },
            1, // new_index
            0, // leading_order_next
            1, // first_order_index
        );

        let solutionAddBid2 = produce_solution_add_limit_order_bid(
            0, // leading_key
            0, // trailing_key
            LimitOrder {
                max_amnt: 100,
                price: 100,
                is_bid: true,
                addr: _addr0_i64,
                auth: _auth_i64,
                next_key: 0,
            },
            1, // new_index
            0, // leading_order_next
            1, // first_order_index
        );

        let solutionAddAsk = produce_solution_add_limit_order_ask(
            0, // leading_key
            0, // trailing_key
            LimitOrder {
                max_amnt: 100,
                price: 100,
                is_bid: false,
                addr: _addr1_i64,
                auth: _auth_i64,
                next_key: 0,
            },
            1, // new_index
            0, // leading_order_next    
            1, // first_order_index
        );

        let solution_set_add = SolutionSet {
            // solutions: vec![solutionAddBid.clone(), solutionAddAsk.clone()],
            solutions: vec![solutionAddBid.clone()],
        };
        // println!("solution_set: {:?}", solution_set);
    
        utils::builder::submit(&dbs.builder, solution_set_add.clone())
        .await
        .unwrap();

        // validate the solution
        let t1 = Instant::now();
        let result = utils::node::validate_solution(&dbs.node, solution_set_add.clone())
        .await
        .unwrap();
        println!("⏱️ validate_solution took: {:?}", t1.elapsed());
        println!("result: {:?}", result);

        // Build a block
        let t0 = Instant::now();
        let o = utils::builder::build_default(&dbs).await.unwrap();
        println!("o: {:?}", o);
        println!("⏱️ build_default took: {:?}", t0.elapsed());
        assert!(o.failed.is_empty(), "{:?}", o.failed);

        let solution_set = SolutionSet {
            solutions: vec![solutionAddAsk.clone()],
        };
        // println!("solution_set: {:?}", solution_set);
    
        utils::builder::submit(&dbs.builder, solution_set.clone())
        .await
        .unwrap();

        // validate the solution
        let result = utils::node::validate_solution(&dbs.node, solution_set.clone())
        .await
        .unwrap();

        println!("result: {:?}", result);

        // Build a block
        let t0 = Instant::now();
        let o = utils::builder::build_default(&dbs).await.unwrap();
        println!("o: {:?}", o);
        println!("⏱️ build_default took: {:?}", t0.elapsed());
        assert!(o.failed.is_empty(), "{:?}", o.failed);

        let bid_order_keys = fetch_bid_order_keys(1);
        println!("bid_order_keys: {:?}", bid_order_keys);
        let bid_order_amount = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &bid_order_keys)
         .await
         .unwrap();
        //  assert_eq!(r, Some(vec![100]));
         println!("bid_order_amount: {:?}", bid_order_amount);

         let balance_0_key_addr0 = balances_0_key(_addr0_i64);
         let balance_1_key_addr0 = balances_1_key(_addr0_i64);
         let balance_0_key_addr1 = balances_0_key(_addr1_i64);
         let balance_1_key_addr1 = balances_1_key(_addr1_i64);
         
         let balance_0_addr0 = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_0_key_addr0).await.unwrap();
         let balance_1_addr0 = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_1_key_addr0).await.unwrap();
         let balance_0_addr1 = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_0_key_addr1).await.unwrap();
         let balance_1_addr1 = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_1_key_addr1).await.unwrap();

         println!("balance_0_addr0: {:?}", balance_0_addr0);
         println!("balance_1_addr0: {:?}", balance_1_addr0);
         println!("balance_0_addr1: {:?}", balance_0_addr1);
         println!("balance_1_addr1: {:?}", balance_1_addr1);
     }



     #[tokio::test]
     async fn test_settle_limit_order() {
            // Convert the addresses for our order
        let _addr_zero_i64 = hex_to_i64_array("0x0000000000000000000000000000000000000000000000000000000000000000");
         let _addr0_i64 = hex_to_i64_array("0x5B5F934E382FDC4AD1C4AB2448B32BD66B5C53D5A3D5166A9EF48CB6DB3B2B95");
         let _addr1_i64 = hex_to_i64_array("0x7AE73AE363588924F50D5B87F807642B7193D2A0265B451000FAE4318007CD86");
         let _addr2_i64 = hex_to_i64_array("0x5F9C2BD1A47E8039D1A3B687DCE92F33A187E904B61D2A3C9F82C0EF99B72D41");
         let _auth_i64 = hex_to_i64_array("0x7AE73AE363588924F50D5B87F807642B7193D2A0265B451000FAE4318007CD86");
         let _addr_word = hex_to_word_array("0x5B5F934E382FDC4AD1C4AB2448B32BD66B5C53D5A3D5166A9EF48CB6DB3B2B95");
         let _auth_word = hex_to_word_array("0x7AE73AE363588924F50D5B87F807642B7193D2A0265B451000FAE4318007CD86");
    
         // Load the contract bytecode
        //  tracing_subscriber::fmt::init();
         let contract_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../PintLOB/orderbook").into();
         let (orderbook, programs): (Contract, Vec<Program>) =
             compile_pint_project(contract_path).await.unwrap();
     
         let contract_address = essential_hash::contract_addr::from_contract(&orderbook);
         let predicate_address = essential_hash::content_addr(&orderbook.predicates[2]);
         println!("predicate_address: {:?}", predicate_address);
         let predicate_address = PredicateAddress {
             contract: contract_address,
             predicate: predicate_address,
         };
 
         
         // Initialize the database
         
         let dbs = utils::db::new_dbs().await;
 
         // Load the node types
         let big_bang = BigBang::default();
         // Deploy the contract
 
         let contract_registry = big_bang.contract_registry;
         let program_registry = big_bang.program_registry;
         essential_app_utils::deploy::register_contract_and_programs(
             &dbs.builder,
             &contract_registry,
             &program_registry,
             &orderbook,
             programs,
         )
         .await
         .unwrap();

        // Next, let's submit a solution
        let solution0 = produce_solution_deposit(
            10000,         //amount_0_delta: i64,
            10000,         //amount_0_final: i64,
            100,         //amount_1_delta: i64,
            100,         //amount_1_final: i64,
            _addr0_i64,   //addr_word: [Word; 4],
            _addr0_i64,   //key_word: [Word; 4],
            _addr0_i64    //auth_word: [Word; 4]
            );

        let solution1 = produce_solution_deposit(
            10000,         //amount_0_delta: i64,
            10000,         //amount_0_final: i64,
            100,         //amount_1_delta: i64,
            100,         //amount_1_final: i64,
            _addr1_i64,   //addr_word: [Word; 4],
            _addr1_i64,   //key_word: [Word; 4],
            _addr1_i64    //auth_word: [Word; 4]
        );

        let solution_set = SolutionSet {
            solutions: vec![solution0, solution1],
        };
        // println!("solution_set: {:?}", solution_set);
    
        utils::builder::submit(&dbs.builder, solution_set.clone())
        .await
        .unwrap();

        // validate the solution
        let result = utils::node::validate_solution(&dbs.node, solution_set.clone())
        .await
        .unwrap();
        println!("result: {:?}", result);

        // Build a block
        let t0 = Instant::now();
        let o = utils::builder::build_default(&dbs).await.unwrap();
        println!("o: {:?}", o);
        println!("⏱️ build_default took: {:?}", t0.elapsed());
        assert!(o.failed.is_empty(), "{:?}", o.failed);

        let solution0 = produce_solution_add_limit_order_bid(
            0, // leading_key
            0, // trailing_key
            LimitOrder {
                max_amnt: 100,
                price: 100,
                is_bid: true,
                addr: _addr0_i64,
                auth: _auth_i64,
                next_key: 0,
            },
            1, // new_index
            0, // leading_order_next
            1, // first_order_index
        );

        let solution1 = produce_solution_add_limit_order_ask(
            0, // leading_key
            0, // trailing_key
            LimitOrder {
                max_amnt: 100,
                price: 100,
                is_bid: false,
                addr: _addr1_i64,
                auth: _auth_i64,
                next_key: 0,
            },
            1, // new_index
            0, // leading_order_next    
            1, // first_order_index
        );

        let solution_set = SolutionSet {
            solutions: vec![solution0],
        };
        // println!("solution_set: {:?}", solution_set);
    
        utils::builder::submit(&dbs.builder, solution_set.clone())
        .await
        .unwrap();

        // validate the solution
        let result = utils::node::validate_solution(&dbs.node, solution_set.clone())
        .await
        .unwrap();

        println!("result: {:?}", result);

        // Build a block
        let t0 = Instant::now();
        let o = utils::builder::build_default(&dbs).await.unwrap();
        println!("o: {:?}", o);
        println!("⏱️ build_default took: {:?}", t0.elapsed());
        assert!(o.failed.is_empty(), "{:?}", o.failed);

        let solution_set = SolutionSet {
            solutions: vec![solution1],
        };
        // println!("solution_set: {:?}", solution_set);
    
        utils::builder::submit(&dbs.builder, solution_set.clone())
        .await
        .unwrap();

        // validate the solution
        let result = utils::node::validate_solution(&dbs.node, solution_set.clone())
        .await
        .unwrap();

        println!("result: {:?}", result);

        // Build a block
        let t0 = Instant::now();
        let o = utils::builder::build_default(&dbs).await.unwrap();
        println!("o: {:?}", o);
        println!("⏱️ build_default took: {:?}", t0.elapsed());
        assert!(o.failed.is_empty(), "{:?}", o.failed);

        // Initialize the bid_orders array
        let mut bid_orders: [settle_order; 10] = [settle_order { index: 0, auth: _addr_zero_i64 }; 10];
        bid_orders[0] = settle_order { index: 1, auth: _auth_i64 };
        // Initialize the ask_orders array
        let mut ask_orders: [settle_order; 10] = [settle_order { index: 0, auth: _addr_zero_i64 }; 10];
        ask_orders[0] = settle_order { index: 1, auth: _auth_i64 };
        let solver_orders = [
            LimitOrder { max_amnt: 0, price: 0, is_bid: true, addr: _addr2_i64, auth: _auth_i64, next_key: 0 },
            LimitOrder { max_amnt: 0, price: 0, is_bid: false, addr: _addr2_i64, auth: _auth_i64, next_key: 0 }
         ];
        let mut address_list_bid: [[Word; 4]; 11] = [_addr_zero_i64; 11];
        address_list_bid[0] = _addr0_i64;
        address_list_bid[10] = _addr2_i64;
        let mut address_list_ask: [[Word; 4]; 11] = [_addr_zero_i64; 11];
        address_list_ask[0] = _addr1_i64;
        address_list_ask[10] = _addr2_i64;
        let mut amount_0_final_bid = [0; 11];
        let mut amount_1_final_bid = [0; 11];
        amount_1_final_bid[0] = 200;
        let mut amount_0_final_ask = [0; 11];
        amount_0_final_ask[0] = 20000;
        let mut amount_1_final_ask = [0; 11];
        let first_bid_order = 0;
        let first_ask_order = 0;
        let final_bid_order = [LimitOrder { max_amnt: 0, price: 0, is_bid: false, addr: _addr_zero_i64, auth: _addr_zero_i64, next_key: 0 }; 10];
        let final_ask_order = [LimitOrder { max_amnt: 0, price: 0, is_bid: false, addr: _addr_zero_i64, auth: _addr_zero_i64, next_key: 0 }; 10];
        let solution0 = produce_solution_settle(
            100, // partial_amount_bid
            100, // partial_amount_ask
            0, // partial_bid_index
            0, // partial_ask_index
            bid_orders, // bid_orders: [settle_order; 10],
            ask_orders, // ask_orders: [settle_order; 10],
            solver_orders, // solver_orders: [LimitOrder; 2],
            address_list_bid, // address_list: [[Word; 4]; 11], // the last index is the solver address
            address_list_ask, // address_list: [[Word; 4]; 11], // the last index is the solver address
            amount_0_final_bid, // amount_0_final: [i64; 11],
            amount_1_final_bid, // amount_1_final: [i64; 11],
            amount_0_final_ask, // amount_0_final: [i64; 11],
            amount_1_final_ask, // amount_1_final: [i64; 11],
            first_bid_order, // first_bid_order: i64,
            first_ask_order, // first_ask_order: i64,
            final_bid_order, // final_bid_order: [LimitOrder; 10],
            final_ask_order, // final_ask_order: [LimitOrder; 10],
        );
        let solution_set = SolutionSet {
            solutions: vec![solution0],
        };
        // println!("solution_set: {:?}", solution_set);
    
        utils::builder::submit(&dbs.builder, solution_set.clone())
        .await
        .unwrap();

        // validate the solution
        let result = utils::node::validate_solution(&dbs.node, solution_set.clone())
        .await
        .unwrap();

        println!("result: {:?}", result);

        // Build a block
        let t0 = Instant::now();
        let o = utils::builder::build_default(&dbs).await.unwrap();
        println!("o: {:?}", o);
        println!("⏱️ build_default took: {:?}", t0.elapsed());
        assert!(o.failed.is_empty(), "{:?}", o.failed);
        let bid_order_keys = fetch_bid_order_keys(1);
        println!("bid_order_keys: {:?}", bid_order_keys);
        let bid_order_amount = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &bid_order_keys)
         .await
         .unwrap();
        //  assert_eq!(r, Some(vec![100]));
         println!("bid_order_amount: {:?}", bid_order_amount);

         let balance_0_key_addr0 = balances_0_key(_addr0_i64);
         let balance_1_key_addr0 = balances_1_key(_addr0_i64);
         let balance_0_key_addr1 = balances_0_key(_addr1_i64);
         let balance_1_key_addr1 = balances_1_key(_addr1_i64);
         
         let balance_0_addr0 = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_0_key_addr0).await.unwrap();
         let balance_1_addr0 = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_1_key_addr0).await.unwrap();
         let balance_0_addr1 = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_0_key_addr1).await.unwrap();
         let balance_1_addr1 = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_1_key_addr1).await.unwrap();

         println!("balance_0_addr0: {:?}", balance_0_addr0);
         println!("balance_1_addr0: {:?}", balance_1_addr0);
         println!("balance_0_addr1: {:?}", balance_0_addr1);
         println!("balance_1_addr1: {:?}", balance_1_addr1);
     }

     #[tokio::test]
     async fn test_market_order() {
            // Convert the addresses for our order
        let _addr_zero_i64 = hex_to_i64_array("0x0000000000000000000000000000000000000000000000000000000000000000");
        let _addr_i64 = [
            hex_to_i64_array("0x5B5F934E382FDC4AD1C4AB2448B32BD66B5C53D5A3D5166A9EF48CB6DB3B2B95"),
            hex_to_i64_array("0x7AE73AE363588924F50D5B87F807642B7193D2A0265B451000FAE4318007CD86"),
            hex_to_i64_array("0x5F9C2BD1A47E8039D1A3B687DCE92F33A187E904B61D2A3C9F82C0EF99B72D41"),
            hex_to_i64_array("0x1D3A4F5B7E92834A9C82F7D1E4C73FAD10562E89AC3489F00E217C5DAAB03129"),
            hex_to_i64_array("0xA6B1D47F84392ECBE9D54263A1F73AD40EF73C93D98F4DA51C902B6F776C8BEE"),
            hex_to_i64_array("0x4FAD62397BE8D64CE7B0A3DC129C7A03F56AB0C9D22ADDA2F1EC72D35C7B39A0"),
            hex_to_i64_array("0x936E1C27B9F04A7D01A6B5B193846C903AF45D2379D08A8EB21C75EF9A543621"),
            hex_to_i64_array("0x8B67E24C3DF159A2B4E6D1FA23409C77EFA8BCDA45D6AEF33D19BEA0183F69C4"),
            hex_to_i64_array("0x2D9B7A1ECFA4761BD3C287E4B5F1A8DA01F6C930E8B3AA76F7C13E6DDEF0E111"),
            hex_to_i64_array("0xEC0148A993D273FC7B021D69E3C7A5B1FA98317C9E6D4AB2A03B78E351B3F294"),
        ];        
        let _auth_word = hex_to_word_array("0x7AE73AE363588924F50D5B87F807642B7193D2A0265B451000FAE4318007CD86");
        let _auth_i64 = hex_to_i64_array("0x7AE73AE363588924F50D5B87F807642B7193D2A0265B451000FAE4318007CD86");
    
         // Load the contract bytecode
        //  tracing_subscriber::fmt::init(); // need to initialize the logger only once
         let contract_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../PintLOB/orderbook").into();
         let (orderbook, programs): (Contract, Vec<Program>) =
             compile_pint_project(contract_path).await.unwrap();
     
         let contract_address = essential_hash::contract_addr::from_contract(&orderbook);
         let predicate_address = essential_hash::content_addr(&orderbook.predicates[2]);
         println!("predicate_address: {:?}", predicate_address);
         let predicate_address = PredicateAddress {
             contract: contract_address,
             predicate: predicate_address,
         };
 
        //  println!("orderbook: {:?}", orderbook);
        //  println!("programs: {:?}", programs);
         
         // Initialize the database
         
         let dbs = utils::db::new_dbs().await;
 
         // Load the node types
         let big_bang = BigBang::default();
         // Deploy the contract
 
         let contract_registry = big_bang.contract_registry;
         let program_registry = big_bang.program_registry;
         essential_app_utils::deploy::register_contract_and_programs(
             &dbs.builder,
             &contract_registry,
             &program_registry,
             &orderbook,
             programs,
         )
         .await
         .unwrap();
        // Next, let's submit a solution
        // bid address
        let solution0 = produce_solution_deposit(
            10000,         //amount_0_delta: i64,
            10000,         //amount_0_final: i64,
            100,         //amount_1_delta: i64,
            100,         //amount_1_final: i64,
            _addr_i64[0],   //addr_word: [Word; 4],
            _addr_i64[0],   //key_word: [Word; 4],
            _auth_word    //auth_word: [Word; 4]
            );
        // ask address
        let solution1 = produce_solution_deposit(
            10000,         //amount_0_delta: i64,
            10000,         //amount_0_final: i64,
            100,         //amount_1_delta: i64,
            100,         //amount_1_final: i64,
            _addr_i64[1],   //addr_word: [Word; 4],
            _addr_i64[1],   //key_word: [Word; 4],
            _auth_word    //auth_word: [Word; 4]
        );
        // bid market order address 1
        let solution2 = produce_solution_deposit(
            10000,         //amount_0_delta: i64,
            10000,         //amount_0_final: i64,
            100,         //amount_1_delta: i64,
            100,         //amount_1_final: i64,
            _addr_i64[2],   //addr_word: [Word; 4],
            _addr_i64[2],   //key_word: [Word; 4],
            _auth_word    //auth_word: [Word; 4]
        );
        // bid market order address 2
        let solution3 = produce_solution_deposit(
            10000,         //amount_0_delta: i64,
            10000,         //amount_0_final: i64,
            100,         //amount_1_delta: i64,
            100,         //amount_1_final: i64,
            _addr_i64[3],   //addr_word: [Word; 4],
            _addr_i64[3],   //key_word: [Word; 4],
            _auth_word    //auth_word: [Word; 4]
        );
        // ask market order address 1
        let solution4 = produce_solution_deposit(
            10000,         //amount_0_delta: i64,
            10000,         //amount_0_final: i64,
            100,         //amount_1_delta: i64,
            100,         //amount_1_final: i64, 
            _addr_i64[4],   //addr_word: [Word; 4],
            _addr_i64[4],   //key_word: [Word; 4],
            _auth_word    //auth_word: [Word; 4]
        );
        // ask market order address 2
        let solution5 = produce_solution_deposit(
            10000,         //amount_0_delta: i64,
            10000,         //amount_0_final: i64,
            100,         //amount_1_delta: i64,
            100,         //amount_1_final: i64,
            _addr_i64[5],   //addr_word: [Word; 4],
            _addr_i64[5],   //key_word: [Word; 4],
            _auth_word    //auth_word: [Word; 4]
        );
        let solution_set = SolutionSet {
            solutions: vec![solution0, solution1, solution2, solution3, solution4, solution5],
        };
        // println!("solution_set: {:?}", solution_set);
    
        utils::builder::submit(&dbs.builder, solution_set.clone())
        .await
        .unwrap();

        // validate the solution
        let result = utils::node::validate_solution(&dbs.node, solution_set.clone())
        .await
        .unwrap();
        println!("result: {:?}", result);

        // Build a block
        let t0 = Instant::now();
        let o = utils::builder::build_default(&dbs).await.unwrap();
        println!("o: {:?}", o);
        println!("⏱️ build_default took: {:?}", t0.elapsed());
        assert!(o.failed.is_empty(), "{:?}", o.failed);

        let solution0 = produce_solution_add_limit_order_bid(
            0, // leading_key
            0, // trailing_key
            LimitOrder {
                max_amnt: 100,
                price: 100,
                is_bid: true,
                addr: _addr_i64[0],
                auth: _auth_i64,
                next_key: 0,
            },
            1, // new_index
            0, // leading_order_next
            1, // first_order_index
        );

        let solution1 = produce_solution_add_limit_order_ask(
            0, // leading_key
            0, // trailing_key
            LimitOrder {
                max_amnt: 100,
                price: 100,
                is_bid: false,
                addr: _addr_i64[1],
                auth: _auth_i64,
                next_key: 0,
            },
            1, // new_index
            0, // leading_order_next    
            1, // first_order_index
        );

        let solution_set = SolutionSet {
            solutions: vec![solution0],
        };
        // println!("solution_set: {:?}", solution_set);
    
        utils::builder::submit(&dbs.builder, solution_set.clone())
        .await
        .unwrap();

        // validate the solution
        let result = utils::node::validate_solution(&dbs.node, solution_set.clone())
        .await
        .unwrap();

        println!("result: {:?}", result);

        // Build a block
        let t0 = Instant::now();
        let o = utils::builder::build_default(&dbs).await.unwrap();
        println!("o: {:?}", o);
        println!("⏱️ build_default took: {:?}", t0.elapsed());
        assert!(o.failed.is_empty(), "{:?}", o.failed);

        let solution_set = SolutionSet {
            solutions: vec![solution1],
        };
        // println!("solution_set: {:?}", solution_set);
    
        utils::builder::submit(&dbs.builder, solution_set.clone())
        .await
        .unwrap();

        // validate the solution
        let result = utils::node::validate_solution(&dbs.node, solution_set.clone())
        .await
        .unwrap();

        println!("result: {:?}", result);

        // Build a block
        let t0 = Instant::now();
        let o = utils::builder::build_default(&dbs).await.unwrap();
        println!("o: {:?}", o);
        println!("⏱️ build_default took: {:?}", t0.elapsed());
        assert!(o.failed.is_empty(), "{:?}", o.failed);
        // Initialize the bid_orders array
        let mut bid_orders: [settle_order; 10] = [settle_order { index: 0, auth: _addr_zero_i64 }; 10];
        bid_orders[0] = settle_order { index: 1, auth: _auth_i64 };
        // Initialize the ask_orders array
        let mut ask_orders: [settle_order; 10] = [settle_order { index: 0, auth: _addr_zero_i64 }; 10];
        ask_orders[0] = settle_order { index: 1, auth: _auth_i64 };
        let mut bid_market_orders: [market_order; 10] = [market_order { amount: 0, addr: _addr_zero_i64, auth: _addr_zero_i64 }; 10];
        bid_market_orders[0] = market_order { amount: 10, addr: _addr_i64[2], auth: _auth_i64 };
        bid_market_orders[1] = market_order { amount: 10, addr: _addr_i64[3], auth: _auth_i64 };
        let mut ask_market_orders: [market_order; 10] = [market_order { amount: 0, addr: _addr_zero_i64, auth: _addr_zero_i64 }; 10];
        ask_market_orders[0] = market_order { amount: 10, addr: _addr_i64[4], auth: _auth_i64 };
        ask_market_orders[1] = market_order { amount: 10, addr: _addr_i64[5], auth: _auth_i64 };
        let solver_orders = [
            LimitOrder { max_amnt: 0, price: 0, is_bid: true, addr: _addr_i64[6], auth: _auth_i64, next_key: 0 },
            LimitOrder { max_amnt: 0, price: 0, is_bid: false, addr: _addr_i64[6], auth: _auth_i64, next_key: 0 }
         ];
        let mut address_list_bid: [[Word; 4]; 11] = [_addr_zero_i64; 11];
        address_list_bid[0] = _addr_i64[0];
        address_list_bid[10] = _addr_i64[6];
        let mut address_list_ask: [[Word; 4]; 11] = [_addr_zero_i64; 11];
        address_list_ask[0] = _addr_i64[1];
        address_list_ask[10] = _addr_i64[6];
        let mut address_list_bid_market: [[Word; 4]; 10] = [_addr_zero_i64; 10];
        address_list_bid_market[0] = _addr_i64[2];
        address_list_bid_market[1] = _addr_i64[3];
        let mut address_list_ask_market: [[Word; 4]; 10] = [_addr_zero_i64; 10];
        address_list_ask_market[0] = _addr_i64[4];
        address_list_ask_market[1] = _addr_i64[5];
        let mut amount_0_final_bid = [0; 11];
        let mut amount_1_final_bid = [0; 11];
        amount_0_final_bid[0] = 8000;
        amount_1_final_bid[0] = 120;
        let mut amount_0_final_ask = [0; 11];
        amount_0_final_ask[0] = 12000;
        let mut amount_1_final_ask = [0; 11];
        amount_1_final_ask[0] = 80;
        let mut amount_0_final_bid_market = [0; 10];
        amount_0_final_bid_market[0] = 9000;
        amount_0_final_bid_market[1] = 9000;
        let mut amount_1_final_bid_market = [0; 10];
        amount_1_final_bid_market[0] = 110;
        amount_1_final_bid_market[1] = 110;
        let mut amount_0_final_ask_market = [0; 10];
        amount_0_final_ask_market[0] = 11000;
        amount_0_final_ask_market[1] = 11000;
        let mut amount_1_final_ask_market = [0; 10];
        amount_1_final_ask_market[0] = 90;
        amount_1_final_ask_market[1] = 90;
        let first_bid_order = 1;
        let first_ask_order = 1;
        let mut final_bid_order = [LimitOrder { max_amnt: 0, price: 0, is_bid: false, addr: _addr_zero_i64, auth: _addr_zero_i64, next_key: 0 }; 10];
        final_bid_order[0] = LimitOrder { max_amnt: 80, price: 100, is_bid: true, addr: _addr_i64[0], auth: _auth_i64, next_key: 0 };
        let mut final_ask_order = [LimitOrder { max_amnt: 0, price: 0, is_bid: false, addr: _addr_zero_i64, auth: _addr_zero_i64, next_key: 0 }; 10];
        final_ask_order[0] = LimitOrder { max_amnt: 80, price: 100, is_bid: false, addr: _addr_i64[1], auth: _auth_i64, next_key: 0 };
        let solution0 = produce_solution_market_order(
            20, // partial_amount_bid
            20, // partial_amount_ask
            0, // partial_bid_index
            0, // partial_ask_index
            bid_orders, // bid_orders: [settle_order; 10],
            ask_orders, // ask_orders: [settle_order; 10],
            bid_market_orders, // bid_market_orders: [market_order; 10],
            ask_market_orders, // ask_market_orders: [market_order; 10],
            100, // average_price_bid
            100, // average_price_ask
            solver_orders, // solver_orders: [LimitOrder; 2],
            address_list_bid, // address_list: [[Word; 4]; 11], // the last index is the solver address
            address_list_ask, // address_list: [[Word; 4]; 11], // the last index is the solver address
            address_list_bid_market, // address_list_bid_market: [[Word; 4]; 10],
            address_list_ask_market, // address_list_ask_market: [[Word; 4]; 10],
            amount_0_final_bid, // amount_0_final: [i64; 11],
            amount_1_final_bid, // amount_1_final: [i64; 11],
            amount_0_final_ask, // amount_0_final: [i64; 11],
            amount_1_final_ask, // amount_1_final: [i64; 11],
            amount_0_final_bid_market, // amount_0_final_bid_market: [i64; 10],
            amount_1_final_bid_market, // amount_1_final_bid_market: [i64; 10],
            amount_0_final_ask_market, // amount_0_final_ask_market: [i64; 10],
            amount_1_final_ask_market, // amount_1_final_ask_market: [i64; 10],
            first_bid_order, // first_bid_order_market: i64,
            first_ask_order, // first_ask_order_market: i64,
            final_bid_order, // final_bid_order_market: [LimitOrder; 10],
            final_ask_order, // final_ask_order_market: [LimitOrder; 10],            
        );
        let solution_set = SolutionSet {
            solutions: vec![solution0],
        };
        // println!("solution_set: {:?}", solution_set);
    
        utils::builder::submit(&dbs.builder, solution_set.clone())
        .await
        .unwrap();

        // validate the solution
        let result = utils::node::validate_solution(&dbs.node, solution_set.clone())
        .await
        .unwrap();

        println!("result: {:?}", result);

        // Build a block
        let t0 = Instant::now();
        let o = utils::builder::build_default(&dbs).await.unwrap();
        println!("o: {:?}", o);
        println!("⏱️ build_default took: {:?}", t0.elapsed());
        assert!(o.failed.is_empty(), "{:?}", o.failed);
        let bid_order_keys = fetch_bid_order_keys(1);
        println!("bid_order_keys: {:?}", bid_order_keys);
        let bid_order_amount = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &bid_order_keys)
         .await
         .unwrap();
        //  assert_eq!(r, Some(vec![100]));
         println!("bid_order_amount: {:?}", bid_order_amount);

         let balance_0_key_addr_bid = balances_0_key(_addr_i64[0]);
         let balance_1_key_addr_bid = balances_1_key(_addr_i64[0]);
         let balance_0_key_addr_ask = balances_0_key(_addr_i64[1]);
         let balance_1_key_addr_ask = balances_1_key(_addr_i64[1]);
         let balance_0_key_addr_bid_market0 = balances_0_key(_addr_i64[2]);
         let balance_1_key_addr_bid_market0 = balances_1_key(_addr_i64[2]);
         let balance_0_key_addr_bid_market1 = balances_0_key(_addr_i64[3]);
         let balance_1_key_addr_bid_market1 = balances_1_key(_addr_i64[3]);
         let balance_0_key_addr_ask_market0 = balances_0_key(_addr_i64[4]);
         let balance_1_key_addr_ask_market0 = balances_1_key(_addr_i64[4]);
         let balance_0_key_addr_ask_market1 = balances_0_key(_addr_i64[5]);
         let balance_1_key_addr_ask_market1 = balances_1_key(_addr_i64[5]);
         
         let balance_0_addr_bid = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_0_key_addr_bid).await.unwrap();
         let balance_1_addr_bid = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_1_key_addr_bid).await.unwrap();
         let balance_0_addr_ask = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_0_key_addr_ask).await.unwrap();
         let balance_1_addr_ask = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_1_key_addr_ask).await.unwrap();
         let balance_0_addr_bid_market0 = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_0_key_addr_bid_market0).await.unwrap();
         let balance_1_addr_bid_market0 = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_1_key_addr_bid_market0).await.unwrap();
         let balance_0_addr_bid_market1 = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_0_key_addr_bid_market1).await.unwrap();
         let balance_1_addr_bid_market1 = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_1_key_addr_bid_market1).await.unwrap();
         let balance_0_addr_ask_market0 = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_0_key_addr_ask_market0).await.unwrap();
         let balance_1_addr_ask_market0 = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_1_key_addr_ask_market0).await.unwrap();
         let balance_0_addr_ask_market1 = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_0_key_addr_ask_market1).await.unwrap();
         let balance_1_addr_ask_market1 = utils::node::query_state_head(&dbs.node, &predicate_address.contract, &balance_1_key_addr_ask_market1).await.unwrap();
         
         println!("balance_0_addr_bid: {:?}", balance_0_addr_bid);
         println!("balance_1_addr_bid: {:?}", balance_1_addr_bid);
         println!("balance_0_addr_ask: {:?}", balance_0_addr_ask);
         println!("balance_1_addr_ask: {:?}", balance_1_addr_ask);
         println!("balance_0_addr_bid_market0: {:?}", balance_0_addr_bid_market0);
         println!("balance_1_addr_bid_market0: {:?}", balance_1_addr_bid_market0);
         println!("balance_0_addr_bid_market1: {:?}", balance_0_addr_bid_market1);
         println!("balance_1_addr_bid_market1: {:?}", balance_1_addr_bid_market1);
         println!("balance_0_addr_ask_market0: {:?}", balance_0_addr_ask_market0);
         println!("balance_1_addr_ask_market0: {:?}", balance_1_addr_ask_market0);
         println!("balance_0_addr_ask_market1: {:?}", balance_0_addr_ask_market1);
         println!("balance_1_addr_ask_market1: {:?}", balance_1_addr_ask_market1);
         
    }
    #[tokio::test]
    async fn test_experiment_trace() {
        /* 
        In this test, we will run a large experiment trace.
        Step 1: Generate n addresses.
        Step 2: Generate a sequence of prices, p_1, p_2, ..., p_m. Here we assume m = 100. Also, |p_i - p_i+1| = 1. So, p(t) becomes a random walk.
        And the price sequence is generated by a function price_seq(t).
        Step 3: Deposit k = 1000,000 tokens to all addresses. 
        Step 4: For each time step t, first, settlement of existing limit orders happens based on current price p(t). Then, market orders are settled.
        Then, limit orders are added such that the bid orders go from p(t) to p(t) - 1, and the ask orders go from p(t) to p(t) + 1.
        Step 5: Repeat Step 4 for m times.
        */
        let n = 1000; // number of addresses
        let k = 1000000; // initial amount of tokens in each address
        let m = 20; // number of price steps
        let mut balance_0: HashMap<[i64; 4], i64> = HashMap::new();
        let mut balance_1: HashMap<[i64; 4], i64> = HashMap::new();
        // Step 1: generate n addresses

        // Use a fixed seed for reproducibility
        let seed: u64 = 42;
        let mut rng = StdRng::seed_from_u64(seed);

        for _ in 0..10 {
            println!("{:?}", hex_to_i64_array(generate_random_hash(&mut rng).as_str()));
        }

        let mut addresses = vec![];
        for i in 0..n {
            addresses.push(hex_to_i64_array(generate_random_hash(&mut rng).as_str()));
        }
        let _addr_zero_i64 = hex_to_i64_array("0x0000000000000000000000000000000000000000000000000000000000000000");
        // Step 2: generate a random walk price sequence
        let mut price_seq = vec![0i64; m];
        price_seq[0] = 100;
    
        for i in 1..m {
            let step: i64 = if rng.gen_bool(0.6) { 5 } else { -5 };
            price_seq[i] = (price_seq[i - 1] + step).max(0);
        }
    
        // Optional: print the first few values
        for price in price_seq.iter().take(100) {
            println!("{}", price);
        }

        // Step 3: deposit k = 1000,000 tokens to all addresses
        
        // Load the contract bytecode
        tracing_subscriber::fmt::init(); // need to initialize the logger only once
        let contract_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../PintLOB/orderbook").into();
        let (orderbook, programs): (Contract, Vec<Program>) =
            compile_pint_project(contract_path).await.unwrap();
    
        let contract_address = essential_hash::contract_addr::from_contract(&orderbook);
        let predicate_address = essential_hash::content_addr(&orderbook.predicates[2]);
        println!("predicate_address: {:?}", predicate_address);
        let predicate_address = PredicateAddress {
            contract: contract_address,
            predicate: predicate_address,
        };
        
        // Initialize the database
        
        let dbs = utils::db::new_dbs().await;

        // Load the node types
        let big_bang = BigBang::default();
        // Deploy the contract

        let contract_registry = big_bang.contract_registry;
        let program_registry = big_bang.program_registry;
        essential_app_utils::deploy::register_contract_and_programs(
            &dbs.builder,
            &contract_registry,
            &program_registry,
            &orderbook,
            programs,
        )
        .await
        .unwrap();
       
        for addr in &addresses {
            let solution = produce_solution_deposit(
                1000000,         //amount_0_delta: i64,
                1000000,         //amount_0_final: i64,
                1000000,         //amount_1_delta: i64,
                1000000,         //amount_1_final: i64,
                addr.clone(),   //addr_word: [Word; 4],
                addr.clone(),   //key_word: [Word; 4],
                addr.clone()    //auth_word: [Word; 4]
                );
                let solution_set = SolutionSet {
                    solutions: vec![solution],
                };
            
                utils::builder::submit(&dbs.builder, solution_set.clone())
                .await
                .unwrap();
         
                // validate the solution
                let result = utils::node::validate_solution(&dbs.node, solution_set.clone())
                .await
                .unwrap();
                println!("result: {:?}", result);
         
                // Build a block
                let t0 = Instant::now();
                let o = utils::builder::build_default(&dbs).await.unwrap();
                println!("o: {:?}", o);
                println!("⏱️ build_default took: {:?}", t0.elapsed());
                assert!(o.failed.is_empty(), "{:?}", o.failed);
                balance_0.insert(addr.clone(), 1_000_000);
                balance_1.insert(addr.clone(), 1_000_000);
        }

        // Step 4.1: For each time step t, first, settlement of existing limit orders happens based on current price p(t).
        // Then, limit orders are added such that the bid orders go from p(t) to p(t) - 1, and the ask orders go from p(t) to p(t) + 1.

        // bids is a BTreeMap of price -> PriceLevel, where PriceLevel is a VecDeque of Order
        // asks is a BTreeMap of price -> PriceLevel, where PriceLevel is a VecDeque of Order
        // PriceLevel is always sorted by time-priority, i.e. the oldest order is at the front
        let mut orderbook = OrderBook {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        };
        for t in 0..m {
            let current_price = price_seq[t] as u64;
            println!("price: {:?}", current_price);
            // settlement of existing limit orders:

            // collect all bid orders with price greater than or equal to p(t)
            let mut bid_orders_list = VecDeque::new(); // elements from index 0 to n-1 onwards are sorted by price-time priority
            let to_remove: Vec<u64> = orderbook
                .bids
                .range(current_price..)
                .map(|(&price, _)| price)
                .collect();

            for price in to_remove {
                if let Some(orders) = orderbook.bids.remove(&price) {
                    for order in orders.into_iter().rev() { // reverse to maintain push_front logic
                        bid_orders_list.push_front(order);
                    }
                }
            }
        

            // collect all ask orders with price less than or equal to p(t)
            let mut ask_orders_list = VecDeque::new(); // elements from index 0 to n-1 onwards are sorted by price-time priority
            let to_remove: Vec<u64> = orderbook
                .asks
                .range(..=current_price)
                .map(|(&price, _)| price)
                .collect();

            for price in to_remove {
                if let Some(orders) = orderbook.asks.remove(&price) {
                    for order in orders {
                        ask_orders_list.push_back(order);
                    }
                }
            }

            // settle the bid and askorders
            while bid_orders_list.len() > 0 || ask_orders_list.len() > 0 {
                // Initialize the address_list_bid and address_list_ask arrays
                let mut address_list_bid: [[Word; 4]; 11] = [_addr_zero_i64; 11];
                let mut address_list_ask: [[Word; 4]; 11] = [_addr_zero_i64; 11];
                let mut amount_0_final_bid = [0; 11];
                let mut amount_1_final_bid = [0; 11];
                let mut amount_0_final_ask = [0; 11];
                let mut amount_1_final_ask = [0; 11];
                let mut total_bid_amount = 0;
                let mut total_bid_token0 = 0;
                let mut total_ask_amount = 0;
                let mut total_ask_token0 = 0;
                let mut partial_bid_index: i64 = 0;
                let mut partial_ask_index: i64 = 0;
                let mut partial_bid_amount: i64 = 0;
                let mut partial_ask_amount: i64 = 0;
                // Initialize the bid_orders array
                let mut bid_orders: [settle_order; 10] = [settle_order { index: 0, auth: _addr_zero_i64 }; 10];
                // Safely goes through first 10 bid orders
                for i in 0..10 {
                    if let Some(order) = bid_orders_list.pop_front() {
                        bid_orders[i] = settle_order {index: order.index, auth: order.auth}; //ToDo: why is auth needed? Since the order is already part of the linked list, it should be settled.
                        address_list_bid[i] = order.addr;
                        amount_0_final_bid[i] = balance_0[&order.addr] - order.max_amnt * order.price;
                        amount_1_final_bid[i] = balance_1[&order.addr] + order.max_amnt;
                        // update the balance
                        balance_0.insert(order.addr, amount_0_final_bid[i]);
                        balance_1.insert(order.addr, amount_1_final_bid[i]);
                        total_bid_amount += order.max_amnt;
                        total_bid_token0 += order.max_amnt * order.price;
                        partial_bid_index = i as i64; // it keeps track of the highest index of the bid order that has been settled
                        partial_bid_amount = order.max_amnt; // it keeps track of the amount of the highest index bid order that has been settled
                        println!("BID -> ID: {}, Price: {}, Qty: {}", order.index, order.price, order.max_amnt);
                    } else {
                        break; // fewer than 10 orders
                    }
                }
                // Initialize the ask_orders array
                let mut ask_orders: [settle_order; 10] = [settle_order { index: 0, auth: _addr_zero_i64 }; 10];
                for i in 0..10 {
                    if let Some(order) = ask_orders_list.pop_front() {
                        ask_orders[i] = settle_order {index: order.index, auth: order.auth};
                        address_list_ask[i] = order.addr;
                        amount_0_final_ask[i] = balance_0[&order.addr] + order.max_amnt*order.price;
                        amount_1_final_ask[i] = balance_1[&order.addr] - order.max_amnt;
                        // update the balance
                        balance_0.insert(order.addr, amount_0_final_ask[i]);
                        balance_1.insert(order.addr, amount_1_final_ask[i]);
                        total_ask_amount += order.max_amnt;
                        total_ask_token0 += order.max_amnt * order.price;
                        partial_ask_index = i as i64; // it keeps track of the highest index of the ask order that has been settled
                        partial_ask_amount = order.max_amnt; // it keeps track of the amount of the highest index ask order that has been settled
                        println!("ASK -> ID: {}, Price: {}, Qty: {}", order.index, order.price, order.max_amnt);
                    } else {
                        break; // fewer than 10 orders
                    }
                }
                // calculate the amount of tokens from solver orders
                let solver_addr = addresses.iter().next_back().unwrap().clone();
                let mut solver_orders = [
                    LimitOrder { max_amnt: 0, price: 0, is_bid: true, addr: solver_addr, auth: solver_addr.clone(), next_key: 0 },
                    LimitOrder { max_amnt: 0, price: 0, is_bid: false, addr: solver_addr, auth: solver_addr.clone(), next_key: 0 }
                ];

                // solver bids all the asks
                solver_orders[0].max_amnt = total_ask_amount;
                solver_orders[0].price = if total_ask_amount != 0 {total_ask_token0/total_ask_amount+1} else {0}; // ceil because the solver buys at a slightly higher price
                println!("solver total_ask_amount: {:?}", total_ask_amount);
                println!("solver total_ask_token0: {:?}", total_ask_token0);
                // solver asks all the bids
                solver_orders[1].max_amnt = total_bid_amount;
                solver_orders[1].price = if total_bid_amount != 0 {total_bid_token0/total_bid_amount} else {0};
                // update the solverbalance
                amount_0_final_bid[10] = balance_0[&solver_addr] - solver_orders[0].max_amnt * solver_orders[0].price + solver_orders[1].max_amnt * solver_orders[1].price;
                amount_0_final_ask[10] = amount_0_final_bid[10];
                amount_1_final_bid[10] = balance_1[&solver_addr] + solver_orders[0].max_amnt - solver_orders[1].max_amnt;
                amount_1_final_ask[10] = amount_1_final_bid[10];
                balance_0.insert(solver_addr, amount_0_final_bid[10]);
                balance_1.insert(solver_addr, amount_1_final_bid[10]);
                // update the address_list_bid and address_list_ask
                address_list_bid[10] = solver_addr;
                address_list_ask[10] = solver_addr;
                // update the first_bid_order and first_ask_order
                let first_bid_order = if let Some(order) = bid_orders_list.front() {
                    order.index
                } else if let Some((_price, orders)) = orderbook.bids.iter().next_back() {
                    orders.get(0).map_or(0, |order| order.index)
                } else {
                    0
                };
                let first_ask_order = if let Some(order) = ask_orders_list.front() {
                    order.index
                } else if let Some((_price, orders)) = orderbook.asks.iter().next() {
                    orders.get(0).map_or(0, |order| order.index)
                } else {
                    0
                };
                let final_bid_order = [LimitOrder { max_amnt: 0, price: 0, is_bid: false, addr: _addr_zero_i64, auth: _addr_zero_i64, next_key: 0 }; 10];
                let final_ask_order = [LimitOrder { max_amnt: 0, price: 0, is_bid: false, addr: _addr_zero_i64, auth: _addr_zero_i64, next_key: 0 }; 10];
                let solution0 = produce_solution_settle(
                    partial_bid_amount, // partial_amount_bid
                    partial_ask_amount, // partial_amount_ask
                    partial_bid_index, // partial_bid_index
                    partial_ask_index, // partial_ask_index
                    bid_orders, // bid_orders: [settle_order; 10],
                    ask_orders, // ask_orders: [settle_order; 10],
                    solver_orders, // solver_orders: [LimitOrder; 2],
                    address_list_bid, // address_list: [[Word; 4]; 11], // the last index is the solver address
                    address_list_ask, // address_list: [[Word; 4]; 11], // the last index is the solver address
                    amount_0_final_bid, // amount_0_final: [i64; 11],
                    amount_1_final_bid, // amount_1_final: [i64; 11],
                    amount_0_final_ask, // amount_0_final: [i64; 11],
                    amount_1_final_ask, // amount_1_final: [i64; 11],
                    first_bid_order, // first_bid_order: i64,
                    first_ask_order, // first_ask_order: i64,
                    final_bid_order, // final_bid_order: [LimitOrder; 10],
                    final_ask_order, // final_ask_order: [LimitOrder; 10],
                );
                println!("partial_bid_amount: {:?}", partial_bid_amount);
                println!("partial_ask_amount: {:?}", partial_ask_amount);
                println!("partial_bid_index: {:?}", partial_bid_index);
                println!("partial_ask_index: {:?}", partial_ask_index);
                println!("bid_orders: {:?}", bid_orders);
                println!("ask_orders: {:?}", ask_orders);
                println!("solver_orders: {:?}", solver_orders);
                println!("address_list_bid: {:?}", address_list_bid);
                println!("address_list_ask: {:?}", address_list_ask);
                println!("amount_0_final_bid: {:?}", amount_0_final_bid);
                println!("amount_1_final_bid: {:?}", amount_1_final_bid);
                println!("amount_0_final_ask: {:?}", amount_0_final_ask);
                println!("amount_1_final_ask: {:?}", amount_1_final_ask);
                println!("first_bid_order: {:?}", first_bid_order);
                println!("first_ask_order: {:?}", first_ask_order);
                let solution_set = SolutionSet {
                    solutions: vec![solution0],
                };
                // println!("solution_set: {:?}", solution_set);
            
                utils::builder::submit(&dbs.builder, solution_set.clone())
                .await
                .unwrap();

                // validate the solution
                let result = utils::node::validate_solution(&dbs.node, solution_set.clone())
                .await
                .unwrap();

                println!("result: {:?}", result);

                // Build a block
                let t0 = Instant::now();
                let o = utils::builder::build_default(&dbs).await.unwrap();
                println!("o: {:?}", o);
                println!("⏱️ build_default took: {:?}", t0.elapsed());
                assert!(o.failed.is_empty(), "{:?}", o.failed);
            
            }

            // add new limit orders
            // fill all the asks from p(t) onwards and above
            // fill all the bids from p(t) onwards and below

            // bids
            // Fill bid gaps: current_price ↓ to lowest existing bid
            let highest_bid_price = if let Some((&_price, _)) = orderbook.bids.iter().next_back() {
                _price + 1
            } else {
                current_price - 1
            };
            if current_price >= highest_bid_price {
                for price in (highest_bid_price..=current_price) {
                    println!("price: {:?}", price);
                    if !orderbook.bids.contains_key(&price) {
                        let mut tentative_orders = VecDeque::new();
                        for i in 0..3 {
                            let _index = generate_index(&mut rng);
                            let _addr = addresses.pop().unwrap();
                            tentative_orders.push_back(Order {
                                index: _index,
                                max_amnt: 100,
                                price: price as i64,
                                is_bid: true,
                                addr: _addr,
                                auth: _addr_zero_i64,
                            });
                            let leading_key = if i == 0 { // first order in the bid orderbook
                                0
                            } else { // second order in the tentative orders
                                tentative_orders.get(i-1).unwrap().index
                            };
                            let trailing_key = orderbook
                            .bids
                            .iter()
                            .next_back()
                            .and_then(|(_, orders)| orders.get(0))
                            .map(|order| order.index)
                            .unwrap_or(0);
                            let leading_order_next = if i == 0 { // first order in the bid orderbook
                                0
                            } else {
                                _index
                            };
                            let first_order_index = tentative_orders.get(0).unwrap().index;

                            println!("tentative_orders: {:?}", tentative_orders);
                            println!("index: {:?}", _index);
                            println!("leading_key: {:?}", leading_key);
                            println!("trailing_key: {:?}", trailing_key);
                            println!("leading_order_next: {:?}", leading_order_next);
                            println!("first_order_index: {:?}", first_order_index);
                            println!("price: {:?}", price);
                            let solutionAddBid = produce_solution_add_limit_order_bid(
                                leading_key, // leading_key
                                trailing_key, // trailing_key
                                LimitOrder {
                                    max_amnt: 100,
                                    price: price as i64,
                                    is_bid: true,
                                    addr: _addr,
                                    auth: _addr_zero_i64,
                                    next_key: trailing_key,
                                },
                                _index, // new_index
                                leading_order_next, // leading_order_next
                                first_order_index, // first_order_index
                            );
                    
                            let solution_set_add = SolutionSet {
                                // solutions: vec![solutionAddBid, solutionAddAsk],
                                solutions: vec![solutionAddBid.clone()],
                            };
                            // println!("solution_set: {:?}", solution_set);
                        
                            utils::builder::submit(&dbs.builder, solution_set_add.clone())
                            .await
                            .unwrap();
                    
                            // validate the solution
                            let result = utils::node::validate_solution(&dbs.node, solution_set_add.clone())
                            .await
                            .unwrap();
                    
                            println!("result: {:?}", result);
                    
                            // Build a block
                            let t0 = Instant::now();
                            let o = utils::builder::build_default(&dbs).await.unwrap();
                            println!("o: {:?}", o);
                            println!("⏱️ build_default add limit bid order took: {:?}", t0.elapsed());
                            assert!(o.failed.is_empty(), "{:?}", o.failed);

                        }
                        orderbook.bids.insert(price, tentative_orders);
                    }
                }
            }

            // Fill ask gaps: current_price ↑ to highest existing ask
            let lowest_ask_price = if let Some((&_price, _)) = orderbook.asks.iter().next() {
                _price - 1
            } else {
                current_price + 1
            };
            if current_price <= lowest_ask_price {
                for price in (current_price..=lowest_ask_price).rev() {
                    if !orderbook.asks.contains_key(&price) {
                        let mut tentative_orders = VecDeque::new();
                        for i in 0..3 {
                            let _index = generate_index(&mut rng);
                            let _addr = addresses.pop().unwrap();
                            tentative_orders.push_back(Order {
                                index: _index,
                                max_amnt: 100,
                                price: price as i64,
                                is_bid: false,
                                addr: _addr,
                                auth: _addr_zero_i64,
                            });
                            let leading_key = if i == 0 { // first order in the ask orderbook
                                0
                            } else { // second order in the tentative orders
                                tentative_orders.get(i-1).unwrap().index
                            };
                            let trailing_key = orderbook
                            .asks
                            .iter()
                            .next()
                            .and_then(|(_, orders)| orders.get(0))
                            .map(|order| order.index)
                            .unwrap_or(0);
                            let leading_order_next = if i == 0 { // first order in the ask orderbook
                                0
                            } else {
                                _index
                            };
                            let first_order_index = tentative_orders.get(0).unwrap().index;
                            println!("leading_key: {:?}", leading_key);
                            println!("trailing_key: {:?}", trailing_key);
                            println!("index: {:?}", _index);
                            println!("leading_order_next: {:?}", leading_order_next);
                            println!("first_order_index: {:?}", first_order_index);
                            println!("price: {:?}", price);
                            let solutionAddAsk = produce_solution_add_limit_order_ask(
                                leading_key, // leading_key
                                trailing_key, // trailing_key
                                LimitOrder {
                                    max_amnt: 100,
                                    price: price as i64,
                                    is_bid: false,
                                    addr: _addr,
                                    auth: _addr_zero_i64,
                                    next_key: trailing_key,
                                },
                                _index, // new_index
                                leading_order_next, // leading_order_next
                                first_order_index, // first_order_index
                            );
                    
                            let solution_set_add = SolutionSet {
                                // solutions: vec![solutionAddBid, solutionAddAsk],
                                solutions: vec![solutionAddAsk.clone()],
                            };
                            // println!("solution_set: {:?}", solution_set);
                        
                            utils::builder::submit(&dbs.builder, solution_set_add.clone())
                            .await
                            .unwrap();
                            
                            // validate the solution
                            let result = utils::node::validate_solution(&dbs.node, solution_set_add.clone())
                            .await
                            .unwrap();
                            
                            println!("result: {:?}", result);
                            
                            // Build a block
                            let t0 = Instant::now();
                            let o = utils::builder::build_default(&dbs).await.unwrap();
                            println!("o: {:?}", o);
                            println!("⏱️ build_default add limit ask order took: {:?}", t0.elapsed());
                            assert!(o.failed.is_empty(), "{:?}", o.failed);

                        }
                        orderbook.asks.insert(price, tentative_orders);
                    }
                }
            }



        }

        
        
    }
}
