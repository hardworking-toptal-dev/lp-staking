use crate::contract::{execute, instantiate, query};
use crate::testing::mock_querier::mock_dependencies;
use crate::ContractError;
use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{attr, from_binary, to_binary, CosmosMsg, SubMsg, Uint128, WasmMsg};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use crate::msg::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, OrderBy, QueryMsg, StakerInfoResponse,
    StakersInfoResponse, StateResponse,
};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);
    let default_genesis_seconds: u64 = mock_env().block.time.seconds();

    let msg = InstantiateMsg {
        miaw_token: "miaw0000".to_string(),
        miaw_lp_token: "miawlp0000".to_string(),
        distribution_schedule: vec![(100, 200, Uint256::from(1000000u128))],
    };

    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            miaw_token: "miaw0000".to_string(),
            miaw_lp_token: "miawlp0000".to_string(),
            distribution_schedule: vec![(100, 200, Uint256::from(1000000u128))],
        }
    );

    let res = query(deps.as_ref(), mock_env(), QueryMsg::State {}).unwrap();
    let state: StateResponse = from_binary(&res).unwrap();
    assert_eq!(
        state,
        StateResponse {
            last_distributed: default_genesis_seconds,
            total_bond_amount: Uint256::zero(),
            global_reward_index: Decimal256::zero(),
        }
    );
}

#[test]
fn test_bond_tokens() {
    let mut deps = mock_dependencies(&[]);
    let default_genesis_seconds: u64 = mock_env().block.time.seconds();

    let msg = InstantiateMsg {
        miaw_token: "miaw0000".to_string(),
        miaw_lp_token: "miawlp0000".to_string(),
        distribution_schedule: vec![
            (
                default_genesis_seconds,
                default_genesis_seconds + 100,
                Uint256::from(1000000u128),
            ),
            (
                default_genesis_seconds + 100,
                default_genesis_seconds + 200,
                Uint256::from(10000000u128),
            ),
        ],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    let info = mock_info("miawlp0000", &[]);
    let mut env = mock_env();
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_index: Decimal256::zero(),
            pending_reward: Uint256::zero(),
            bond_amount: Uint256::from(100u128),
        }
    );

    assert_eq!(
        from_binary::<StateResponse>(
            &query(deps.as_ref(), mock_env(), QueryMsg::State {}).unwrap()
        )
        .unwrap(),
        StateResponse {
            total_bond_amount: Uint256::from(100u128),
            global_reward_index: Decimal256::zero(),
            last_distributed: default_genesis_seconds,
        }
    );

    // bond 100 more tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    env.block.time = env.block.time.plus_seconds(10);

    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_index: Decimal256::from_ratio(1000, 1),
            pending_reward: Uint256::from(100000u128),
            bond_amount: Uint256::from(200u128),
        }
    );

    assert_eq!(
        from_binary::<StateResponse>(&query(deps.as_ref(), env, QueryMsg::State {}).unwrap())
            .unwrap(),
        StateResponse {
            total_bond_amount: Uint256::from(200u128),
            global_reward_index: Decimal256::from_ratio(1000, 1),
            last_distributed: default_genesis_seconds + 10,
        }
    );

    // failed with unautorized
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    let info = mock_info("staking0001", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {})
}

