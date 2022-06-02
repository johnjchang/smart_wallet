use cosmwasm_std::{Coin, Decimal, QuerierWrapper, StdResult, Uint128, Deps, BalanceResponse, QueryRequest, BankQuery};

use terra_cosmwasm::TerraQuerier;



static DECIMAL_FRACTION: Uint128 = Uint128::new(1_000_000_000_000_000_000u128);

pub fn compute_tax(querier: &QuerierWrapper, coin: &Coin) -> StdResult<Uint128> {
    let terra_querier = TerraQuerier::new(querier);
    let tax_rate: Decimal = (terra_querier.query_tax_rate()?).rate;
    let tax_cap: Uint128 = (terra_querier.query_tax_cap(coin.denom.to_string())?).cap;
    Ok(std::cmp::min(
        (coin.amount.checked_sub(coin.amount.multiply_ratio(
            DECIMAL_FRACTION,
            DECIMAL_FRACTION * tax_rate + DECIMAL_FRACTION,
        )))?,
        tax_cap,
    ))
}

pub fn deduct_tax(querier: &QuerierWrapper, coin: Coin) -> StdResult<Coin> {
    let tax_amount = compute_tax(querier, &coin)?;
    Ok(Coin {
        denom: coin.denom,
        amount: (coin.amount.checked_sub(tax_amount))?,
    })
}

pub fn query_balance(deps: Deps, account_addr: String, denom: String) -> StdResult<Uint128> {
    // load price form the oracle
    let balance: BalanceResponse = deps.querier.query(&QueryRequest::Bank(BankQuery::Balance {
        address: account_addr,
        denom,
    }))?;
    Ok(balance.amount.amount.into())
}