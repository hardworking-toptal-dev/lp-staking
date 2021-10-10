use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Nothing to stake")]
    NothingToStake {},

    #[error("Cannot unbond more than bond amount")]
    InvalidUnbondAmount {},

    #[error("Invalid Cw20 msg")]
    InvalidCw20Msg {},
}
