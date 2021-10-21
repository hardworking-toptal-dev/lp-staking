use cosmwasm_std::{Addr, CanonicalAddr, Deps, Env};

use crate::error::ContractError;
use crate::handle::{compute_reward, compute_staker_reward};
use crate::msg::{ConfigResponse, OrderBy, StakerInfoResponse, StakersInfoResponse, StateResponse};
use crate::state::{
    read_stakers_with_updated_reward, Config, StakerInfo, State, CONFIG, STAKER_INFO, STATE,
};

pub fn query_config(deps: Deps) -> Result<ConfigResponse, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;

    config.as_res(deps.api)
}

pub fn query_state(deps: Deps) -> Result<StateResponse, ContractError> {
    let state: State = STATE.load(deps.storage)?;

    Ok(state.as_res())
}

pub fn query_staker_info(
    deps: Deps,
    env: Env,
    staker: String,
) -> Result<StakerInfoResponse, ContractError> {
    let staker_addr: Addr = deps.api.addr_validate(&staker)?;
    let staker_raw: CanonicalAddr = deps.api.addr_canonicalize(staker_addr.as_str())?;

    let config: Config = CONFIG.load(deps.storage)?;
    let mut state: State = STATE.load(deps.storage)?;
    let mut staker_info: StakerInfo = STAKER_INFO.load(deps.storage, staker_raw.as_slice())?;

    compute_reward(&config, &mut state, env.block.time.seconds());
    compute_staker_reward(&state, &mut staker_info);

    Ok(staker_info.as_res(&staker_addr))
}

pub fn query_stakers_info(
    deps: Deps,
    env: Env,
    start_after: Option<String>,
    limit: Option<u32>,
    order_by: Option<OrderBy>,
) -> Result<StakersInfoResponse, ContractError> {
    let start_after: Option<CanonicalAddr> =
        start_after.map(|addr| deps.api.addr_canonicalize(addr.as_str()).unwrap());

    // update global reward
    let config: Config = CONFIG.load(deps.storage)?;
    let mut state: State = STATE.load(deps.storage)?;
    compute_reward(&config, &mut state, env.block.time.seconds());

    let res: StakersInfoResponse = read_stakers_with_updated_reward(
        deps.storage,
        deps.api,
        &state,
        start_after,
        limit,
        order_by,
    )?;
    Ok(res)
}
