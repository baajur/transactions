use std::sync::Arc;

use super::super::error::*;
use super::super::system::SystemService;
use client::{BlockchainClient, ExchangeClient, KeysClient};
use config::Config;
use models::*;
use prelude::*;
use repos::{KeyValuesRepo, PendingBlockchainTransactionsRepo};
use utils::log_and_capture_error;

pub struct FeeEstimate {
    pub gross_fee: Amount,
    pub fee_price: f64,
    pub currency: Currency,
}

pub trait BlockchainService: Send + Sync + 'static {
    fn create_bitcoin_tx(
        &self,
        from: BlockchainAddress,
        to: BlockchainAddress,
        value: Amount,
        fee_price: f64,
    ) -> Result<BlockchainTransactionId, Error>;
    fn create_ethereum_tx(
        &self,
        from: BlockchainAddress,
        to: BlockchainAddress,
        value: Amount,
        fee_price: f64,
        currency: Currency,
    ) -> Result<BlockchainTransactionId, Error>;
    fn estimate_withdrawal_fee(
        &self,
        input_gross_fee: Amount,
        input_fee_currency: Currency,
        withdrawal_currency: Currency,
    ) -> Result<FeeEstimate, Error>;
}

#[derive(Clone)]
pub struct BlockchainServiceImpl {
    config: Arc<Config>,
    keys_client: Arc<dyn KeysClient>,
    blockchain_client: Arc<dyn BlockchainClient>,
    exchange_client: Arc<dyn ExchangeClient>,
    pending_blockchain_transactions_repo: Arc<PendingBlockchainTransactionsRepo>,
    key_values_repo: Arc<KeyValuesRepo>,
    system_service: Arc<SystemService>,
}

impl BlockchainServiceImpl {
    pub fn new(
        config: Arc<Config>,
        keys_client: Arc<dyn KeysClient>,
        blockchain_client: Arc<dyn BlockchainClient>,
        exchange_client: Arc<ExchangeClient>,
        pending_blockchain_transactions_repo: Arc<PendingBlockchainTransactionsRepo>,
        key_values_repo: Arc<KeyValuesRepo>,
        system_service: Arc<SystemService>,
    ) -> Self {
        Self {
            config,
            keys_client,
            blockchain_client,
            exchange_client,
            pending_blockchain_transactions_repo,
            key_values_repo,
            system_service,
        }
    }
}

impl BlockchainService for BlockchainServiceImpl {
    // Note that withdrawal_currency may not equal to FeeEstimate currency. E.g. if
    // withdrawal_currency = stq, FeeEstimate currency will = eth (since you need eth to withdraw stq)
    fn estimate_withdrawal_fee(
        &self,
        input_gross_fee: Amount,
        input_fee_currency: Currency,
        withdrawal_currency: Currency,
    ) -> Result<FeeEstimate, Error> {
        let total_blockchain_fee_native_currency = input_gross_fee
            .checked_div(Amount::new(self.config.fees_options.fee_upside as u128))
            .ok_or(ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal))?;
        let estimate_currency = match withdrawal_currency {
            Currency::Btc => Currency::Btc,
            Currency::Eth => Currency::Eth,
            Currency::Stq => Currency::Eth,
        };
        let total_blockchain_fee_esitmate_currency = if input_fee_currency == estimate_currency {
            total_blockchain_fee_native_currency
        } else {
            let input_rate = RateInput {
                id: ExchangeId::generate(),
                from: input_fee_currency,
                to: estimate_currency,
                amount: total_blockchain_fee_native_currency,
                amount_currency: input_fee_currency,
            };
            // Todo - fix client endpoint
            let Rate { rate, .. } = self
                .exchange_client
                .rate(input_rate.clone(), Role::System)
                .wait()
                .map_err(ectx!(try ErrorKind::Internal => input_rate))?;
            total_blockchain_fee_native_currency.convert(input_fee_currency, estimate_currency, rate)
        };