#[test]
fn test_unbond() {
    let mut deps = mock_dependencies(&[]);
    let default_genesis_seconds: u64 = mock_env().block.time.seconds();

    let msg = InstantiateMsg {
        miaw_token: "miaw0000".to_string(),
        miaw_lp_token: "miawlp0000".to_string(),
        distribution_schedule: vec![
            (
                default_genesis_seconds,
                default_genesis_seconds + 100,
                Uint256::from(1000000u128),
            ),
            (
                default_genesis_seconds + 100,
                default_genesis_seconds + 200,
                Uint256::from(10000000u128),
            ),
        ],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("miawlp0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // unbond 150 tokens; failed
    let msg = ExecuteMsg::Unbond {
        amount: Some(Uint256::from(150u128)),
    };

    let info = mock_info("addr0000", &[]);
    let err = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap_err();
    assert_eq!(err, ContractError::InvalidUnbondAmount {});

    // normal unbond
    let msg = ExecuteMsg::Unbond {
        amount: Some(Uint256::from(60u128)),
    };

    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "miawlp0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(60u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );

    // unbond remaining
    let msg = ExecuteMsg::Unbond { amount: None };

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "miawlp0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(40u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );
}

#[test]
fn test_compute_reward() {
    let mut deps = mock_dependencies(&[]);
    let default_genesis_seconds: u64 = mock_env().block.time.seconds();

    let msg = InstantiateMsg {
        miaw_token: "miaw0000".to_string(),
        miaw_lp_token: "miawlp0000".to_string(),
        distribution_schedule: vec![
            (
                default_genesis_seconds,
                default_genesis_seconds + 100,
                Uint256::from(1000000u128),
            ),
            (
                default_genesis_seconds + 100,
                default_genesis_seconds + 200,
                Uint256::from(2000000u128),
            ),
        ],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("miawlp0000", &[]);
    let mut env = mock_env();
    let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    // 10 seconds passed
    // 100,000 rewards distributed
    env.block.time = env.block.time.plus_seconds(10);

    // bond 100 more tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_index: Decimal256::from_ratio(1000, 1),
            pending_reward: Uint256::from(100000u128),
            bond_amount: Uint256::from(200u128),
        }
    );

    // 100 seconds passed (90 first slot + 10 next slot)
    // 900,000 + 200,000 rewards distributed
    env.block.time = env.block.time.plus_seconds(100);
    let info = mock_info("addr0000", &[]);

    // unbond
    let msg = ExecuteMsg::Unbond {
        amount: Some(Uint256::from(100u128)),
    };
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(
        from_binary::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                },
            )
            .unwrap()
        )
        .unwrap(),
        StakerInfoResponse {
            staker: "addr0000".to_string(),
            reward_index: Decimal256::from_ratio(6500, 1), // 1,000 + 5,500
            pending_reward: Uint256::from(1200000u128),    // 0.1 + 1.1
            bond_amount: Uint256::from(100u128),
        }
    );
}

#[test]
fn test_claim_rewards() {
    let mut deps = mock_dependencies(&[]);
    let default_genesis_seconds: u64 = mock_env().block.time.seconds();

    let msg = InstantiateMsg {
        miaw_token: "miaw0000".to_string(),
        miaw_lp_token: "miawlp0000".to_string(),
        distribution_schedule: vec![
            (
                default_genesis_seconds,
                default_genesis_seconds + 100,
                Uint256::from(1000000u128),
            ),
            (
                default_genesis_seconds + 100,
                default_genesis_seconds + 200,
                Uint256::from(10000000u128),
            ),
        ],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("miawlp0000", &[]);
    let mut env = mock_env();
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // 100 seconds passed
    // 1,000,000 rewards distributed
    env.block.time = env.block.time.plus_seconds(100);
    let info = mock_info("addr0000", &[]);

    let msg = ExecuteMsg::ClaimRewards {};
    let res = execute(deps.as_mut(), env, info, msg).unwrap();

    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "miaw0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "addr0000".to_string(),
                amount: Uint128::from(1000000u128),
            })
            .unwrap(),
            funds: vec![],
        }))]
    );
}

#[test]
fn test_bond_hook() {
    let mut deps = mock_dependencies(&[]);
    let default_genesis_seconds: u64 = mock_env().block.time.seconds();

    let msg = InstantiateMsg {
        miaw_token: "miaw0000".to_string(),
        miaw_lp_token: "miawlp0000".to_string(),
        distribution_schedule: vec![
            (
                default_genesis_seconds,
                default_genesis_seconds + 100,
                Uint256::from(1000000u128),
            ),
            (
                default_genesis_seconds + 100,
                default_genesis_seconds + 200,
                Uint256::from(10000000u128),
            ),
        ],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // bond 100 tokens
    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    let info = mock_info("miawlp0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    deps.querier.with_balances(&[(
        &"miawlp0000".to_string(),
        &[(
            &MOCK_CONTRACT_ADDR.to_string(),
            &Uint128::from(100u128 + 300u128),
        )],
    )]);

    let msg = ExecuteMsg::BondHook {};
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "bond_hook"),
            attr("owner", "addr0000"),
            attr("amount", "300"),
        ]
    );
    assert!(res.messages.is_empty());
}

