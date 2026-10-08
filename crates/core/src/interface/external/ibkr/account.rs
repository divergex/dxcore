use std::collections::HashSet;
use std::sync::Arc;
use std::thread;

use ibapi::accounts::types::AccountId;
use ibapi::accounts::AccountUpdate;
use ibapi::contracts::Contract;
use ibapi::market_data::historical::{BarSize as IbapiBarSize, ToDuration, WhatToShow};

use super::IbkrInterface;
use crate::core::{Instrument, Portfolio};
use crate::interface::AccountInterface;
use crate::{Error, Event};

impl AccountInterface for IbkrInterface {
    fn portfolio(&self, account_id: &str) -> Result<Portfolio, Error> {
        let client = self.connect()?;
        let account = AccountId(account_id.to_string());

        let subscription = client
            .account_updates(&account)
            .map_err(|e| Error::Subscription(e.to_string()))?;

        let mut portfolio = Portfolio::default();
        let mut seen = HashSet::new();

        for update in subscription.iter_data() {
            let update = update.map_err(|e| Error::Subscription(e.to_string()))?;

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

    fn listen_async(&self, account_id: &str) -> Result<(), Error> {
        let client = self.connect()?;
        self.record(Event::Connected);

        let events = Arc::clone(&self.events);
        let account = AccountId(account_id.to_string());

        thread::spawn(move || {
            let record = |event: Event| {
                if let Ok(mut events) = events.lock() {
                    events.push(event);
                }
            };

            let Ok(subscription) = client.account_updates(&account) else {
                return;
            };

            let mut contracts: Vec<Contract> = Vec::new();
            let mut seen = HashSet::new();

            for update in subscription.iter_data() {
                let Ok(update) = update else { return };

                match update {
                    AccountUpdate::AccountValue(av) => record(Event::AccountValue(av)),
                    AccountUpdate::PortfolioValue(pv) => {
                        if pv.position != 0.0 && seen.insert(pv.contract.contract_id) {
                            contracts.push(pv.contract.clone());
                            record(Event::Position(pv));
                        }
                    }
                    AccountUpdate::UpdateTime(ut) => record(Event::UpdateTime(ut.timestamp)),
                    AccountUpdate::End => break,
                }
            }

            drop(subscription);

            for contract in &contracts {
                match client
                    .historical_data(contract, IbapiBarSize::Day)
                    .what_to_show(WhatToShow::Trades)
                    .duration(30.days())
                    .fetch()
                {
                    Ok(data) => record(Event::HistoricalBars {
                        contract_id: contract.contract_id,
                        bars: data.bars,
                    }),
                    Err(e) => record(Event::HistoricalError {
                        contract_id: contract.contract_id,
                        error: e.to_string(),
                    }),
                }
            }
        });

        Ok(())
    }

    fn events(&self, _account_id: &str) -> Result<Vec<Event>, Error> {
        let mut events = self
            .events
            .lock()
            .map_err(|_| Error::Connection("events lock poisoned".into()))?;
        Ok(events.drain(..).collect())
    }
}