        let base = match withdrawal_currency {
            Currency::Btc => self.config.fees_options.btc_transaction_size,
            Currency::Eth => self.config.fees_options.eth_gas_limit,
            Currency::Stq => self.config.fees_options.stq_gas_limit,
        };
        let base = Amount::new(base as u128);
        let fee_price_int = total_blockchain_fee_esitmate_currency
            .checked_div(base)
            .ok_or(ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal))?;
        let fee_price = if fee_price_int < Amount::new(1000) {
            (total_blockchain_fee_esitmate_currency.raw() as f64) / (base.raw() as f64)
        } else {
            fee_price_int.raw() as f64
        };
        Ok(FeeEstimate {
            gross_fee: total_blockchain_fee_esitmate_currency,
            fee_price,
            currency: estimate_currency,
        })
    }

    fn create_bitcoin_tx(
        &self,
        from: BlockchainAddress,
        to: BlockchainAddress,
        value: Amount,
        fee_price: f64,
    ) -> Result<BlockchainTransactionId, Error> {
        let from_clone = from.clone();
        let utxos = self
            .blockchain_client
            .get_bitcoin_utxos(from.clone())
            .map_err(ectx!(try convert => from_clone))
            .wait()?;

        let create_blockchain_input = CreateBlockchainTx::new(from, to, Currency::Btc, value, fee_price, None, Some(utxos));
        let create_blockchain_input_clone = create_blockchain_input.clone();

        let raw_tx = self
            .keys_client
            .sign_transaction(create_blockchain_input.clone(), Role::User)
            .map_err(ectx!(try convert => create_blockchain_input_clone, Role::User))
            .wait()?;

        let blockchain_tx_id = self
            .blockchain_client
            .post_bitcoin_transaction(raw_tx.clone())
            .map_err(ectx!(try convert => raw_tx))
            .wait()?;

        let new_pending = (create_blockchain_input, blockchain_tx_id.clone()).into();
        // Note - we don't rollback here, because the tx is already in blockchain. so after that just silently
        // fail if we couldn't write a pending tx. Not having pending tx in db doesn't do a lot of harm, we could cure
        // it later.
        match self.pending_blockchain_transactions_repo.create(new_pending) {
            Err(e) => log_and_capture_error(e),
            _ => (),
        };

        Ok(blockchain_tx_id)
    }

    fn create_ethereum_tx(
        &self,
        from: BlockchainAddress,
        to: BlockchainAddress,
        value: Amount,
        fee_price: f64,
        currency: Currency,
    ) -> Result<BlockchainTransactionId, Error> {
        match currency {
            Currency::Eth => (),
            Currency::Stq => (),
            _ => return Err(ectx!(err ErrorContext::InvalidCurrency, ErrorKind::InvalidInput(currency.to_string()))),
        };
        let tx_initiator = match currency {
            Currency::Stq => {
                self.system_service
                    .get_system_fees_account(Currency::Eth)
                    .map_err(ectx!(try ErrorKind::Internal => Currency::Eth))?
                    .address
            }
            _ => from.clone(),
        };

        let tx_initiator_ = tx_initiator.clone();
        let maybe_db_nonce = match currency {
            Currency::Stq | Currency::Eth => self
                .key_values_repo
                .get_nonce(tx_initiator_.clone())
                .map_err(ectx!(try ErrorKind::Internal))?,
            _ => None,
        };

        let tx_initiator_ = tx_initiator.clone();
        let ethereum_nonce = self
            .blockchain_client
            .get_ethereum_nonce(tx_initiator.clone())
            .map_err(ectx!(try convert => tx_initiator_))
            .wait()?;

        let nonce = match (maybe_db_nonce, ethereum_nonce) {
            (None, ethereum_nonce) => ethereum_nonce,
            // if for some reason we missed blockchain nonce
            (Some(db_nonce), ethereum_nonce) => db_nonce.max(ethereum_nonce),
        };

        let _ = self
            .key_values_repo
            .set_nonce(tx_initiator.clone(), nonce + 1)
            .map_err(ectx!(try ErrorKind::Internal => tx_initiator, nonce + 1))?;

        // TODO, at this stage transaction is dropped if there's another tx in progress
        // but this needs to be additionally verified
        // Therefore we don't do any ether transaction
        // alternative - use locks, but there are also caveats depending on the transactions isolation
        // and master / slave reads
        // https://www.postgresql.org/docs/9.6/applevel-consistency.html
        // https://www.postgresql.org/docs/9.6/explicit-locking.html

        // sleeping so there's a guaranteed interval between withdrawals
        std::thread::sleep(std::time::Duration::from_millis(1500));

        // creating blockchain transactions array
        let create_blockchain_input = CreateBlockchainTx::new(from, to, currency, value, fee_price, Some(nonce), None);

        let create_blockchain = create_blockchain_input.clone();
        let raw_tx = self
            .keys_client
            .sign_transaction(create_blockchain_input.clone(), Role::User)
            .map_err(ectx!(try convert => create_blockchain_input))
            .wait()?;
        let tx_id = self
            .blockchain_client
            .post_ethereum_transaction(raw_tx.clone())
            .map_err(ectx!(try convert => raw_tx))
            .wait()?;

        let tx_id = match currency {
            Currency::Eth => tx_id,
            // Erc-20 token, we need event log number here, to make a tx_id unique
            _ => BlockchainTransactionId::new(format!("{}:0", tx_id)),
        };
        let new_pending = (create_blockchain, tx_id.clone()).into();
        // Note - we don't rollback here, because the tx is already in blockchain. so after that just silently
        // fail if we couldn't write a pending tx. Not having pending tx in db doesn't do a lot of harm, we could cure
        // it later.
        match self.pending_blockchain_transactions_repo.create(new_pending) {
            Err(e) => log_and_capture_error(e),
            _ => (),
        };
        Ok(tx_id)
    }
}
