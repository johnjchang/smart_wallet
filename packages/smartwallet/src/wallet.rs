use cosmwasm_std::{Binary, Decimal, Uint128, Addr};
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub hot_wallet: String,
    pub cold_wallets: Vec<String>,
    pub cold_x: u64,
    pub cold_n: u64,
    pub max_expiration: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AnchorEarnDeposit {},
    BlunaClaim {},
    ColdWasmExecute {address: String, command: Binary, expiration: Option<u64>},
    ColdNativeTransfer {address: String, denom: String, amount: Uint128, expiration: Option<u64>},
    ColdConfirm {}, 
    UpdateConfig {
        hot_wallet: Option<String>,
        cold_wallets: Option<Vec<String>>,
        cold_x: Option<u64>,
        cold_n: Option<u64>,
        max_expiration: Option<u64>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    State {},
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub hot_wallet: String,
    pub cold_wallets: Vec<Addr>,
    pub cold_x: u64,
    pub cold_n: u64,
    pub max_expiration: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub cold_running: u64,
    pub expiration: u64,
    pub cold_x: u64,
    pub cold_n: u64,
    pub cold_native_transfer: NativeTransferResponse,
    pub cold_wasm_execute: WasmExecuteResponse,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct NativeTransferResponse{
    pub address: Addr,
    pub denom: String,
    pub amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct WasmExecuteResponse{
    pub address: Addr,
    pub message: Binary,
}