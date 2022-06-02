#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env,
    MessageInfo, Response, StdResult, Uint128, Addr, BankMsg, WasmMsg, CosmosMsg, Coin, SubMsg, Reply, StdError
};

use smartwallet::wallet::{
    ExecuteMsg, InstantiateMsg, QueryMsg, ConfigResponse, HotWallet, HotWalletStateResponse, WhitelistedContract, Cw3InstantiateMsg, MultiSigVoter, Duration
};
use protobuf::Message;
use crate::response::MsgInstantiateContractResponse;
use crate::state::{CONFIG, HOT_WALLETS, Config, HotWalletState};
use std::cmp::{min, max};
use crate::tax_querier::{query_balance, deduct_tax};
use moneymarket::market::ExecuteMsg::{DepositStable, RepayStable};
use basset::reward::ExecuteMsg::ClaimRewards;
use crate::error::ContractError;

pub const GAS_BUFFER: u64 = 100000000u64;
pub const ANCHOR_MARKET_CONTRACT: &str = "anchor_market";
pub const BLUNA_REWARD_CONTRACT: &str = "bluna_reward";
pub const ANCHOR_EARN_DEPOSIT_ID: u64 = 0u64;
pub const BLUNA_CLAIM_ID: u64 = 1u64;
pub const ANCHOR_REPAY_STABLE_ID: u64 = 2u64;

