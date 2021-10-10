use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_binary, from_slice, to_binary, BalanceResponse, BankQuery, Coin, ContractResult, Empty,
    OwnedDeps, Querier, QuerierResult, QueryRequest, SystemError, SystemResult, Uint128, WasmQuery,
};
use cw20::{BalanceResponse as Cw20BalanceResponse, TokenInfoResponse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// mock_dependencies is a drop-in replacement for cosmwasm_std::testing::mock_dependencies
/// this uses our CustomQuerier.
pub fn mock_dependencies(
    contract_balance: &[Coin],
) -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let custom_querier: WasmMockQuerier =
        WasmMockQuerier::new(MockQuerier::new(&[(MOCK_CONTRACT_ADDR, contract_balance)]));

    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier: custom_querier,
    }
}

pub struct WasmMockQuerier {
    base: MockQuerier<Empty>,
    balance_querier: BalanceQuerier,
    terraswap_factory_querier: TerraswapFactoryQuerier,
}

#[derive(Clone, Default)]
pub struct BalanceQuerier {
    balances: HashMap<String, HashMap<String, Uint128>>,
}

impl BalanceQuerier {
    pub fn new(balances: &[(&String, &[(&String, &Uint128)])]) -> Self {
        BalanceQuerier {
            balances: balances_to_map(balances),
        }
    }
}

pub(crate) fn balances_to_map(
    balances: &[(&String, &[(&String, &Uint128)])],
) -> HashMap<String, HashMap<String, Uint128>> {
    let mut balances_map: HashMap<String, HashMap<String, Uint128>> = HashMap::new();
    for (contract_addr, balances) in balances.iter() {
        let mut contract_balances_map: HashMap<String, Uint128> = HashMap::new();
        for (asset, balance) in balances.iter() {
            contract_balances_map.insert(asset.to_string(), **balance);
        }

        balances_map.insert(contract_addr.to_string(), contract_balances_map);
    }
    balances_map
}

#[derive(Clone, Default)]
pub struct TerraswapFactoryQuerier {
    pairs: HashMap<String, String>,
}

impl TerraswapFactoryQuerier {
    pub fn new(pairs: &[(&String, &String)]) -> Self {
        TerraswapFactoryQuerier {
            pairs: pairs_to_map(pairs),
        }
    }
}

pub(crate) fn pairs_to_map(pairs: &[(&String, &String)]) -> HashMap<String, String> {
    let mut pairs_map: HashMap<String, String> = HashMap::new();
    for (key, pair) in pairs.iter() {
        pairs_map.insert(key.to_string(), pair.to_string());
    }
    pairs_map
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        // MockQuerier doesn't support Custom, so we ignore it completely here
        let request: QueryRequest<Empty> = match from_slice(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                })
            }
        };
        self.handle_query(&request)
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MockQueryMsg {
    TokenInfo {},
    Balance { address: String },
}

impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
        match &request {
            QueryRequest::Wasm(WasmQuery::Smart { contract_addr, msg }) => {
                match from_binary(msg).unwrap() {
                    MockQueryMsg::TokenInfo {} => {
                        let balances: &HashMap<String, Uint128> =
                            match self.balance_querier.balances.get(contract_addr) {
                                Some(balances) => balances,
                                None => {
                                    return SystemResult::Err(SystemError::InvalidRequest {
                                        error: format!(
                                            "No balance info exists for the contract {}",
                                            contract_addr
                                        ),
                                        request: msg.as_slice().into(),
                                    })
                                }
                            };
                        let mut total_supply = Uint128::zero();

                        for balance in balances {
                            total_supply += *balance.1;
                        }
                        let token_info = TokenInfoResponse {
                            name: "token0000".to_string(),
                            symbol: "tokenX".to_string(),
                            decimals: 6,
                            total_supply,
                        };
                        SystemResult::Ok(ContractResult::from(to_binary(&token_info)))
                    }
                    MockQueryMsg::Balance { address } => {
                        let balances: &HashMap<String, Uint128> =
                            match self.balance_querier.balances.get(contract_addr) {
                                Some(balances) => balances,
                                None => {
                                    return SystemResult::Err(SystemError::InvalidRequest {
                                        error: format!(
                                            "No balance info exists for the contract {}",
                                            contract_addr
                                        ),
                                        request: msg.as_slice().into(),
                                    })
                                }
                            };
                        let balance = match balances.get(&address) {
                            Some(v) => v,
                            None => {
                                return SystemResult::Err(SystemError::InvalidRequest {
                                    error: "Balance not found".to_string(),
                                    request: msg.as_slice().into(),
                                })
                            }
                        };
                        let res = Cw20BalanceResponse { balance: *balance };
                        SystemResult::Ok(ContractResult::from(to_binary(&res)))
                    }
                }
            }
            QueryRequest::Bank(BankQuery::Balance { address, denom }) => {
                let balances: &HashMap<String, Uint128> = match self
                    .balance_querier
                    .balances
                    .get(denom)
                {
                    Some(balances) => balances,
                    None => {
                        return SystemResult::Err(SystemError::InvalidRequest {
                            error: format!("No balance info exists for the address {}", address),
                            request: Default::default(),
                        })
                    }
                };

                let balance = match balances.get(address) {
                    Some(v) => v,
                    None => {
                        return SystemResult::Err(SystemError::InvalidRequest {
                            error: "Balance not found".to_string(),
                            request: Default::default(),
                        })
                    }
                };
                let bank_res = BalanceResponse {
                    amount: Coin {
                        amount: *balance,
                        denom: denom.to_string(),
                    },
                };
                SystemResult::Ok(ContractResult::from(to_binary(&bank_res)))
            }
            _ => self.base.handle_query(request),
        }
    }
}

impl WasmMockQuerier {
    pub fn new(base: MockQuerier<Empty>) -> Self {
        WasmMockQuerier {
            base,
            balance_querier: BalanceQuerier::default(),
            terraswap_factory_querier: TerraswapFactoryQuerier::default(),
        }
    }

    // configure the terraswap pair
    pub fn with_terraswap_pairs(&mut self, pairs: &[(&String, &String)]) {
        self.terraswap_factory_querier = TerraswapFactoryQuerier::new(pairs);
    }

    pub fn with_balances(&mut self, balances: &[(&String, &[(&String, &Uint128)])]) {
        self.balance_querier = BalanceQuerier::new(balances);
    }
}
