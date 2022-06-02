use cosmwasm_std::Uint128;
use cosmwasm_bignumber::Uint256;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cw20::Cw20ReceiveMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateMsg {
    
    pub owner: String,
    pub market_contract: String,
    pub overseer_contract: String,
    pub custody_contracts: Vec<CustodyContractInfo>,
    pub liquidation_contract: String,
    pub aust_address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CustodyContractInfo{
    pub address: String,
    pub collateral_token: String,
    pub label: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    
    Receive(Cw20ReceiveMsg),
    UpdateConfig{
        owner: Option<String>,
        market_contract: Option<String>,
        overseer_contract: Option<String>,
        liquidation_contract: Option<String>,
    },
    UpsertCustodyContract{custody_contract: CustodyContractInfo},

    //market contract operations
    DepositStable {},
    ClaimRewards {},

    //overseer operations
    LiquidateCollateral {borrower: String},

    //overseer + custody composite
    WithdrawCollateral{collateral_token: String, amount: Option<Uint256>}, //unlock + withdraw

    //liquidation queue operations
    SubmitBid {collateral_token: String, premium_slot: u8 },
    RetractBid {bid_idx: Uint128, amount: Option<Uint256>},
    ActivateBids {collateral_token: String, bids_idx: Option<Vec<Uint128>>},
    ClaimLiquidations {collateral_token: String, bids_idx: Option<Vec<Uint128>>},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    //market contract operations
    RedeemStable {},

    //overseer + custody composite
    DepositCollateral {}, //deposit + lock
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    BalanceCheck {address: String},
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub market_contract: String,
    pub overseer_contract: String,
    pub custody_contracts: Vec<CustodyContractInfo>,
    pub liquidation_contract: String,
}

//TODO: cw_asset::AssetList?
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct BalanceCheckResponse {
    pub balance: Uint128,
}