pub const SPAWN_MULTISIG_REPLY_ID: u64 = 100u64;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {

    match msg{
        InstantiateMsg::ExistingMultiSig {hot_wallets, cw3_address, whitelisted_contracts} => instantiate_existing_multisig(deps, hot_wallets, cw3_address, whitelisted_contracts),
        InstantiateMsg::SpawnMultiSig{hot_wallets, whitelisted_contracts, max_voting_period_in_blocks, required_weight, multisig_voters, cw3_code_id} => instantiate_spawn_multisig(deps, info, hot_wallets, whitelisted_contracts, max_voting_period_in_blocks, required_weight, multisig_voters, cw3_code_id),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn instantiate_existing_multisig(
    deps: DepsMut,
    hot_wallets: Vec<HotWallet>,
    cw3_address: String,
    whitelisted_contracts: Vec<WhitelistedContract>,
) -> Result<Response, ContractError> {

    let config = Config {
        hot_wallets: hot_wallets,
        cw3_address: deps.api.addr_validate(&cw3_address)?,
        whitelisted_contracts: whitelisted_contracts,
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attributes(vec![("action", "init_existing_multisig")]))
}

#[allow(clippy::too_many_arguments)]
pub fn instantiate_spawn_multisig(
    deps: DepsMut,
    info: MessageInfo,
    hot_wallets: Vec<HotWallet>,
    whitelisted_contracts: Vec<WhitelistedContract>,
    max_voting_period_in_blocks: u64,
    required_weight: u64,
    multisig_voters: Vec<MultiSigVoter>,
    cw3_code_id: u64,
) -> Result<Response, ContractError> {

    CONFIG.save(deps.storage, &Config{
        hot_wallets: hot_wallets, 
        cw3_address: Addr::unchecked(""), 
        whitelisted_contracts: whitelisted_contracts,
    })?;

    Ok(Response::default()
    .add_submessage(SubMsg::reply_on_success(
        CosmosMsg::Wasm(WasmMsg::Instantiate{
            admin: Some(info.sender.to_string()),
            code_id: cw3_code_id,
            msg: to_binary(&Cw3InstantiateMsg{
                voters: multisig_voters,
                required_weight: required_weight,
                max_voting_period: Duration::Height(max_voting_period_in_blocks),
            })?,
            funds: vec![],
            label: String::from("multisig"),
        }),
        SPAWN_MULTISIG_REPLY_ID,
    )))
}

#[entry_point]
pub fn reply(deps: DepsMut, _env: Env, reply: Reply) -> Result<Response, ContractError> {
    match reply.id{
        SPAWN_MULTISIG_REPLY_ID => {

            let data = reply.result.unwrap().data.unwrap();
            let res: MsgInstantiateContractResponse = Message::parse_from_bytes(data.as_slice()).map_err(|_| {
                StdError::parse_err("MsgInstantiateContractResponse", "failed to parse data")
            })?;
            
            let mut config: Config = CONFIG.load(deps.storage)?;
            config.cw3_address = deps.api.addr_validate(res.get_contract_address())?;
            CONFIG.save(deps.storage, &config)?;

            Ok(Response::new())
        },
        _ => Err(ContractError::InvalidReplyId{})
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {

        //hot wallet actions
        ExecuteMsg::AnchorEarnDeposit {amount} => execute_anchor_earn_deposit(deps, info, amount), //id=0
        ExecuteMsg::BlunaClaim{} => execute_bluna_claim_rewards(deps, info), //id=1
        ExecuteMsg::RepayStable{amount} => execute_repay_stable(deps, info, amount), //id=2
        ExecuteMsg::FillUpGas{} => execute_fill_up_gas(deps, env, info), //any
        
        ExecuteMsg::ExecuteHotCommand {contract_address, funds, command} => execute_hot_command(deps, info, contract_address, funds, command), //execute whitelisted wasm message

        //hot wallet mgmt
        ExecuteMsg::RemoveHot {address} => execute_remove_hot(deps, info, address),
        ExecuteMsg::UpsertHot {hot_wallet} => execute_upsert_hot(deps, info, hot_wallet),
        ExecuteMsg::ReplaceContractWhitelist { whitelisted_contracts } => execute_replace_contracts(deps, info, whitelisted_contracts),


        //update multsig
        ExecuteMsg::ReplaceMultisig {address} => execute_replace_multisig(deps, info, address),

        //generalized exec for multisig
        ExecuteMsg::Execute {command} => execute_command(deps, info, command),
    }
}


#[allow(clippy::too_many_arguments)]
pub fn execute_hot_command(
    deps: DepsMut,
    info: MessageInfo,
    contract_address: String,
    funds: Vec<Coin>,
    command: Binary,
) -> Result<Response, ContractError> {

    let config: Config = CONFIG.load(deps.storage)?;

    let hot_wallet_config = config.hot_wallets.iter().find(|&x| x.address == info.sender.to_string());

    //hot wallet check
    if hot_wallet_config.is_none(){
        return Err(ContractError::Unauthorized{});
    }

    let destination_contract = config.whitelisted_contracts.iter().find(|&x| x.address == contract_address.clone());

    //contract check
    if destination_contract.is_none(){
        return Err(ContractError::ContractNotWhitelisted{});
    }

    let hot_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: destination_contract.unwrap().address.clone(),
        funds: funds,
        msg: command,
    });

    Ok(Response::new().add_attributes(vec![("action", "hot_command")]).add_message(hot_msg))
}


#[allow(clippy::too_many_arguments)]
pub fn execute_repay_stable(
    deps: DepsMut,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {

    let config: Config = CONFIG.load(deps.storage)?;

    let hot_wallet_config = config.hot_wallets.iter().find(|&x| x.address == info.sender.to_string());

    //hot wallet check
    if hot_wallet_config.is_none(){
        return Err(ContractError::Unauthorized{});
    }

    //hot wallet is enabled for this action
    if hot_wallet_config.unwrap().whitelisted_messages.iter().find(|&&x| x == ANCHOR_REPAY_STABLE_ID).is_none(){
        return Err(ContractError::UnauthorizedAction{});
    }

    let anchor_market_contract = config.whitelisted_contracts.iter().find(|&x| x.label == String::from(ANCHOR_MARKET_CONTRACT));

    //contract check
    if anchor_market_contract.is_none(){
        return Err(ContractError::ContractNotWhitelisted{});
    }

    let repay_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: anchor_market_contract.unwrap().address.clone(),
        funds: vec![Coin{
            denom: String::from("uusd"),
            amount: amount,
        }],
        msg: to_binary(&RepayStable{})?,
    });

    Ok(Response::new().add_attributes(vec![("action", "anchor_earn_deposit")]).add_message(repay_msg))
}

