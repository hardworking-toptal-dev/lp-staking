use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{Addr, Api, CanonicalAddr, Order, StdResult, Storage};
use cw_storage_plus::{Bound, Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    handle::compute_staker_reward,
    msg::{ConfigResponse, OrderBy, StakerInfoResponse, StakersInfoResponse, StateResponse},
    ContractError,
};

pub const CONFIG: Item<Config> = Item::new("\u{0}\u{6}config");
pub const STATE: Item<State> = Item::new("\u{0}\u{5}state");
pub const STAKER_INFO: Map<&[u8], StakerInfo> = Map::new("staker_info");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub miaw_token: CanonicalAddr,
    pub miaw_lp_token: CanonicalAddr,
    pub distribution_schedule: Vec<(u64, u64, Uint256)>,
}

impl Config {
    pub fn as_res(&self, api: &dyn Api) -> Result<ConfigResponse, ContractError> {
        let res = ConfigResponse {
            miaw_token: api.addr_humanize(&self.miaw_token)?.to_string(),
            miaw_lp_token: api.addr_humanize(&self.miaw_lp_token)?.to_string(),
            distribution_schedule: self.distribution_schedule.clone(),
        };
        Ok(res)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct State {
    pub last_distributed: u64,
    pub total_bond_amount: Uint256,
    pub global_reward_index: Decimal256,
}

impl State {
    pub fn as_res(&self) -> StateResponse {
        StateResponse {
            last_distributed: self.last_distributed,
            total_bond_amount: self.total_bond_amount,
            global_reward_index: self.global_reward_index,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct StakerInfo {
    pub reward_index: Decimal256,
    pub bond_amount: Uint256,
    pub pending_reward: Uint256,
}

impl StakerInfo {
    pub fn as_res(&self, staker: &Addr) -> StakerInfoResponse {
        StakerInfoResponse {
            staker: staker.to_string(),
            reward_index: self.reward_index,
            bond_amount: self.bond_amount,
            pending_reward: self.pending_reward,
        }
    }
}

const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

pub fn read_stakers_with_updated_reward(
    storage: &dyn Storage,
    api: &dyn Api,
    state: &State,
    start_after: Option<CanonicalAddr>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> StdResult<StakersInfoResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let (start, end, order_by) = match order_by {
        Some(OrderBy::Asc) => (
            calc_range_start_addr(start_after).map(Bound::exclusive),
            None,
            Order::Ascending,
        ),
        _ => (
            None,
            calc_range_end_addr(start_after).map(Bound::exclusive),
            Order::Descending,
        ),
    };

    let stakers: Vec<StakerInfoResponse> = STAKER_INFO
        .range(storage, start, end, order_by)
        .take(limit)
        .map(|item| {
            let (k, mut staker) = item?;
            let addr = api.addr_humanize(&CanonicalAddr::from(k))?;
            compute_staker_reward(state, &mut staker);

            Ok(staker.as_res(&addr))
        })
        .collect::<StdResult<Vec<StakerInfoResponse>>>()?;

    Ok(StakersInfoResponse { stakers })
}

fn calc_range_start_addr(start_after: Option<CanonicalAddr>) -> Option<Vec<u8>> {
    start_after.map(|addr| {
        let mut v = addr.as_slice().to_vec();
        v.push(1);
        v
    })
}

fn calc_range_end_addr(start_after: Option<CanonicalAddr>) -> Option<Vec<u8>> {
    start_after.map(|addr| addr.as_slice().to_vec())
}
