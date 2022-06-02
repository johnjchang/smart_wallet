use cosmwasm_std::Addr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw_storage_plus::{Map, Item};

pub const CONFIG: Item<Config> = Item::new("\u{0}\u{6}config");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub market_contract: Addr,
    pub overseer_contract: Addr,
    pub custody_contracts: Vec<CustodyContractInfo>,
    pub liquidation_contract: Addr,
    pub aust_address: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CustodyContractInfo{
    pub address: Addr,
    pub collateral_token: Addr,
    pub label: String,
}