#[allow(clippy::too_many_arguments)]
pub fn execute_anchor_earn_deposit(
    deps: DepsMut,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {

    let config: Config = CONFIG.load(deps.storage)?;

    let hot_wallet_config = config.hot_wallets.iter().find(|&x| x.address == info.sender.to_string());

    //hot wallet check
    if hot_wallet_config.is_none(){
        return Err(ContractError::Unauthorized{});
    }

    //hot wallet is enabled for this action
    if hot_wallet_config.unwrap().whitelisted_messages.iter().find(|&&x| x == ANCHOR_EARN_DEPOSIT_ID).is_none(){
        return Err(ContractError::UnauthorizedAction{});
    }

    let anchor_market_contract = config.whitelisted_contracts.iter().find(|&x| x.label == String::from(ANCHOR_MARKET_CONTRACT));

    //contract check
    if anchor_market_contract.is_none(){
        return Err(ContractError::ContractNotWhitelisted{});
    }

    if query_balance(deps.as_ref(), info.sender.to_string(), String::from("uusd")).unwrap() < Uint128::from(GAS_BUFFER){
        return Err(ContractError::SmartWalletGas{});
    }

    //figure out send amount, net gas buffer
    let gas_adjusted_amount = min(
        query_balance(deps.as_ref(), info.sender.to_string(), String::from("uusd")).unwrap() - Uint128::from(GAS_BUFFER),
        amount.into());

    let earn_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: anchor_market_contract.unwrap().address.clone(),
        funds: vec![Coin{
            denom: String::from("uusd"),
            amount: gas_adjusted_amount,
        }],
        msg: to_binary(&DepositStable{})?,
    });

    Ok(Response::new().add_attributes(vec![("action", "anchor_earn_deposit")]).add_message(earn_msg))
}

#[allow(clippy::too_many_arguments)]
pub fn execute_bluna_claim_rewards(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response, ContractError> {

    let config: Config = CONFIG.load(deps.storage)?;

    let hot_wallet_config = config.hot_wallets.iter().find(|&x| x.address == info.sender.to_string());

    //hot wallet check
    if hot_wallet_config.is_none(){
        return Err(ContractError::Unauthorized{});
    }

    //hot wallet is enabled for this action
    if hot_wallet_config.unwrap().whitelisted_messages.iter().find(|&&x| x == BLUNA_CLAIM_ID).is_none(){
        return Err(ContractError::UnauthorizedAction{});
    }

    let bluna_reward_contract = config.whitelisted_contracts.iter().find(|&x| x.label == String::from(BLUNA_REWARD_CONTRACT));

    //contract check
    if bluna_reward_contract.is_none(){
        return Err(ContractError::ContractNotWhitelisted{});
    }

    let claim_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: bluna_reward_contract.unwrap().address.clone(),
        funds: vec![],
        msg: to_binary(&ClaimRewards{recipient: None})?,
    });


    Ok(Response::new().add_attributes(vec![("action", "bluna_claim_rewards")]).add_message(claim_msg))
}

