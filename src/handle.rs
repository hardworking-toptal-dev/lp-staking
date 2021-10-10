use cosmwasm_bignumber::{Decimal256, Uint256};
#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    attr, to_binary, Addr, CanonicalAddr, CosmosMsg, DepsMut, Env, MessageInfo, Response, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use terraswap::querier::query_token_balance;

use crate::state::{Config, StakerInfo, State, CONFIG, STAKER_INFO, STATE};
use crate::ContractError;

pub fn bond(
    deps: DepsMut,
    env: Env,
    sender_addr: Addr,
    amount: Uint256,
) -> Result<Response, ContractError> {
    let sender_addr_raw: CanonicalAddr = deps.api.addr_canonicalize(sender_addr.as_str())?;

    let config: Config = CONFIG.load(deps.storage)?;
    let mut state: State = STATE.load(deps.storage)?;
    let mut staker_info: StakerInfo =
        match STAKER_INFO.may_load(deps.storage, sender_addr_raw.as_slice())? {
            Some(staker_info) => staker_info,
            None => StakerInfo::default(),
        };

    // Compute global reward & staker reward
    compute_reward(&config, &mut state, env.block.time.seconds());
    compute_staker_reward(&state, &mut staker_info);

    // Increase bond_amount
    increase_bond_amount(&mut state, &mut staker_info, amount);

    // Store updated state with staker's staker_info
    STAKER_INFO.save(deps.storage, sender_addr_raw.as_slice(), &staker_info)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "bond"),
        attr("owner", sender_addr),
        attr("amount", amount.to_string()),
    ]))
}

pub fn bond_hook(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let sender_addr_raw: CanonicalAddr = deps.api.addr_canonicalize(info.sender.as_str())?;

    let config: Config = CONFIG.load(deps.storage)?;
    let mut state: State = STATE.load(deps.storage)?;

    // Compare lp token balance to bond amount to obtain newly received LP tokens
    let lp_token: Addr = deps.api.addr_humanize(&config.miaw_lp_token)?;
    let lp_token_balance: Uint256 =
        query_token_balance(&deps.querier, lp_token, env.contract.address)?.into();

    let amount: Uint256 = lp_token_balance - state.total_bond_amount;
    if amount.is_zero() {
        return Err(ContractError::NothingToStake {});
    }

    let mut staker_info: StakerInfo = STAKER_INFO.load(deps.storage, sender_addr_raw.as_slice())?;

    // Compute global reward & staker reward
    compute_reward(&config, &mut state, env.block.time.seconds());
    compute_staker_reward(&state, &mut staker_info);

    // Increase bond_amount
    increase_bond_amount(&mut state, &mut staker_info, amount);

    // Store updated state with staker's staker_info
    STAKER_INFO.save(deps.storage, sender_addr_raw.as_slice(), &staker_info)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "bond_hook"),
        attr("owner", info.sender),
        attr("amount", amount.to_string()),
    ]))
}

pub fn unbond(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Option<Uint256>,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;
    let sender_addr_raw: CanonicalAddr = deps.api.addr_canonicalize(info.sender.as_str())?;

    let mut state: State = STATE.load(deps.storage)?;
    let mut staker_info: StakerInfo = STAKER_INFO.load(deps.storage, sender_addr_raw.as_slice())?;

    let amount_to_unbond: Uint256 = if let Some(amount) = amount {
        if staker_info.bond_amount < amount {
            return Err(ContractError::InvalidUnbondAmount {});
        } else {
            amount
        }
    } else {
        staker_info.bond_amount
    };

    // Compute global reward & staker reward
    compute_reward(&config, &mut state, env.block.time.seconds());
    compute_staker_reward(&state, &mut staker_info);

    // Decrease bond_amount
    decrease_bond_amount(&mut state, &mut staker_info, amount_to_unbond);

    // Store or remove updated rewards info
    // depends on the left pending reward and bond amount
    if staker_info.pending_reward.is_zero() && staker_info.bond_amount.is_zero() {
        STAKER_INFO.remove(deps.storage, sender_addr_raw.as_slice());
    } else {
        STAKER_INFO.save(deps.storage, sender_addr_raw.as_slice(), &staker_info)?;
    }

    // Store updated state
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.addr_humanize(&config.miaw_lp_token)?.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: info.sender.to_string(),
                amount: amount_to_unbond.into(),
            })?,
            funds: vec![],
        })])
        .add_attributes(vec![
            attr("action", "unbond"),
            attr("owner", info.sender),
            attr("amount", amount_to_unbond.to_string()),
        ]))
}

pub fn claim_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let sender_addr_raw: CanonicalAddr = deps.api.addr_canonicalize(info.sender.as_str())?;

    let config: Config = CONFIG.load(deps.storage)?;
    let mut state: State = STATE.load(deps.storage)?;
    let mut staker_info: StakerInfo = STAKER_INFO.load(deps.storage, sender_addr_raw.as_slice())?;

    // Compute global reward & staker reward
    compute_reward(&config, &mut state, env.block.time.seconds());
    compute_staker_reward(&state, &mut staker_info);

    let amount: Uint256 = staker_info.pending_reward;
    staker_info.pending_reward = Uint256::zero();

    // Store or remove updated rewards info
    // depends on the left pending reward and bond amount
    if staker_info.bond_amount.is_zero() {
        STAKER_INFO.remove(deps.storage, sender_addr_raw.as_slice());
    } else {
        STAKER_INFO.save(deps.storage, sender_addr_raw.as_slice(), &staker_info)?;
    }

    // Store updated state
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.addr_humanize(&config.miaw_token)?.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: info.sender.to_string(),
                amount: amount.into(),
            })?,
            funds: vec![],
        })])
        .add_attributes(vec![
            attr("action", "withdraw"),
            attr("owner", info.sender),
            attr("amount", amount.to_string()),
        ]))
}

fn increase_bond_amount(state: &mut State, staker_info: &mut StakerInfo, amount: Uint256) {
    state.total_bond_amount += amount;
    staker_info.bond_amount += amount;
}

fn decrease_bond_amount(state: &mut State, staker_info: &mut StakerInfo, amount: Uint256) {
    state.total_bond_amount = state.total_bond_amount - amount;
    staker_info.bond_amount = staker_info.bond_amount - amount;
}

// compute distributed rewards and update global reward index
pub fn compute_reward(config: &Config, state: &mut State, current_time: u64) {
    if state.total_bond_amount.is_zero() {
        state.last_distributed = current_time;
        return;
    }

    let mut distributed_amount: Uint256 = Uint256::zero();
    for s in config.distribution_schedule.iter() {
        if s.0 > current_time || s.1 < state.last_distributed {
            continue;
        }

        // min(s.1, current_time) - max(s.0, last_distributed)
        let seconds_passed =
            std::cmp::min(s.1, current_time) - std::cmp::max(s.0, state.last_distributed);

        let num_seconds = s.1 - s.0;
        let distribution_amount_per_second: Decimal256 = Decimal256::from_ratio(s.2, num_seconds);
        distributed_amount += distribution_amount_per_second * Uint256::from(seconds_passed);
    }

    state.last_distributed = current_time;
    state.global_reward_index +=
        Decimal256::from_ratio(distributed_amount, state.total_bond_amount);
}

// withdraw reward to pending reward
pub fn compute_staker_reward(state: &State, staker_info: &mut StakerInfo) {
    let pending_reward: Uint256 = (staker_info.bond_amount * state.global_reward_index)
        - (staker_info.bond_amount * staker_info.reward_index);

    staker_info.reward_index = state.global_reward_index;
    staker_info.pending_reward += pending_reward;
}
