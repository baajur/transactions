use std::time::SystemTime;

use diesel::sql_types::Numeric;
use diesel::sql_types::Uuid as SqlUuid;
use validator::Validate;

use models::*;
use schema::transactions;

#[derive(Debug, Queryable, Clone)]
pub struct Transaction {
    pub id: TransactionId,
    pub user_id: UserId,
    pub dr_account_id: AccountId,
    pub cr_account_id: AccountId,
    pub currency: Currency,
    pub value: Amount,
    pub status: TransactionStatus,
    pub blockchain_tx_id: Option<BlockchainTransactionId>,
    pub hold_until: Option<SystemTime>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub fee: Amount,
}

#[derive(Debug, Queryable, Clone, QueryableByName)]
pub struct TransactionSum {
    #[sql_type = "SqlUuid"]
    pub account_id: AccountId,
    #[sql_type = "Numeric"]
    pub sum: Amount,
}

impl Default for Transaction {
    fn default() -> Self {
        Self {
            id: TransactionId::generate(),
            user_id: UserId::generate(),
            dr_account_id: AccountId::generate(),
            cr_account_id: AccountId::generate(),
            currency: Currency::Stq,
            value: Amount::default(),
            status: TransactionStatus::Pending,
            blockchain_tx_id: None,
            hold_until: None,
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            fee: Amount::default(),
        }
    }
}

#[derive(Debug, Insertable, Validate, Clone)]
#[table_name = "transactions"]
pub struct NewTransaction {
    pub id: TransactionId,
    pub user_id: UserId,
    pub dr_account_id: AccountId,
    pub cr_account_id: AccountId,
    pub currency: Currency,
    pub value: Amount,
    pub status: TransactionStatus,
    pub blockchain_tx_id: Option<BlockchainTransactionId>,
    pub hold_until: Option<SystemTime>,
    pub fee: Amount,
}

impl Default for NewTransaction {
    fn default() -> Self {
        Self {
            id: TransactionId::generate(),
            user_id: UserId::generate(),
            dr_account_id: AccountId::generate(),
            cr_account_id: AccountId::generate(),
            currency: Currency::Stq,
            value: Amount::default(),
            status: TransactionStatus::Pending,
            blockchain_tx_id: None,
            hold_until: None,
            fee: Amount::default(),
        }
    }
}

#[derive(Debug, Clone, Validate)]
pub struct CreateTransactionInput {
    pub id: TransactionId,
    pub user_id: UserId,
    pub from: AccountId,
    pub to: Receipt,
    pub to_type: ReceiptType,
    pub to_currency: Currency,
    pub value: Amount,
    pub fee: Amount,
    pub hold_until: Option<SystemTime>,
}

#[derive(Debug, Clone, Validate)]
pub struct CreateTransaction {
    pub user_id: UserId,
    pub dr_account_id: AccountId,
    pub to: Receipt,
    pub to_type: ReceiptType,
    pub to_currency: Currency,
    pub value: Amount,
    pub fee: Amount,
    pub hold_until: Option<SystemTime>,
}

#[derive(Debug, Clone, Validate)]
pub struct CreateTransactionLocal {
    pub user_id: UserId,
    pub dr_account: Account,
    pub cr_account: Account,
    pub currency: Currency,
    pub value: Amount,
    pub hold_until: Option<SystemTime>,
}

impl CreateTransactionLocal {
    pub fn new(create: &CreateTransaction, dr_account: Account, cr_account: Account) -> Self {
        Self {
            user_id: create.user_id,
            dr_account,
            cr_account,
            currency: create.to_currency,
            value: create.value,
            hold_until: create.hold_until,
        }
    }
}

impl Default for CreateTransactionLocal {
    fn default() -> Self {
        Self {
            user_id: UserId::generate(),
            dr_account: Account::default(),
            cr_account: Account::default(),
            currency: Currency::Eth,
            value: Amount::default(),
            hold_until: None,
        }
    }
}

impl NewTransaction {
    pub fn from_local(create: &CreateTransactionLocal) -> Self {
        Self {
            id: TransactionId::generate(),
            user_id: create.user_id,
            dr_account_id: create.dr_account.id,
            cr_account_id: create.cr_account.id,
            currency: create.currency,
            value: create.value,
            hold_until: create.hold_until,
            status: TransactionStatus::Done,
            blockchain_tx_id: None,
            fee: Amount::default(),
        }
    }
}

