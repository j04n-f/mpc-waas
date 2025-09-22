use crate::db::models::{TransactionActiveModel, TransactionModel};
use anyhow::Result;
use sea_orm::{ActiveModelTrait, DatabaseConnection, DatabaseTransaction};

pub enum DbExecutor<'a> {
    #[allow(dead_code)]
    Connection(&'a DatabaseConnection),
    Transaction(&'a DatabaseTransaction),
}

pub struct TransactionRepository<'a> {
    executor: DbExecutor<'a>,
}

impl<'a> TransactionRepository<'a> {
    #[allow(dead_code)]
    pub fn new_with_connection(db: &'a DatabaseConnection) -> Self {
        Self {
            executor: DbExecutor::Connection(db),
        }
    }

    pub fn new_with_transaction(txn: &'a DatabaseTransaction) -> Self {
        Self {
            executor: DbExecutor::Transaction(txn),
        }
    }

    pub async fn create(&self, model: TransactionActiveModel) -> Result<TransactionModel> {
        match &self.executor {
            DbExecutor::Connection(db) => Ok(model.insert(*db).await?),
            DbExecutor::Transaction(txn) => Ok(model.insert(*txn).await?),
        }
    }
}