#[allow(clippy::too_many_arguments)]
pub fn execute_fill_up_gas(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {

    let config: Config = CONFIG.load(deps.storage)?;

    let hot_wallet_config = config.hot_wallets.iter().find(|&x| x.address == info.sender.to_string());

    //hot wallet check
    if hot_wallet_config.is_none(){
        return Err(ContractError::Unauthorized{});
    }

    let mut hot_wallet_state = HOT_WALLETS
        .may_load(deps.storage, info.sender.to_string())?
        .unwrap_or(
            HotWalletState{
                address: info.sender.to_string(), 
                last_gas_fillup: env.block.time.seconds() - hot_wallet_config.unwrap().gas_cooldown - 10u64,
        });

    //cooldown check
    if hot_wallet_state.last_gas_fillup + hot_wallet_config.unwrap().gas_cooldown > env.block.time.seconds(){
        return Err(ContractError::GasCooldown{});
    }

    //figure out how much gas needed to fill hot wallet's tank
    let hot_wallet_gas_level = query_balance(deps.as_ref(), info.sender.to_string(), String::from("uusd")).unwrap();

    if hot_wallet_config.unwrap().gas_tank_max <= hot_wallet_gas_level {
        return Err(ContractError::GasTankFull{});
    }

    let hot_wallet_gas_need = hot_wallet_config.unwrap().gas_tank_max - hot_wallet_gas_level;

    //sufficient smart_wallet uusd check
    if query_balance(deps.as_ref(), env.contract.address.to_string(), String::from("uusd")).unwrap() < hot_wallet_gas_need + Uint128::from(GAS_BUFFER) {
        return Err(ContractError::SmartWalletGas{});
    }

    let bank_msg = CosmosMsg::Bank(BankMsg::Send{
        to_address: info.sender.to_string(),
        amount: vec![deduct_tax(
            &deps.querier,
            Coin{
                denom: String::from("uusd"),
                amount: hot_wallet_gas_need,
            }
        )?]
    });

    hot_wallet_state.last_gas_fillup = env.block.time.seconds();

    HOT_WALLETS.save(deps.storage, info.sender.to_string(), &hot_wallet_state)?;

    Ok(Response::new().add_attributes(vec![("action", "fill_up_gas")]).add_message(bank_msg))
}

#[allow(clippy::too_many_arguments)]
pub fn execute_remove_hot(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {

    let mut config: Config = CONFIG.load(deps.storage)?;

    //multisig check
    if info.sender.to_string() != config.cw3_address{
        return Err(ContractError::Unauthorized{});
    }

    let hot_wallet_config = config.hot_wallets.iter().find(|&x| x.address == address);

    //check if valid hot address
    if hot_wallet_config.is_none(){
        return Err(ContractError::InvalidHotAddress{});
    }

    //remove from state
    HOT_WALLETS.remove(deps.storage, address.clone());

    //remove from config
    config.hot_wallets.retain(|x| x.address != address);

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attributes(vec![("action", "remove_hot")]))
}

#[allow(clippy::too_many_arguments)]
pub fn execute_upsert_hot(
    deps: DepsMut,
    info: MessageInfo,
    hot_wallet: HotWallet,
) -> Result<Response, ContractError> {

    let mut config: Config = CONFIG.load(deps.storage)?;

    //multisig check
    if info.sender.to_string() != config.cw3_address{
        return Err(ContractError::Unauthorized{});
    }

    //check if valid hot address
    let address: Addr = deps.api.addr_validate(&hot_wallet.address)?;

    //remove from config
    config.hot_wallets.retain(|x| x.address != address);
    config.hot_wallets.push(hot_wallet);

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attributes(vec![("action", "upsert_hot")]))
}

#[allow(clippy::too_many_arguments)]
pub fn execute_replace_contracts(
    deps: DepsMut,
    info: MessageInfo,
    whitelisted_contracts: Vec<WhitelistedContract>,
) -> Result<Response, ContractError> {

    let mut config: Config = CONFIG.load(deps.storage)?;

    //multisig check
    if info.sender.to_string() != config.cw3_address{
        return Err(ContractError::Unauthorized{});
    }

    config.whitelisted_contracts = whitelisted_contracts;

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attributes(vec![("action", "replace_contracts")]))
}

#[allow(clippy::too_many_arguments)]
pub fn execute_replace_multisig(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {

    let mut config: Config = CONFIG.load(deps.storage)?;

    //multisig check
    if info.sender.to_string() != config.cw3_address{
        return Err(ContractError::Unauthorized{});
    }

    config.cw3_address = deps.api.addr_validate(&address)?;

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attributes(vec![("action", "replace_multisig")]))
}

#[allow(clippy::too_many_arguments)]
pub fn execute_command(
    deps: DepsMut,
    info: MessageInfo,
    command: CosmosMsg,
) -> Result<Response, ContractError> {

    let config: Config = CONFIG.load(deps.storage)?;

    //multisig check
    if info.sender.to_string() != config.cw3_address{
        return Err(ContractError::Unauthorized{});
    }

    Ok(Response::new().add_attributes(vec![("action", "execute_command")]).add_message(command))
}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => Ok(to_binary(&query_config(deps)?)?),
        QueryMsg::HotWallet {address} => Ok(to_binary(&query_hot_wallet_state(deps, env, address)?)?),
    }
}


pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config: Config = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        hot_wallets: config.hot_wallets,
        cw3_address: config.cw3_address,
        whitelisted_contracts: config.whitelisted_contracts,
    })
  }
  

pub fn query_hot_wallet_state(deps: Deps, env: Env, address: String) -> StdResult<HotWalletStateResponse> {

    let hot_wallet: HotWalletState = HOT_WALLETS.may_load(deps.storage, address)?.unwrap_or(HotWalletState{address: String::from(""), last_gas_fillup: env.block.time.seconds()});

    //gas_time_left is janky; it's the num seconds since last fillup
    Ok(HotWalletStateResponse{
        address: hot_wallet.address,
        gas_time_left: max(env.block.time.seconds() - hot_wallet.last_gas_fillup, 0u64)
    })
  }