#[derive(Debug, Clone, Validate)]
pub struct DepositFounds {
    pub user_id: UserId,
    pub address: AccountAddress,
    pub currency: Currency,
    pub value: Amount,
    pub blockchain_tx_id: BlockchainTransactionId,
}

impl Default for DepositFounds {
    fn default() -> Self {
        Self {
            user_id: UserId::default(),
            address: AccountAddress::default(),
            currency: Currency::Eth,
            value: Amount::default(),
            blockchain_tx_id: BlockchainTransactionId::default(),
        }
    }
}

impl NewTransaction {
    pub fn from_deposit(deposit: DepositFounds, cr_account_id: AccountId, dr_account_id: AccountId) -> Self {
        Self {
            id: TransactionId::generate(),
            user_id: deposit.user_id,
            currency: deposit.currency,
            value: deposit.value,
            hold_until: None,
            cr_account_id,
            dr_account_id,
            status: TransactionStatus::Done,
            blockchain_tx_id: Some(deposit.blockchain_tx_id),
            fee: Amount::default(),
        }
    }
}

#[derive(Debug, Clone, Validate)]
pub struct Withdraw {
    pub user_id: UserId,
    pub dr_account: Account,
    pub address: AccountAddress,
    pub currency: Currency,
    pub value: Amount,
    pub fee: Amount,
}

impl Withdraw {
    pub fn new(create: &CreateTransaction, dr_account: Account, address: AccountAddress) -> Self {
        Self {
            user_id: create.user_id,
            dr_account,
            address,
            currency: create.to_currency,
            value: create.value,
            fee: create.fee,
        }
    }
}

impl Default for Withdraw {
    fn default() -> Self {
        Self {
            user_id: UserId::default(),
            dr_account: Account::default(),
            address: AccountAddress::default(),
            currency: Currency::Eth,
            value: Amount::default(),
            fee: Amount::default(),
        }
    }
}

#[derive(Debug, Validate, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBlockchainTx {
    pub id: TransactionId,
    pub from: AccountAddress,
    pub to: AccountAddress,
    pub currency: Currency,
    pub value: Amount,
    pub fee_price: Amount,
    pub nonce: Option<u64>,
    pub utxos: Option<Vec<BitcoinUtxos>>,
}

impl Default for CreateBlockchainTx {
    fn default() -> Self {
        Self {
            id: TransactionId::generate(),
            from: AccountAddress::default(),
            to: AccountAddress::default(),
            currency: Currency::Eth,
            value: Amount::default(),
            fee_price: Amount::default(),
            nonce: Some(0),
            utxos: None,
        }
    }
}

impl CreateBlockchainTx {
    pub fn new(
        from: AccountAddress,
        to: AccountAddress,
        currency: Currency,
        value: Amount,
        fee_price: Amount,
        nonce: Option<u64>,
        utxos: Option<Vec<BitcoinUtxos>>,
    ) -> Self {
        Self {
            id: TransactionId::generate(),
            from,
            to,
            currency,
            value,
            fee_price,
            nonce,
            utxos,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BitcoinUtxos {
    tx_hash: BlockchainTransactionId,
    index: u64,
    value: Amount,
}

impl Default for BitcoinUtxos {
    fn default() -> Self {
        Self {
            tx_hash: BlockchainTransactionId::default(),
            index: 0,
            value: Amount::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransactionOut {
    pub id: TransactionId,
    pub from: Vec<TransactionAddressInfo>,
    pub to: TransactionAddressInfo,
    pub currency: Currency,
    pub value: Amount,
    pub fee: Amount,
    pub status: TransactionStatus,
    pub blockchain_tx_id: Option<BlockchainTransactionId>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

impl TransactionOut {
    pub fn new(transaction: &Transaction, from: Vec<TransactionAddressInfo>, to: TransactionAddressInfo) -> Self {
        Self {
            id: transaction.id,
            from,
            to,
            currency: transaction.currency,
            value: transaction.value,
            fee: transaction.fee,
            status: transaction.status,
            blockchain_tx_id: transaction.blockchain_tx_id.clone(),
            created_at: transaction.created_at.clone(),
            updated_at: transaction.updated_at.clone(),
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct TransactionAddressInfo {
    pub account_id: Option<AccountId>,
    pub blockchain_address: AccountAddress,
}

impl TransactionAddressInfo {
    pub fn new(account_id: Option<AccountId>, blockchain_address: AccountAddress) -> Self {
        Self {
            account_id,
            blockchain_address,
        }
    }
}
