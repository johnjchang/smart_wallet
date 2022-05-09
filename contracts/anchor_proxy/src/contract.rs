#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, from_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128, Addr, BankMsg, WasmMsg, CosmosMsg, Coin, SubMsg, Reply, StdError
};

use cosmwasm_bignumber::Uint256;

use smartwallet::anchor_proxy::{ExecuteMsg, InstantiateMsg, QueryMsg, ConfigResponse, BalanceCheckResponse, CustodyContractInfo, Cw20HookMsg};

use crate::state::{CONFIG, Config, CustodyContractInfo as StateCustodyContractInfo};
use moneymarket::{
    market::ExecuteMsg as AnchorExecuteMsg,
    overseer::ExecuteMsg as OverseerExecuteMsg,
    custody::ExecuteMsg as CustodyExecuteMsg,
};
use basset::reward::{ExecuteMsg as BassetExecuteMsg};
use crate::error::ContractError;
use cw20::Cw20ReceiveMsg;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {

    let mut custody_contracts: Vec<StateCustodyContractInfo> = vec![];

    for contract in msg.custody_contracts{
        custody_contracts.push(StateCustodyContractInfo{
            address: deps.api.addr_validate(&contract.address)?,
            collateral_token: deps.api.addr_validate(&contract.collateral_token)?,
            label: contract.label,
        });
    }

    CONFIG.save(deps.storage, &Config{
        owner: info.sender,
        market_contract: deps.api.addr_validate(&msg.market_contract)?,
        overseer_contract: deps.api.addr_validate(&msg.overseer_contract)?,
        custody_contracts: custody_contracts,
        liquidation_contract: deps.api.addr_validate(&msg.liquidation_contract)?,
    })?;
    
    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {

        ExecuteMsg::Receive(msg) => receive_cw20(deps, info, msg),
        ExecuteMsg::UpdateConfig{
            owner,
            market_contract,
            overseer_contract,
            liquidation_contract,
        } => execute_update_config(deps, env, info, owner, market_contract, overseer_contract, liquidation_contract),
        ExecuteMsg::UpsertCustodyContract{custody_contract} => execute_upsert_custody_contract(deps, env, info, custody_contract),

        ExecuteMsg::DepositStable {} => execute_deposit_stable(deps, env, info),
        ExecuteMsg::ClaimRewards {} => execute_claim_rewards(deps, env, info),

        ExecuteMsg::LiquidateCollateral { borrower } => execute_liquidate_collateral(deps, env, info, borrower),

        ExecuteMsg::WithdrawCollateral { collateral_token, amount } => execute_withdraw_collateral(deps, env, info, collateral_token, amount),

        ExecuteMsg::SubmitBid { collateral_token, premium_slot } => Ok(Response::new()),
        ExecuteMsg::RetractBid {bid_idx, amount} => Ok(Response::new()),
        ExecuteMsg::ActivateBids {collateral_token, bids_idx} => Ok(Response::new()),
        ExecuteMsg::ClaimLiquidations {collateral_token, bids_idx} => Ok(Response::new()),
    }
}


pub fn receive_cw20(
    deps: DepsMut,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let contract_addr = info.sender;

    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::RedeemStable {}) => {
            // aust check

            let cw20_sender_addr = deps.api.addr_validate(&cw20_msg.sender)?;
            //redeem_stable(deps, cw20_sender_addr, cw20_msg.amount.into())
            Ok(Response::new())
        },

        Ok(Cw20HookMsg::DepositCollateral {}) => {
            // anchor-whitelisted collateral check

            let cw20_sender_addr = deps.api.addr_validate(&cw20_msg.sender)?;
            //deposit_collateral(deps, cw20_sender_addr, cw20_msg.amount.into())
            Ok(Response::new())
        },

        _ => Err(ContractError::InvalidReceiveMsg {}),
    }
}