#[test]
fn test_query_stakers() {
    let mut deps = mock_dependencies(&[]);
    let default_genesis_seconds: u64 = mock_env().block.time.seconds();

    let msg = InstantiateMsg {
        miaw_token: "miaw0000".to_string(),
        miaw_lp_token: "miawlp0000".to_string(),
        distribution_schedule: vec![(
            default_genesis_seconds,
            default_genesis_seconds + 100,
            Uint256::from(1000000u128),
        )],
    };
    let info = mock_info("addr0000", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0000".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });

    let info = mock_info("miawlp0000", &[]);
    let env = mock_env();
    execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0001".to_string(),
        amount: Uint128::from(200u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "addr0002".to_string(),
        amount: Uint128::from(300u128),
        msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
    });
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(
        from_binary::<StakersInfoResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::StakersInfo {
                    start_after: None,
                    limit: None,
                    order_by: Some(OrderBy::Asc),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakersInfoResponse {
            stakers: vec![
                StakerInfoResponse {
                    staker: "addr0000".to_string(),
                    reward_index: Decimal256::zero(),
                    pending_reward: Uint256::zero(),
                    bond_amount: Uint256::from(100u128),
                },
                StakerInfoResponse {
                    staker: "addr0001".to_string(),
                    reward_index: Decimal256::zero(),
                    pending_reward: Uint256::zero(),
                    bond_amount: Uint256::from(200u128),
                },
                StakerInfoResponse {
                    staker: "addr0002".to_string(),
                    reward_index: Decimal256::zero(),
                    pending_reward: Uint256::zero(),
                    bond_amount: Uint256::from(300u128),
                },
            ]
        }
    );
    assert_eq!(
        from_binary::<StakersInfoResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::StakersInfo {
                    start_after: None,
                    limit: None,
                    order_by: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakersInfoResponse {
            stakers: vec![
                StakerInfoResponse {
                    staker: "addr0002".to_string(),
                    reward_index: Decimal256::zero(),
                    pending_reward: Uint256::zero(),
                    bond_amount: Uint256::from(300u128),
                },
                StakerInfoResponse {
                    staker: "addr0001".to_string(),
                    reward_index: Decimal256::zero(),
                    pending_reward: Uint256::zero(),
                    bond_amount: Uint256::from(200u128),
                },
                StakerInfoResponse {
                    staker: "addr0000".to_string(),
                    reward_index: Decimal256::zero(),
                    pending_reward: Uint256::zero(),
                    bond_amount: Uint256::from(100u128),
                },
            ]
        }
    );
    assert_eq!(
        from_binary::<StakersInfoResponse>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::StakersInfo {
                    start_after: Some("addr0002".to_string()),
                    limit: Some(1u32),
                    order_by: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakersInfoResponse {
            stakers: vec![StakerInfoResponse {
                staker: "addr0001".to_string(),
                reward_index: Decimal256::zero(),
                pending_reward: Uint256::zero(),
                bond_amount: Uint256::from(200u128),
            },]
        }
    );
    assert_eq!(
        from_binary::<StakersInfoResponse>(
            &query(
                deps.as_ref(),
                env,
                QueryMsg::StakersInfo {
                    start_after: Some("addr0001".to_string()),
                    limit: Some(1u32),
                    order_by: Some(OrderBy::Asc),
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakersInfoResponse {
            stakers: vec![StakerInfoResponse {
                staker: "addr0002".to_string(),
                reward_index: Decimal256::zero(),
                pending_reward: Uint256::zero(),
                bond_amount: Uint256::from(300u128),
            },]
        }
    );
}
