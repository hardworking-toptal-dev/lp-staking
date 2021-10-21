#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response,
};
use cw20::Cw20ReceiveMsg;

use crate::error::ContractError;
use crate::handle::{bond, bond_hook, claim_rewards, unbond};
use crate::msg::{Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::query::{query_config, query_staker_info, query_stakers_info, query_state};
use crate::state::{Config, State, CONFIG, STATE};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let config = Config {
        miaw_token: deps.api.addr_canonicalize(&msg.miaw_token)?,
        miaw_lp_token: deps.api.addr_canonicalize(&msg.miaw_lp_token)?,
        distribution_schedule: msg.distribution_schedule,
    };
    CONFIG.save(deps.storage, &config)?;

    STATE.save(
        deps.storage,
        &State {
            last_distributed: env.block.time.seconds(),
            ..State::default()
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::Unbond { amount } => unbond(deps, env, info, amount),
        ExecuteMsg::ClaimRewards {} => claim_rewards(deps, env, info),
        ExecuteMsg::BondHook {} => bond_hook(deps, env, info),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;
    if config.miaw_lp_token != deps.api.addr_canonicalize(info.sender.as_str())? {
        return Err(ContractError::Unauthorized {});
    }
    let cw20_sender: Addr = deps.api.addr_validate(&cw20_msg.sender)?;

    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Bond {}) => bond(deps, env, cw20_sender, cw20_msg.amount.into()),
        Err(_) => Err(ContractError::InvalidCw20Msg {}),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Config {} => Ok(to_binary(&query_config(deps)?)?),
        QueryMsg::State {} => Ok(to_binary(&query_state(deps)?)?),
        QueryMsg::StakerInfo { staker } => Ok(to_binary(&query_staker_info(deps, env, staker)?)?),
        QueryMsg::StakersInfo {
            start_after,
            limit,
            order_by,
        } => Ok(to_binary(&query_stakers_info(
            deps,
            env,
            start_after,
            limit,
            order_by,
        )?)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::default())
}