pub fn execute_withdraw_collateral(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    collateral_token: String,
    amount: Option<Uint256>,
) -> Result<Response, ContractError>{

    let config: Config = CONFIG.load(deps.storage)?;

    let collateral_address: Addr = deps.api.addr_validate(&collateral_token)?;

    let custody_contract = &config
        .custody_contracts
        .iter()
        .find(|x| x.address == collateral_address.clone())
        .ok_or_else(||{
            ContractError::Unauthorized{}
        })?.address;

    let withdraw_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: custody_contract.into(),
        funds: vec![],
        msg: to_binary(&CustodyExecuteMsg::WithdrawCollateral{amount: amount})?,
    });

    Ok(Response::new().add_attributes(vec![("action", "overseer_liquidate_collateral")]).add_message(withdraw_msg))
}

pub fn execute_liquidate_collateral(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    borrower: String,
) -> Result<Response, ContractError>{

    let config: Config = CONFIG.load(deps.storage)?;

    let liquidate_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.overseer_contract.into(),
        funds: vec![],
        msg: to_binary(&OverseerExecuteMsg::LiquidateCollateral{borrower: borrower})?,
    });

    Ok(Response::new().add_attributes(vec![("action", "overseer_liquidate_collateral")]).add_message(liquidate_msg))
}

pub fn execute_claim_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError>{

    let config: Config = CONFIG.load(deps.storage)?;

    let claim_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.market_contract.into(),
        funds: vec![],
        msg: to_binary(&AnchorExecuteMsg::ClaimRewards{to: None})?,
    });

    Ok(Response::new().add_attributes(vec![("action", "market_claim_rewards")]).add_message(claim_msg))
}

pub fn execute_deposit_stable(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError>{

    //fetch payment
    let payment = info 
        .funds
        .iter()
        .find(|x| x.denom == "uusd" && x.amount > Uint128::zero())
        .ok_or_else(|| {
            StdError::generic_err(format!("No {} sent", "uusd"))
        })?.amount;

    let config: Config = CONFIG.load(deps.storage)?;

    let deposit_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.market_contract.into(),
        funds: vec![Coin{
            denom: String::from("uusd"),
            amount: payment.into(),
        }],
        msg: to_binary(&AnchorExecuteMsg::DepositStable{})?,
    });

    Ok(Response::new().add_attributes(vec![("action", "market_deposit_stable")]).add_message(deposit_msg))
}


pub fn execute_upsert_custody_contract(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    custody_contract: CustodyContractInfo,
) -> Result<Response, ContractError>{

    //owner check
    let mut config: Config = CONFIG.load(deps.storage)?;

    if config.owner != info.sender{
        return Err(ContractError::Unauthorized{});
    }

    let address: Addr = deps.api.addr_validate(&custody_contract.address)?;
    let new_custody_contract: StateCustodyContractInfo = StateCustodyContractInfo{
        address: address.clone(),
        label: custody_contract.label,
        collateral_token: deps.api.addr_validate(&custody_contract.collateral_token)?,
    };

    config.custody_contracts.retain(|x| x.address != address);
    config.custody_contracts.push(new_custody_contract);

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new())
}

pub fn execute_update_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: Option<String>,
    market_contract: Option<String>,
    overseer_contract: Option<String>,
    liquidation_contract: Option<String>,
) -> Result<Response, ContractError>{

    //owner check
    let mut config: Config = CONFIG.load(deps.storage)?;

    if config.owner != info.sender{
        return Err(ContractError::Unauthorized{});
    }

    if let Some(owner) = owner{
        config.owner = deps.api.addr_validate(&owner)?;
    }

    if let Some(market_contract) = market_contract{
        config.market_contract = deps.api.addr_validate(&market_contract)?;
    }

    if let Some(overseer_contract) = overseer_contract{
        config.overseer_contract = deps.api.addr_validate(&overseer_contract)?;
    }

    if let Some(liquidation_contract) = liquidation_contract{
        config.liquidation_contract = deps.api.addr_validate(&liquidation_contract)?;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new())
}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => Ok(to_binary(&query_config(deps)?)?),
        QueryMsg::BalanceCheck {address} => Ok(to_binary(&query_balance_check(deps, env, address)?)?),
    }
}


pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    todo!()
}
  

pub fn query_balance_check(deps: Deps, env: Env, address: String) -> StdResult<BalanceCheckResponse> {
    todo!()
}
