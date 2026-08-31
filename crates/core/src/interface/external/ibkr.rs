use std::collections::HashSet;
use std::sync::mpsc::Sender;

use polars::prelude::*;

use ibapi::accounts::types::AccountId;
use ibapi::accounts::AccountUpdate;
use ibapi::client::blocking::Client;
use ibapi::contracts::Contract;
use ibapi::market_data::historical::{BarSize, Duration, ToDuration, WhatToShow};

use crate::core::{Instrument, Portfolio};
use crate::interface::MarketApi;
use crate::{Error, Event};

pub struct IbkrInterface {
    host: String,
    client_id: i32,
}

impl IbkrInterface {
    pub fn new(host: String, client_id: i32) -> Self {
        Self { host, client_id }
    }

    fn connect(&self) -> Result<Client, Error> {
        Client::connect(&self.host, self.client_id).map_err(|e| Error::Connection(e.to_string()))
    }
}

impl MarketApi for IbkrInterface {
    fn market_history(
        &self,
        contract: &Contract,
        bar_size: BarSize,
        duration: Duration,
    ) -> PolarsResult<DataFrame> {
        let client = self
            .connect()
            .map_err(|e| PolarsError::ComputeError(e.to_string().into()))?;

        let data = client
            .historical_data(contract, bar_size)
            .what_to_show(WhatToShow::Trades)
            .duration(duration)
            .fetch()
            .map_err(|e| PolarsError::ComputeError(e.to_string().into()))?;

        bars_to_dataframe(&data.bars)
    }

    fn portfolio(&self, account_id: &str) -> Result<Portfolio, Error> {
        let client = self.connect()?;
        let account = AccountId(account_id.to_string());

        let subscription = client
            .account_updates(&account)
            .map_err(|e| Error::Subscription(e.to_string()))?;

        let mut portfolio = Portfolio::default();
        let mut seen = HashSet::new();

        for update in subscription.iter_data() {
            let update =
                update.map_err(|e| Error::Subscription(e.to_string()))?;

            match update {
                AccountUpdate::AccountValue(av) => {
                    portfolio.upsert_metric(av.key, av.value, av.currency);
                }
                AccountUpdate::PortfolioValue(pv) => {
                    if pv.position != 0.0 && seen.insert(pv.contract.contract_id) {
                        let instrument = Instrument {
                            contract_id: pv.contract.contract_id,
                            symbol: pv.contract.symbol.to_string(),
                            security_type: pv.contract.security_type.to_string(),
                            exchange: pv.contract.exchange.to_string(),
                            currency: pv.contract.currency.to_string(),
                        };
                        portfolio.set_holding(instrument, pv.position);
                    }
                }
                AccountUpdate::UpdateTime(_) => {}
                AccountUpdate::End => break,
            }
        }

        Ok(portfolio)
    }

    fn listen(&self, account_id: &str, tx: Sender<Event>) -> Result<(), Error> {
        let client = self.connect()?;
        tx.send(Event::Connected).ok();

        let account = AccountId(account_id.to_string());

        let subscription = client
            .account_updates(&account)
            .map_err(|e| Error::Subscription(e.to_string()))?;

        let mut portfolio_contracts: Vec<Contract> = Vec::new();
        let mut seen = HashSet::new();

        for update in subscription.iter_data() {
            let update = match update {
                Ok(u) => u,
                Err(e) => {
                    tx.send(Event::Disconnected(e.to_string())).ok();
                    return Err(Error::Subscription(e.to_string()));
                }
            };

            match update {
                AccountUpdate::AccountValue(av) => {
                    tx.send(Event::AccountValue(av)).ok();
                }
                AccountUpdate::PortfolioValue(pv) => {
                    if pv.position != 0.0 && seen.insert(pv.contract.contract_id) {
                        portfolio_contracts.push(pv.contract.clone());
                        tx.send(Event::Position(pv)).ok();
                    }
                }
                AccountUpdate::UpdateTime(ut) => {
                    tx.send(Event::UpdateTime(ut.timestamp)).ok();
                }
                AccountUpdate::End => break,
            }
        }

        // Release the subscription so the client can accept new requests.
        drop(subscription);

        for contract in &portfolio_contracts {
            match client
                .historical_data(contract, BarSize::Day)
                .what_to_show(WhatToShow::Trades)
                .duration(30.days())
                .fetch()
            {
                Ok(data) => {
                    tx.send(Event::HistoricalBars {
                        contract_id: contract.contract_id,
                        bars: data.bars,
                    })
                    .ok();
                }
                Err(e) => {
                    tx.send(Event::HistoricalError {
                        contract_id: contract.contract_id,
                        error: e.to_string(),
                    })
                    .ok();
                }
            }
        }

        Ok(())
    }
}

fn bars_to_dataframe(bars: &[ibapi::market_data::historical::Bar]) -> PolarsResult<DataFrame> {
    let dates: Vec<String> = bars.iter().map(|b| b.date.to_string()).collect();
    let opens: Vec<f64> = bars.iter().map(|b| b.open).collect();
    let highs: Vec<f64> = bars.iter().map(|b| b.high).collect();
    let lows: Vec<f64> = bars.iter().map(|b| b.low).collect();
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let volumes: Vec<f64> = bars.iter().map(|b| b.volume).collect();

    DataFrame::new(vec![
        Column::new("date".into(), &dates),
        Column::new("open".into(), &opens),
        Column::new("high".into(), &highs),
        Column::new("low".into(), &lows),
        Column::new("close".into(), &closes),
        Column::new("volume".into(), &volumes),
    ])
}
