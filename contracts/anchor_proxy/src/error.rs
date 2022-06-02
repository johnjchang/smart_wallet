use cosmwasm_std::StdError;
use thiserror::Error;

/// only purpose of this is to accommodate the execute command message.
/// have no idea how to convert a cosmosmsg<empty> to a terramsgwrapper
#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
  #[error("{0}")]
  Std(#[from] StdError),


  #[error("unauthorized")]
  Unauthorized {},

  #[error("unauthorized action")]
  UnauthorizedAction {},

  #[error("gas cooldown not done")]
  GasCooldown {},

  #[error("smart_wallet insufficient gas")]
  SmartWalletGas {},

  #[error("hot address does not exist")]
  InvalidHotAddress {},

  #[error("must whitelist contract")]
  ContractNotWhitelisted {},

  #[error("gas tank is full")]
  GasTankFull{},

  #[error("reply id not implemented")]
  InvalidReplyId,

  #[error("unauthorized message for hot wallet")]
  UnauthorizedHotMessage,

  #[error("reply id not implemented")]
  InvalidReceiveMsg {},

  #[error("must deposit positive number of assets")]
  InvalidDepositAmount {},
}
